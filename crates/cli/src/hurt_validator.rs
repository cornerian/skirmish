//! Fox/Falco Blaster hurtbox-timing measurement tool (see `docs/hurt-validator.md`).
//!
//! Purpose: Skirmish registers Blaster hits one frame early in several
//! console (Slippi 2.0.1) recordings where laser positions are not
//! recorded. Slippi 3.x replays record item (laser) positions every frame,
//! giving ground truth for both the laser geometry and the exact recorded
//! frame a hit lands, leaving the victim's hurtbox reach under Skirmish's
//! pose model as the only unknown this tool measures.
//!
//! For every recorded Fox-laser item (Slippi item type 54) in a directory
//! of eligible `.slp` files, this tool:
//!
//! 1. Reconstructs the laser's own per-frame capsules from its *recorded*
//!    item position (never simulated): four fixed hitboxes
//!    (`specials.neutral.laser.hitboxes` of the shooter's pack fighter),
//!    offset along local -X and scaled by the same growth factor the live
//!    engine applies (`game::projectile::step`: `scale.current += |speed|
//!    / 11.25` every frame, capped at `laser.scale`), swept from the
//!    previous recorded frame's laser position (or degenerate on the spawn
//!    frame, since no earlier recorded position exists then).
//! 2. Reconstructs the *other* fighter's hurtbox capsules every frame from
//!    the recorded post-frame observation alone (action state, state age,
//!    position, facing, airborne) -- not from a full driven simulation.
//!    This reuses the same reverse action-state mapping and pose-selection
//!    logic `crates/skirmish-replay`'s `observation.rs` and
//!    `src/game/simulation.rs::local_pose` use, but as a same-crate-API
//!    port rather than a call: `simulation::pose`/`local_pose`/
//!    `hurtbox_state` are `pub(crate)` inside the `skirmish` library crate
//!    and are not reachable from this external tool crate without editing
//!    simulator source, which this batch does not do. See "Coverage" below
//!    for exactly which actions this reimplements and which it skips.
//! 3. Tests contact with `collision::shield::capsule_matrix`, the same
//!    primitive `game::projectile::step` calls, at `broadphase_scale =
//!    3.0` (`lbColl_80006E58`'s real item-vs-fighter broadphase scale;
//!    Skirmish's own live projectile path currently passes `1.0` -- an
//!    unrelated, pre-existing simplification this tool does not modify or
//!    inherit).
//! 4. Compares the first frame with simulated contact against the first
//!    frame the recording shows the hit (victim `percent` increases with
//!    `last_hit_by` equal to the shooter's port, or -- when no such frame
//!    exists -- the laser's last recorded frame if it disappeared adjacent
//!    to the victim).
//!
//! **Coverage.** Only the action-state family `game::movement::pose`
//! already handles from `action_frame` alone, plus `LandingFallSpecial`
//! (via `escape_air::pose`'s `landing_elapsed` indexing, itself already
//! bit-exact to the recorded `state_age` per `docs/validation.md`'s
//! 2026-09-14 entry) is reimplemented: Wait (bind-pose fallback when the
//! idle sub-motion, not recoverable from recorded fields, is unknown),
//! Walk (15/16/17), Turn, RunTurn, Dash, Run, RunBrake, JumpSquat, Jump
//! (25/26), JumpAerial (27/28), Fall (29/32), FallSpecial, Landing,
//! LandingFallSpecial, Squat, SquatWait, SquatRv, Pass, Ottotto,
//! OttottoWait, EntryStart. Every other recorded victim action (grabs,
//! ledge, wall-jump, damage/hitstun, dodges/rolls, shield, attacks,
//! specials, death/respawn, entry, prone) needs additional Fighter
//! sub-state (grab target, ledge timer, attack-frame index tables, ...)
//! that the recorded post-frame fields alone do not determine, and is
//! skipped per-frame with the skipped state tallied in the report, per
//! this task's own "if the mapping is not available for some actions, say
//! which and skip them" instruction.
//!
//! No simulator source (`src/`, `crates/skirmish-replay`) is modified by
//! this batch; every helper below is a small, cited, from-scratch port
//! built only from `pub` items of the `skirmish` library crate.
use anyhow::{Context, Result, anyhow};
use serde::Serialize;
use skirmish::{
    characters::Specials,
    collision::{
        bones::{self, BoneCapsule, Matrix, Pose},
        shield::{Contact, capsule_matrix},
    },
    fighter::combat::Capsule,
    game::{
        Action, Fighter,
        data::{Bone as DataBone, FighterData, MatchData},
        locomotion::WalkKind,
        movement::loop_period,
    },
};
use skirmish_replay::slippi::{self, Port, Timeline, row};
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

const FOX_LASER_ITEM_TYPE: u16 = 54;
/// The real item-vs-fighter broadphase scale (`lbColl_80006E58`'s own
/// caller convention, matching `docs/shield.md`'s "the shield caller
/// supplies 20 * its native scale" note extended to items: 3.0 is the
/// pinned item broadphase multiplier the task's own citation gives).
/// `game::projectile::step` currently passes `1.0` on this path (a known,
/// pre-existing, unrelated simplification); this tool intentionally uses
/// the more faithful `3.0` instead, per the task's own instruction, not by
/// reading it back out of the live (currently-1.0) call site.
const BROADPHASE_SCALE: f32 = 3.0;

/// `CHARACTER_EXTERNAL_IDS`/`STAGE_EXTERNAL_IDS` (`crate::initialization`)
/// give the slug<->external-id table; this tool adds the separate
/// directory-abbreviation convention `skirmish-assets` gameplay-export
/// pack directories use (`fox-fd`, `fox-falco-fd`, ...), which is pack
/// layout, not simulator/replay data, so it is not already defined
/// anywhere else in this workspace.
const STAGE_DIR_CODE: &[(&str, &str)] = &[
    ("fountain-of-dreams", "fod"),
    ("pokemon-stadium", "ps"),
    ("yoshis-story", "ys"),
    ("dream-land", "dl"),
    ("battlefield", "bf"),
    ("final-destination", "fd"),
];

fn stage_dir_code(slug: &str) -> Option<&'static str> {
    STAGE_DIR_CODE
        .iter()
        .find(|(name, _)| *name == slug)
        .map(|(_, code)| *code)
}

fn character_slug(external_id: u8) -> Option<&'static str> {
    crate::initialization::CHARACTER_EXTERNAL_IDS
        .iter()
        .find(|(_, id)| *id == external_id)
        .map(|(name, _)| *name)
}

fn stage_slug(external_id: u16) -> Option<&'static str> {
    crate::initialization::STAGE_EXTERNAL_IDS
        .iter()
        .find(|(_, id)| *id == external_id)
        .map(|(name, _)| *name)
}

pub struct Args {
    pub dataset_dir: PathBuf,
    pub pack_root: PathBuf,
    pub time_budget: Duration,
    pub limit: Option<usize>,
}

/// `(frame id, position, velocity, owner port index)`.
type LaserFrame = (i32, [f32; 2], [f32; 2], Option<i8>);

/// One recorded Fox-laser item's full observed lifetime.
struct LaserTrack {
    id: u32,
    spawn_frame: i32,
    /// Ascending by frame id.
    frames: Vec<LaserFrame>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum SkipReason {
    Reflected,
    ShieldContact,
    VictimIntangible,
    VictimActionUnsupported,
    NoRecordedHit,
    PoseError,
}

#[derive(Debug, Serialize)]
struct EventOutcome {
    file: String,
    item_id: u32,
    spawn_frame: i32,
    shooter_port: u8,
    victim_port: u8,
    recorded_hit_frame: Option<i32>,
    recorded_victim_action: Option<u16>,
    recorded_victim_action_name: Option<&'static str>,
    simulated_hit_frame: Option<i32>,
    delta_frames: Option<i32>,
    frame_before_x_overlap: Option<f32>,
    skip: Option<SkipReason>,
}

#[derive(Default, Debug, Serialize)]
pub struct Report {
    files_considered: usize,
    files_eligible: usize,
    files_processed: usize,
    files_failed: usize,
    laser_items_seen: usize,
    events: Vec<EventOutcome>,
    unsupported_action_tally: HashMap<u16, usize>,
    errors: Vec<String>,
}

pub fn run(args: Args) -> Result<Report> {
    let start = Instant::now();
    let mut report = Report::default();
    let mut files = Vec::new();
    collect_slp_files(&args.dataset_dir, &mut files)?;
    files.sort();
    report.files_considered = files.len();

    let mut pack_cache: HashMap<String, Arc<MatchData>> = HashMap::new();

    for path in files {
        if start.elapsed() > args.time_budget {
            break;
        }
        if let Some(limit) = args.limit
            && report.files_processed >= limit
        {
            break;
        }
        match process_file(&path, &args.pack_root, &mut pack_cache) {
            Ok(Some(mut events)) => {
                report.files_eligible += 1;
                report.files_processed += 1;
                report.laser_items_seen += events.len();
                for event in &events {
                    if event.skip == Some(SkipReason::VictimActionUnsupported)
                        && let Some(id) = event.recorded_victim_action
                    {
                        *report.unsupported_action_tally.entry(id).or_default() += 1;
                    }
                }
                report.events.append(&mut events);
            }
            Ok(None) => {}
            Err(error) => {
                report.files_failed += 1;
                report.errors.push(format!("{}: {error:#}", path.display()));
            }
        }
    }
    Ok(report)
}

fn collect_slp_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_slp_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("slp") {
            out.push(path);
        }
    }
    Ok(())
}

/// `Ok(None)` means the file was considered but is not eligible (wrong
/// version/stage/characters, or no matching pack directory); `Ok(Some(_))`
/// (possibly empty) means it was fully processed.
fn process_file(
    path: &Path,
    pack_root: &Path,
    pack_cache: &mut HashMap<String, Arc<MatchData>>,
) -> Result<Option<Vec<EventOutcome>>> {
    let bytes = fs::read(path)?;
    let replay = match slippi::Replay::read(io::Cursor::new(&bytes)) {
        Ok(replay) => replay,
        Err(_) => return Ok(None),
    };
    let start = &replay.game().start;
    if start.slippi.version.0 != 3 {
        return Ok(None);
    }
    if start.is_teams || start.players.len() != 2 {
        return Ok(None);
    }
    let mut players = start.players.clone();
    players.sort_by_key(|player| player.port);
    let characters = [players[0].character, players[1].character];
    if !characters.iter().all(|character| {
        character_slug(*character).is_some_and(|slug| slug == "fox" || slug == "falco")
    }) {
        return Ok(None);
    }
    let Some(stage) = stage_slug(start.stage) else {
        return Ok(None);
    };
    let Some(stage_code) = stage_dir_code(stage) else {
        return Ok(None);
    };
    let char_slugs = [
        character_slug(characters[0]).unwrap(),
        character_slug(characters[1]).unwrap(),
    ];
    let Some(pack_dir) = find_pack_dir(pack_root, char_slugs, stage_code) else {
        return Ok(None);
    };
    let data = load_pack(pack_root, &pack_dir, pack_cache)?;

    // Map sorted ports -> pack fighter index, by character slug.
    let ports = [players[0].port, players[1].port];
    let fighter_index_by_port = match_pack_fighters(&data, char_slugs)?;

    let file_label = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let indices = replay.frame_indices(Timeline::LastRecorded)?;
    let mut laser_tracks: HashMap<u32, LaserTrack> = HashMap::new();
    // Per-frame post-frame records for each of our two tracked ports.
    let mut records: Vec<(i32, [Option<row::Post>; 2])> = Vec::with_capacity(indices.len());

    for &physical in indices {
        let frame = replay.frame(physical)?;
        let mut post = [None, None];
        for actor in &frame.actors {
            if actor.follower {
                continue;
            }
            if let Some(slot) = ports.iter().position(|p| *p == actor.port) {
                post[slot] = Some(actor.post);
            }
        }
        records.push((frame.id, post));

        if let Some(items) = &frame.items {
            for item in items {
                if item.r#type != FOX_LASER_ITEM_TYPE {
                    continue;
                }
                let track = laser_tracks.entry(item.id).or_insert_with(|| LaserTrack {
                    id: item.id,
                    spawn_frame: frame.id,
                    frames: Vec::new(),
                });
                track.frames.push((
                    frame.id,
                    [item.position.x, item.position.y],
                    [item.velocity.x, item.velocity.y],
                    item.owner,
                ));
            }
        }
    }

    let mut events = Vec::new();
    let mut tracks: Vec<LaserTrack> = laser_tracks.into_values().collect();
    tracks.sort_by_key(|t| (t.spawn_frame, t.id));
    for track in tracks {
        events.push(evaluate_laser(
            &file_label,
            &track,
            &data,
            &fighter_index_by_port,
            &ports,
            &records,
        ));
    }
    Ok(Some(events))
}

fn find_pack_dir(pack_root: &Path, chars: [&str; 2], stage_code: &str) -> Option<PathBuf> {
    let candidates: Vec<String> = if chars[0] == chars[1] {
        vec![format!("{}-{}", chars[0], stage_code)]
    } else {
        vec![
            format!("{}-{}-{}", chars[0], chars[1], stage_code),
            format!("{}-{}-{}", chars[1], chars[0], stage_code),
        ]
    };
    candidates
        .into_iter()
        .map(|name| pack_root.join(name))
        .find(|path| {
            path.join("match-data.json").is_file() || path.join("match-data.bin").is_file()
        })
}

fn load_pack(
    pack_root: &Path,
    pack_dir: &Path,
    cache: &mut HashMap<String, Arc<MatchData>>,
) -> Result<Arc<MatchData>> {
    let key = pack_dir
        .strip_prefix(pack_root)
        .unwrap_or(pack_dir)
        .to_string_lossy()
        .into_owned();
    if let Some(data) = cache.get(&key) {
        return Ok(Arc::clone(data));
    }
    let candidate_bin = pack_dir.join("match-data.bin");
    let candidate_json = pack_dir.join("match-data.json");
    let path = if candidate_bin.is_file() {
        candidate_bin
    } else {
        candidate_json
    };
    let data = crate::pack::load_match_data(&path)
        .with_context(|| format!("loading pack {}", path.display()))?;
    let data = Arc::new(data);
    cache.insert(key, Arc::clone(&data));
    Ok(data)
}

fn match_pack_fighters(data: &MatchData, char_slugs: [&str; 2]) -> Result<[usize; 2]> {
    let mut used = [false; 2];
    let mut out = [0usize; 2];
    for (slot, slug) in char_slugs.iter().enumerate() {
        let index = (0..2)
            .find(|&i| !used[i] && character_slug_matches(&data.fighters[i].name, slug))
            .ok_or_else(|| anyhow!("pack does not contain a {slug} fighter slot"))?;
        used[index] = true;
        out[slot] = index;
    }
    Ok(out)
}

fn character_slug_matches(fighter_name: &str, slug: &str) -> bool {
    fighter_name.eq_ignore_ascii_case(slug) || fighter_name.to_ascii_lowercase() == slug
}

/// Slippi action-state ids `game::movement::pose` (plus
/// `escape_air::pose`'s `LandingFallSpecial` arm) already knows how to
/// select a pose for from nothing but the recorded post-frame fields; see
/// the module doc's "Coverage" section. `None` means this tool does not
/// reconstruct a pose for that state.
fn action_from_state(state: u16) -> Option<Action> {
    Some(match state {
        14 => Action::Wait,
        15..=17 => Action::Walk,
        18 => Action::Turn,
        19 => Action::RunTurn,
        20 => Action::Dash,
        21 => Action::Run,
        23 => Action::RunBrake,
        24 => Action::JumpSquat,
        25 | 26 => Action::Jump,
        27 | 28 => Action::JumpAerial,
        29 | 32 => Action::Fall,
        35 => Action::FallSpecial,
        39 => Action::Squat,
        40 => Action::SquatWait,
        41 => Action::SquatRv,
        42 => Action::Landing,
        43 => Action::LandingFallSpecial,
        244 => Action::Pass,
        245 => Action::Ottotto,
        246 => Action::OttottoWait,
        323 => Action::EntryStart,
        _ => return None,
    })
}

fn action_display_name(state: u16) -> &'static str {
    match state {
        14 => "Wait",
        15 => "Walk (Slow)",
        16 => "Walk (Middle)",
        17 => "Walk (Fast)",
        18 => "Turn",
        19 => "RunTurn",
        20 => "Dash",
        21 => "Run",
        23 => "RunBrake",
        24 => "JumpSquat (KneeBend)",
        25 => "Jump (forward)",
        26 => "Jump (backward)",
        27 => "JumpAerial (forward)",
        28 => "JumpAerial (backward)",
        29 => "Fall",
        32 => "Fall (aerial)",
        35 => "FallSpecial",
        39 => "Squat",
        40 => "SquatWait",
        41 => "SquatRv",
        42 => "Landing",
        43 => "LandingFallSpecial",
        178 => "GuardOn",
        179 => "Guard",
        180 => "GuardOff",
        181 => "GuardSetOff",
        182 => "GuardReflect",
        244 => "Pass",
        245 => "Ottotto",
        246 => "OttottoWait",
        323 => "EntryStart",
        _ => "unsupported",
    }
}

fn is_shield_state(state: u16) -> bool {
    matches!(state, 178..=182 | 205..=210)
}

/// Build a fully-defaulted `Fighter` with only the fields this tool's
/// covered pose family reads set from recorded observation, mirroring
/// `game::simulation::spawn`'s own field-by-field construction (every
/// field not listed there is defaulted the identical way here).
fn observed_fighter(
    action: Action,
    action_frame: u32,
    position: [f32; 2],
    facing: f32,
    grounded: bool,
) -> Fighter {
    Fighter {
        script_state: Default::default(),
        position,
        depth: 0.0,
        deferred_position: [0.0; 3],
        nudge: [0.0; 2],
        velocity: [0.0; 2],
        knockback: [0.0; 2],
        ground_knockback: 0.0,
        ground_velocity: 0.0,
        facing,
        grounded,
        ground_line: None,
        last_ground_line: None,
        skip_floor: None,
        floor_normal: [0.0, 1.0, 0.0],
        contacts: [None; 4],
        edge_contact: None,
        ecb: Default::default(),
        ecb_lock: 0,
        locomotion: Default::default(),
        shield: Default::default(),
        aerial: Default::default(),
        side_special: Default::default(),
        up_special: Default::default(),
        down_special: Default::default(),
        neutral_special: Default::default(),
        tilt: Default::default(),
        smash: Default::default(),
        dash: Default::default(),
        jab: Default::default(),
        idle: Default::default(),
        clank: Default::default(),
        grab: Default::default(),
        ledge: Default::default(),
        death: Default::default(),
        entry: Default::default(),
        action,
        action_frame,
        percent: 0.0,
        stocks: 4,
        hitlag: 0.0,
        hitstun: 0,
        action_instance: Default::default(),
        combo: Default::default(),
        damage_elapsed: -1,
        damage_angle_flag: 0,
        damage_angle_timer: 0,
        di_pending: false,
        tumbling: false,
        prone: None,
        down_timer: 0,
        damage_motion: None,
        last_damage_surface: None,
        reflect_lockout: 0,
        surface_tech: Default::default(),
        wall_jump: Default::default(),
        invincibility: 0,
        intangibility: 0,
        body_state: Default::default(),
        l_cancel_status: 0,
        landing_allow_interrupt: false,
        short_hop: false,
        fast_fall: false,
        hit_groups: 0,
        hitboxes: Default::default(),
        staling: Default::default(),
        previous_input: Default::default(),
        pose_blend: Default::default(),
    }
}

/// Port of `game::movement::pose`'s reachable match arms (the actions
/// listed in `action_from_state`, less `LandingFallSpecial`, which
/// `escape_air::pose` claims first in the real priority chain -- see
/// `reconstruct_pose` below). `None` here means "no track for this frame",
/// which the real `local_pose` chain also falls through to `&data.bones`
/// for; this function's caller applies that identical fallback.
fn movement_pose_subset<'a>(fighter: &Fighter, data: &'a FighterData) -> Option<&'a [DataBone]> {
    let poses = data.movement_poses.as_ref()?;
    let (frames, raw, looping): (&Vec<Vec<DataBone>>, usize, bool) = match fighter.action {
        Action::Wait => {
            if fighter.idle.animation != 2 {
                return None;
            }
            (poses.wait.as_ref()?, fighter.idle.frame as usize, true)
        }
        Action::Walk => {
            let field = match fighter.locomotion.walk.kind {
                WalkKind::Slow => &poses.walk_slow,
                WalkKind::Middle => &poses.walk_middle,
                WalkKind::Fast => &poses.walk_fast,
            };
            (
                field.as_ref()?,
                fighter.locomotion.walk.frame as usize,
                true,
            )
        }
        Action::Run => (
            poses.run.as_ref()?,
            fighter.locomotion.run.frame as usize,
            true,
        ),
        Action::Turn => (poses.turn.as_ref()?, fighter.action_frame as usize, false),
        Action::RunTurn => (
            poses.turn_run.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Dash => (poses.dash.as_ref()?, fighter.action_frame as usize, false),
        Action::RunBrake => (
            poses.run_brake.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::JumpSquat => (
            poses.knee_bend.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Jump if fighter.locomotion.jump_backward => {
            (poses.jump_b.as_ref()?, fighter.action_frame as usize, false)
        }
        Action::Jump => (poses.jump_f.as_ref()?, fighter.action_frame as usize, false),
        Action::JumpAerial if fighter.locomotion.jump_backward => (
            poses.jump_aerial_b.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::JumpAerial => (
            poses.jump_aerial_f.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Fall if fighter.locomotion.fall_aerial => (
            poses.fall_aerial.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::Fall => (poses.fall.as_ref()?, fighter.action_frame as usize, true),
        Action::FallSpecial => (
            poses.fall_special.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::Landing => (
            poses.landing.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Squat => (poses.squat.as_ref()?, fighter.action_frame as usize, false),
        Action::SquatWait => (
            poses.squat_wait.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::SquatRv => (
            poses.squat_rv.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::Pass => (poses.pass.as_ref()?, fighter.action_frame as usize, false),
        Action::Ottotto => (
            poses.ottotto.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        Action::OttottoWait => (
            poses.ottotto_wait.as_ref()?,
            fighter.action_frame as usize,
            true,
        ),
        Action::EntryStart => (
            poses.entry_start.as_ref()?,
            fighter.action_frame as usize,
            false,
        ),
        _ => return None,
    };
    if frames.is_empty() {
        return None;
    }
    let index = if looping {
        raw % loop_period(frames.len())
    } else {
        raw.min(frames.len() - 1)
    };
    frames.get(index).map(Vec::as_slice)
}

/// Port of `game::simulation::pose`'s root-facing/model-scale construction
/// and external root translation, applied on top of
/// `movement_pose_subset`/`escape_air`'s `LandingFallSpecial` arm (ported
/// inline below) with the bind-pose (`data.bones`) fallback every other
/// `local_pose` source this tool does not reimplement would also fall
/// through to for an unsupported/missing track. `multi_jump_yaw` and
/// `death.camera_offset` are left at zero (this tool's synthetic fighter
/// never sets them): both default to zero in the real engine too, and
/// neither affects any of this tool's covered actions except
/// `JumpAerial`'s own root yaw, a documented, minor simplification.
fn to_physics_bone(bone: &DataBone) -> bones::Bone {
    bones::Bone {
        parent: bone.parent,
        classical_scale: bone.classical_scale,
        local: bones::LocalTransform {
            translation: bone.translation,
            rotation: bone.rotation,
            rotation_quaternion: None,
            scale: bone.scale,
        },
    }
}

fn reconstruct_pose(fighter: &Fighter, data: &FighterData) -> Result<Pose> {
    let local: &[DataBone] = if fighter.action == Action::LandingFallSpecial
        && let Some(air) = data.escape_air.as_ref()
        && let Some(frame) = air.landing_poses.get(fighter.action_frame as usize)
    {
        frame.as_slice()
    } else {
        movement_pose_subset(fighter, data).unwrap_or(&data.bones)
    };
    let mut bones: Vec<bones::Bone> = local.iter().map(to_physics_bone).collect();
    if let Some(root) = bones.first_mut() {
        root.local.rotation = [0.0, core::f32::consts::FRAC_PI_2 * fighter.facing, 0.0];
        let model_scale = data.model_scaling.unwrap_or(1.0);
        root.local.scale = [model_scale; 3];
    }
    let root: Matrix = [
        [1.0, 0.0, 0.0, fighter.position[0]],
        [0.0, 1.0, 0.0, fighter.position[1]],
        [0.0, 0.0, 1.0, fighter.depth],
    ];
    Pose::evaluate_with_root(&bones, &root).map_err(|e| anyhow!("pose evaluation failed: {e:?}"))
}

/// `action_frame` truncates a fractional recorded `state_age` the same way
/// `escape_air::pose`'s `landing_elapsed as usize` and every raw-index
/// `movement::pose` arm's implicit `f32 -> u32 -> usize` conversion do.
fn recorded_action_frame(state_age: f32) -> u32 {
    state_age.max(0.0) as u32
}

/// Build the synthetic victim `Fighter` this tool's covered pose family
/// needs for one recorded frame; `None` when the recorded action is
/// outside `action_from_state`'s coverage.
fn victim_fighter(post: &row::Post) -> Option<Fighter> {
    let action = action_from_state(post.state)?;
    let facing = if post.direction >= 0.0 { 1.0 } else { -1.0 };
    let state_age = post.state_age.unwrap_or(0.0);
    let grounded = post.airborne.map(|a| a == 0).unwrap_or(true);
    let mut fighter = observed_fighter(
        action,
        recorded_action_frame(state_age),
        [post.position.x, post.position.y],
        facing,
        grounded,
    );
    match post.state {
        15 => fighter.locomotion.walk.kind = WalkKind::Slow,
        16 => fighter.locomotion.walk.kind = WalkKind::Middle,
        17 => fighter.locomotion.walk.kind = WalkKind::Fast,
        26 | 28 => fighter.locomotion.jump_backward = true,
        32 => fighter.locomotion.fall_aerial = true,
        _ => {}
    }
    // `fighter.locomotion.walk.frame`: `movement::pose`'s Walk arm reads
    // this float counter, not `action_frame`. It only ever advances while
    // the pack supplies `movement.walk_animation` (`docs/walk.md`); absent
    // that, the real engine leaves it at its `0.0` default forever, so
    // this mirrors that exactly rather than guessing a rate.
    if matches!(post.state, 15..=17) {
        fighter.locomotion.walk.frame = state_age;
    }
    Some(fighter)
}

/// Victim hurtbox eligibility for this tool's covered actions reduces to
/// the hurtbox's own static declared state: none of them are attacks or
/// edge/teeter actions, the only two dynamic overrides
/// `game::simulation::hurtbox_state` applies.
fn hurtbox_enabled(data: &FighterData, index: usize) -> bool {
    data.hurtboxes
        .get(index)
        .map(|h| h.state.accepts_contact())
        .unwrap_or(false)
}

/// One frame's laser capsule set: fixed local offsets/radii from the pack,
/// and this frame's/previous frame's growth-scaled world offsets.
fn laser_world_capsules(
    center_offsets: &[([f32; 3], f32)], // (local center, radius)
    position: [f32; 2],
    facing: f32,
    scale_factor: f32,
) -> Vec<Capsule> {
    center_offsets
        .iter()
        .map(|(center, radius)| {
            let point = [
                position[0] + center[0] * facing * scale_factor,
                position[1] + center[1] * scale_factor,
                center[2] * scale_factor,
            ];
            Capsule {
                start: point,
                end: point,
                radius: *radius,
            }
        })
        .collect()
}

fn swept_laser_capsules(
    center_offsets: &[([f32; 3], f32)],
    previous_position: [f32; 2],
    previous_scale: f32,
    position: [f32; 2],
    scale: f32,
    facing: f32,
) -> Vec<Capsule> {
    center_offsets
        .iter()
        .map(|(center, radius)| {
            let start = [
                previous_position[0] + center[0] * facing * previous_scale,
                previous_position[1] + center[1] * previous_scale,
                center[2] * previous_scale,
            ];
            let end = [
                position[0] + center[0] * facing * scale,
                position[1] + center[1] * scale,
                center[2] * scale,
            ];
            Capsule {
                start,
                end,
                radius: *radius,
            }
        })
        .collect()
}

fn victim_world_hurtboxes(
    data: &FighterData,
    pose: &Pose,
) -> Result<Vec<(usize, Capsule, Matrix)>> {
    let mut out = Vec::new();
    for (index, hurtbox) in data.hurtboxes.iter().enumerate() {
        if !hurtbox_enabled(data, index) {
            continue;
        }
        let capsule = BoneCapsule {
            bone: hurtbox.bone,
            start: hurtbox.start,
            end: hurtbox.end,
            radius: hurtbox.radius,
        };
        let world = capsule
            .transform(pose, 1.0)
            .map_err(|e| anyhow!("hurtbox transform failed: {e:?}"))?;
        let matrix = *pose
            .world_matrix(hurtbox.bone)
            .map_err(|e| anyhow!("world matrix failed: {e:?}"))?;
        out.push((
            index,
            Capsule {
                start: world.start,
                end: world.end,
                radius: world.radius,
            },
            matrix,
        ));
    }
    Ok(out)
}

/// `(min_x, max_x)` of a capsule's world extent, ignoring radius direction
/// (a simple axis-aligned bound, not the exact rounded-capsule silhouette).
fn x_extent(capsule: &Capsule) -> (f32, f32) {
    let lo = capsule.start[0].min(capsule.end[0]) - capsule.radius;
    let hi = capsule.start[0].max(capsule.end[0]) + capsule.radius;
    (lo, hi)
}

/// This tool's own diagnostic, not a call into any engine primitive: the
/// signed overlap of two capsules' axis-aligned X extents (positive =
/// overlapping ranges, negative = gap size), maximized over every
/// hitbox/hurtbox pair. A simple, self-contained proxy for "how close was
/// the closest pair to overlapping on X", independent of the exact
/// `capsule_matrix` 3D geometry.
fn best_x_overlap(hits: &[Capsule], hurts: &[(usize, Capsule, Matrix)]) -> Option<f32> {
    let mut best: Option<f32> = None;
    for hit in hits {
        let (h_lo, h_hi) = x_extent(hit);
        for (_, hurt, _) in hurts {
            let (v_lo, v_hi) = x_extent(hurt);
            let overlap = h_hi.min(v_hi) - h_lo.max(v_lo);
            best = Some(best.map_or(overlap, |b: f32| b.max(overlap)));
        }
    }
    best
}

fn contact_this_frame(hits: &[Capsule], hurts: &[(usize, Capsule, Matrix)]) -> Result<bool> {
    for hit in hits {
        for (_, hurt, matrix) in hurts {
            let mut contact = Contact::default();
            if capsule_matrix(hit, hurt, matrix, BROADPHASE_SCALE, &mut contact)
                .map_err(|e| anyhow!("capsule_matrix failed: {e:?}"))?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[allow(clippy::too_many_arguments)]
fn evaluate_laser(
    file_label: &str,
    track: &LaserTrack,
    data: &MatchData,
    fighter_index_by_port: &[usize; 2],
    ports: &[Port; 2],
    records: &[(i32, [Option<row::Post>; 2])],
) -> EventOutcome {
    let base = EventOutcome {
        file: file_label.to_string(),
        item_id: track.id,
        spawn_frame: track.spawn_frame,
        shooter_port: 0,
        victim_port: 0,
        recorded_hit_frame: None,
        recorded_victim_action: None,
        recorded_victim_action_name: None,
        simulated_hit_frame: None,
        delta_frames: None,
        frame_before_x_overlap: None,
        skip: None,
    };
    let Some(&(_, _, _, Some(owner0))) = track.frames.first() else {
        return EventOutcome {
            skip: Some(SkipReason::NoRecordedHit),
            ..base
        };
    };
    let reflected = track
        .frames
        .iter()
        .any(|&(_, _, _, owner)| owner != Some(owner0));
    if reflected {
        return EventOutcome {
            skip: Some(SkipReason::Reflected),
            ..base
        };
    }
    let Some(shooter_slot) = ports.iter().position(|p| *p as u8 == owner0 as u8) else {
        return EventOutcome {
            skip: Some(SkipReason::NoRecordedHit),
            ..base
        };
    };
    let victim_slot = 1 - shooter_slot;
    let shooter_port = ports[shooter_slot] as u8;
    let victim_port = ports[victim_slot] as u8;

    let shooter_data = &data.fighters[fighter_index_by_port[shooter_slot]];
    let victim_data = &data.fighters[fighter_index_by_port[victim_slot]];

    let Some((speed, laser_hitboxes, scale_cap)) = shooter_laser(shooter_data) else {
        return EventOutcome {
            shooter_port,
            victim_port,
            skip: Some(SkipReason::NoRecordedHit),
            ..base
        };
    };
    let per_frame = speed.abs() / 11.25;
    let center_offsets: Vec<([f32; 3], f32)> = laser_hitboxes
        .iter()
        .map(|h| (h.center, h.radius))
        .collect();
    let scale_at = |frames_since_spawn: i32| -> f32 {
        if frames_since_spawn < 0 {
            0.0
        } else {
            (per_frame * (frames_since_spawn as f32 + 1.0)).min(scale_cap)
        }
    };

    // Index records by frame id for quick lookup, restricted to the
    // laser's own recorded lifetime window.
    let by_id: HashMap<i32, &[Option<row::Post>; 2]> =
        records.iter().map(|(id, post)| (*id, post)).collect();

    // Recorded hit frame: first frame in the laser's own recorded lifetime
    // where the victim's percent rises with last_hit_by == shooter *and*
    // this specific laser's own recorded position is plausibly within its
    // own reach of the victim -- else (fallback) the laser's last recorded
    // frame if it ends adjacent to the victim. The proximity gate matters:
    // the shooter can land an unrelated hit (a jab, an aerial, a second
    // laser instance, ...) on the same frame a first laser instance is
    // merely still coasting through open space many units away, and
    // `percent`/`last_hit_by` alone cannot tell those apart.
    const PLAUSIBLE_REACH: f32 = 20.0;
    let despawn_frame = track
        .frames
        .last()
        .map(|f| f.0)
        .unwrap_or(track.spawn_frame);
    let mut recorded_hit_frame = None;
    let mut previous_percent = None;
    for &(id, position, _velocity, _owner) in &track.frames {
        let Some(post) = by_id.get(&id).and_then(|p| p[victim_slot]) else {
            continue;
        };
        if let Some(previous) = previous_percent
            && post.percent > previous
            && post.last_hit_by == shooter_port
        {
            let dx = position[0] - post.position.x;
            let dy = position[1] - post.position.y;
            if (dx * dx + dy * dy).sqrt() < PLAUSIBLE_REACH {
                recorded_hit_frame = Some(id);
                break;
            }
        }
        previous_percent = Some(post.percent);
    }
    if recorded_hit_frame.is_none()
        && let Some(post) = by_id.get(&despawn_frame).and_then(|p| p[victim_slot])
        && let Some(&(_, laser_pos, ..)) = track.frames.last()
    {
        let dx = laser_pos[0] - post.position.x;
        let dy = laser_pos[1] - post.position.y;
        if (dx * dx + dy * dy).sqrt() < 8.0 {
            recorded_hit_frame = Some(despawn_frame);
        }
    }

    let Some(hit_frame) = recorded_hit_frame else {
        return EventOutcome {
            shooter_port,
            victim_port,
            skip: Some(SkipReason::NoRecordedHit),
            ..base
        };
    };

    // Shield/intangible exclusion, scanned over the laser's whole flight.
    let mut shielded = false;
    let mut intangible = false;
    let mut recorded_action = None;
    let mut recorded_action_name = None;
    for &(id, ..) in &track.frames {
        if id > hit_frame {
            break;
        }
        let Some(post) = by_id.get(&id).and_then(|p| p[victim_slot]) else {
            continue;
        };
        if is_shield_state(post.state) {
            shielded = true;
        }
        if post.hurtbox_state == Some(2) {
            intangible = true;
        }
        if id == hit_frame {
            recorded_action = Some(post.state);
            recorded_action_name = Some(action_display_name(post.state));
        }
    }
    if shielded || intangible {
        return EventOutcome {
            shooter_port,
            victim_port,
            recorded_hit_frame: Some(hit_frame),
            recorded_victim_action: recorded_action,
            recorded_victim_action_name: recorded_action_name,
            skip: Some(if intangible {
                SkipReason::VictimIntangible
            } else {
                SkipReason::ShieldContact
            }),
            ..base
        };
    }

    let supported_at_hit = recorded_action.is_some_and(|a| action_from_state(a).is_some());
    if !supported_at_hit {
        return EventOutcome {
            shooter_port,
            victim_port,
            recorded_hit_frame: Some(hit_frame),
            recorded_victim_action: recorded_action,
            recorded_victim_action_name: recorded_action_name,
            skip: Some(SkipReason::VictimActionUnsupported),
            ..base
        };
    }

    // Simulated contact scan.
    let mut simulated_hit_frame = None;
    let mut pose_error = false;
    let mut previous_track_position = None;
    for &(id, position, _velocity, _owner) in &track.frames {
        let Some(victim_post) = by_id.get(&id).and_then(|p| p[victim_slot]) else {
            previous_track_position = Some(position);
            continue;
        };
        let Some(victim) = victim_fighter(&victim_post) else {
            previous_track_position = Some(position);
            continue;
        };
        let pose = match reconstruct_pose(&victim, victim_data) {
            Ok(pose) => pose,
            Err(_) => {
                pose_error = true;
                previous_track_position = Some(position);
                continue;
            }
        };
        let hurts = match victim_world_hurtboxes(victim_data, &pose) {
            Ok(hurts) => hurts,
            Err(_) => {
                pose_error = true;
                previous_track_position = Some(position);
                continue;
            }
        };
        let frames_since_spawn = id - track.spawn_frame;
        let facing = if position[0]
            >= previous_track_position
                .map(|p| p[0])
                .unwrap_or(position[0] - 1.0)
        {
            1.0
        } else {
            -1.0
        };
        let hits = if id != track.spawn_frame
            && let Some(previous) = previous_track_position
        {
            swept_laser_capsules(
                &center_offsets,
                previous,
                scale_at(frames_since_spawn - 1),
                position,
                scale_at(frames_since_spawn),
                facing,
            )
        } else {
            laser_world_capsules(
                &center_offsets,
                position,
                facing,
                scale_at(frames_since_spawn),
            )
        };
        if simulated_hit_frame.is_none() {
            match contact_this_frame(&hits, &hurts) {
                Ok(true) => simulated_hit_frame = Some(id),
                Ok(false) => {}
                Err(_) => pose_error = true,
            }
        }
        previous_track_position = Some(position);
    }

    // The diagnostic x-overlap on the frame immediately before the
    // recorded hit (this tool's own metric, not an engine call; see
    // `best_x_overlap`'s doc).
    let frame_before_overlap = {
        let target = hit_frame - 1;
        if let Some(&(id, position, _v, _o)) = track.frames.iter().find(|f| f.0 == target)
            && let Some(victim_post) = by_id.get(&id).and_then(|p| p[victim_slot])
            && let Some(victim) = victim_fighter(&victim_post)
            && let Ok(pose) = reconstruct_pose(&victim, victim_data)
            && let Ok(hurts) = victim_world_hurtboxes(victim_data, &pose)
        {
            let frames_since_spawn = id - track.spawn_frame;
            let prev = track.frames.iter().rev().find(|f| f.0 < id).map(|f| f.1);
            let facing = if let Some(p) = prev {
                if position[0] >= p[0] { 1.0 } else { -1.0 }
            } else {
                1.0
            };
            let hits = if let Some(p) = prev {
                swept_laser_capsules(
                    &center_offsets,
                    p,
                    scale_at(frames_since_spawn - 1),
                    position,
                    scale_at(frames_since_spawn),
                    facing,
                )
            } else {
                laser_world_capsules(
                    &center_offsets,
                    position,
                    facing,
                    scale_at(frames_since_spawn),
                )
            };
            best_x_overlap(&hits, &hurts)
        } else {
            None
        }
    };

    let delta = simulated_hit_frame.map(|s| s - hit_frame);
    EventOutcome {
        file: file_label.to_string(),
        item_id: track.id,
        spawn_frame: track.spawn_frame,
        shooter_port,
        victim_port,
        recorded_hit_frame: Some(hit_frame),
        recorded_victim_action: recorded_action,
        recorded_victim_action_name: recorded_action_name,
        simulated_hit_frame,
        delta_frames: delta,
        frame_before_x_overlap: frame_before_overlap,
        skip: if pose_error {
            Some(SkipReason::PoseError)
        } else {
            None
        },
    }
}

fn shooter_laser(data: &FighterData) -> Option<(f32, Vec<skirmish::game::data::Hitbox>, f32)> {
    let specials = data.specials.as_ref()?;
    let neutral = match specials {
        Specials::Fox { neutral, .. } | Specials::Falco { neutral, .. } => neutral.as_ref()?,
    };
    Some((
        neutral.attributes.speed,
        neutral.laser.hitboxes.clone(),
        neutral.laser.scale.unwrap_or(3.0),
    ))
}

fn median(mut values: Vec<f32>) -> Option<f32> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

/// Print the human-readable tables this tool's task asks for: file/coverage
/// counts, a per-skip-reason tally, a per-victim-action aggregate table
/// (count/exact/early/late/median frame-before-hit x-overlap), an overall
/// row, and per-event detail lines for the five victim actions the task
/// calls out by name (Wait, Walk, Dash, JumpSquat/KneeBend,
/// LandingFallSpecial).
pub fn print_report(report: &Report) {
    println!(
        "files considered: {}  eligible (version/stage/characters/pack pairing): {}  processed: {}  failed: {}",
        report.files_considered, report.files_eligible, report.files_processed, report.files_failed
    );
    println!("recorded Fox-laser items seen: {}", report.laser_items_seen);
    if !report.errors.is_empty() {
        println!("\nerrors:");
        for error in &report.errors {
            println!("  {error}");
        }
    }

    let mut skip_tally: HashMap<&str, usize> = HashMap::new();
    for event in &report.events {
        let key = match event.skip {
            Some(SkipReason::Reflected) => "reflected",
            Some(SkipReason::ShieldContact) => "shield_contact",
            Some(SkipReason::VictimIntangible) => "victim_intangible",
            Some(SkipReason::VictimActionUnsupported) => "victim_action_unsupported",
            Some(SkipReason::NoRecordedHit) => "no_recorded_hit",
            Some(SkipReason::PoseError) => "pose_error",
            None => "analyzed",
        };
        *skip_tally.entry(key).or_default() += 1;
    }
    println!("\nevent disposition:");
    let mut skip_rows: Vec<_> = skip_tally.into_iter().collect();
    skip_rows.sort();
    for (key, count) in &skip_rows {
        println!("  {key:<28} {count}");
    }

    if !report.unsupported_action_tally.is_empty() {
        println!("\nvictim actions with no pose mapping (skipped), by recorded Slippi state id:");
        let mut rows: Vec<_> = report.unsupported_action_tally.iter().collect();
        rows.sort();
        for (id, count) in rows {
            println!(
                "  state {id:<4} ({:<20}) {count} event(s)",
                action_display_name(*id)
            );
        }
    }

    let analyzed: Vec<&EventOutcome> = report.events.iter().filter(|e| e.skip.is_none()).collect();
    println!(
        "\nper-victim-action aggregate (analyzed events only, n={}):",
        analyzed.len()
    );
    println!(
        "  {:<24} {:>6} {:>6} {:>6} {:>6} {:>10}",
        "action", "n", "exact", "early", "late", "median_ovl"
    );
    let mut by_action: HashMap<u16, Vec<&EventOutcome>> = HashMap::new();
    for event in &analyzed {
        if let Some(action) = event.recorded_victim_action {
            by_action.entry(action).or_default().push(event);
        }
    }
    let mut action_ids: Vec<u16> = by_action.keys().copied().collect();
    action_ids.sort();
    for action in action_ids {
        let events = &by_action[&action];
        print_action_row(action_display_name(action), events);
    }
    println!();
    print_action_row("OVERALL", &analyzed);

    println!(
        "\nper-event detail for Wait(14)/Walk(15-17)/Dash(20)/JumpSquat(24)/LandingFallSpecial(43) victims:"
    );
    println!(
        "  {:<32} {:>8} {:>10} {:>10} {:>10} {:>7} {:>10}",
        "file", "item_id", "spawn_fr", "rec_hit_fr", "sim_hit_fr", "delta", "ovl_before"
    );
    for event in &analyzed {
        let Some(action) = event.recorded_victim_action else {
            continue;
        };
        if matches!(action, 14 | 15 | 16 | 17 | 20 | 24 | 43) {
            println!(
                "  {:<32} {:>8} {:>10} {:>10} {:>10} {:>7} {:>10}",
                truncate(&event.file, 32),
                event.item_id,
                event.spawn_frame,
                event
                    .recorded_hit_frame
                    .map(|f| f.to_string())
                    .unwrap_or_default(),
                event
                    .simulated_hit_frame
                    .map(|f| f.to_string())
                    .unwrap_or("none".to_string()),
                event
                    .delta_frames
                    .map(|d| d.to_string())
                    .unwrap_or_default(),
                event
                    .frame_before_x_overlap
                    .map(|o| format!("{o:.3}"))
                    .unwrap_or_default(),
            );
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s[..max].to_string()
    }
}

fn print_action_row(name: &str, events: &[&EventOutcome]) {
    let n = events.len();
    let exact = events.iter().filter(|e| e.delta_frames == Some(0)).count();
    let early = events
        .iter()
        .filter(|e| e.delta_frames.is_some_and(|d| d < 0))
        .count();
    let late = events
        .iter()
        .filter(|e| e.delta_frames.is_some_and(|d| d > 0))
        .count();
    let overlaps: Vec<f32> = events
        .iter()
        .filter_map(|e| e.frame_before_x_overlap)
        .collect();
    let median_overlap = median(overlaps);
    println!(
        "  {name:<24} {n:>6} {exact:>6} {early:>6} {late:>6} {:>10}",
        median_overlap
            .map(|m| format!("{m:.3}"))
            .unwrap_or_else(|| "n/a".to_string())
    );
}
