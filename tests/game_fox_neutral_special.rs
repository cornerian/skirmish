//! Fox/Falco neutral special (Blaster): the Start/Loop/End state machine
//! and the minimal generic projectile system it fires through. See
//! `docs/fox-neutral-special.md` for the full citation list and what
//! remains unmodeled.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_neutral_special.rs"]
mod neutral_special_resources;

use skirmish::game::{Action, BUTTON_B, Controller, Event, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    buttons: 0,
    stick: [0.0; 2],
    cstick: [0.0; 2],
    trigger: 0.0,
}; 2];

fn data() -> MatchData {
    neutral_special_resources::profile(conformance::data())
}

fn input(player: usize, controller: Controller) -> [Controller; 2] {
    let mut inputs = IDLE;
    inputs[player] = controller;
    inputs
}

fn press_b() -> Controller {
    Controller {
        buttons: BUTTON_B,
        ..Controller::default()
    }
}

fn spawned_this_frame(state: &skirmish::game::State, owner: usize) -> bool {
    state
        .events
        .iter()
        .any(|event| matches!(event, Event::ProjectileSpawned { owner: o, .. } if *o == owner))
}

fn hit_this_frame(state: &skirmish::game::State, owner: usize, victim: usize) -> bool {
    state.events.iter().any(
        |event| matches!(event, Event::ProjectileHit { owner: o, victim: v } if *o == owner && *v == victim),
    )
}

#[test]
fn none_keeps_b_inert() {
    let mut plain = conformance::data();
    plain.fighters[0].specials = None;
    let mut game = Match::new(plain, 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialNStart);
}

#[test]
fn strict_thresholds_gate_the_grounded_entry() {
    let mut game = Match::new(data(), 0).unwrap();
    // The fixture's own thresholds are [0.5, 0.5]; 0.5 itself must fail
    // (strict), matching the retained `neutral_input`/`ftCo_800D67C4`
    // boundary.
    let boundary = Controller {
        buttons: BUTTON_B,
        stick: [0.5, 0.0],
        ..Controller::default()
    };
    let state = game.step(input(0, boundary)).unwrap();
    assert_ne!(state.fighters[0].action, Action::SpecialNStart);

    let mut game = Match::new(data(), 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
}

#[test]
fn aerial_entry_selects_the_air_variant() {
    let mut resource = data();
    resource.stage.spawns[0][1] = 6.0;
    let mut game = Match::new(resource, 0).unwrap();
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirNStart);
}

#[test]
fn aerial_entry_preserves_velocity_but_grounded_entry_zeros_it() {
    // `ftFx_SpecialN_Enter` (ground) zeros `gr_vel`/`self_vel.{x,y,z}` right
    // after `ftFox_SpecialN_InitializeState`; `ftFx_SpecialAirN_Enter` (air)
    // does not touch velocity at all -- only `Fighter_ChangeMotionState`,
    // the shared `InitializeState` and the blaster spawn
    // (`ftfoxspecialn.c:246-284`). Confirmed against `fox-bf.slp`: P4's own
    // airborne Blaster press carries its existing falling-jump drift
    // straight through the transition (position continues the same smooth
    // deceleration trend both before and after, `docs/parity.md`).
    let mut resource = data();
    resource.stage.spawns[0][1] = 20.0;
    let mut game = Match::new(resource, 0).unwrap();
    // Let gravity build up a nonzero, non-round falling velocity first,
    // over two frames so the ordinary per-frame gravity delta is known.
    for _ in 0..4 {
        game.step(IDLE).unwrap();
    }
    let before = game.state().fighters[0].velocity;
    let falling = game.step(IDLE).unwrap().fighters[0].velocity;
    assert_ne!(falling, [0.0, 0.0]);
    let gravity_delta = falling[1] - before[1];
    assert_ne!(gravity_delta, 0.0);
    let state = game.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialAirNStart);
    // Velocity is untouched by the transition itself: only the ordinary
    // per-frame gravity this same frame's physics already applies, the
    // same delta as every prior falling frame -- not zeroed or overridden.
    assert_eq!(
        state.fighters[0].velocity,
        [falling[0], falling[1] + gravity_delta]
    );

    let mut grounded = Match::new(data(), 0).unwrap();
    let state = grounded.step(input(0, press_b())).unwrap();
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);
    assert_eq!(state.fighters[0].velocity, [0.0, 0.0]);
    assert_eq!(state.fighters[0].ground_velocity, 0.0);
}

#[test]
fn a_fresh_b_press_repeats_the_loop_while_no_press_ends_it() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // Two-frame Start pose (per the fixture): `ftFx_SpecialN_Enter`'s own
    // extra `ftAnim_8006EBA4` advance (`docs/validation.md`'s entry-advance
    // table) starts the entry frame's `action_frame` at 1 (2 after this
    // same step's generic increment), so the clip (indices 0..=1) already
    // runs out one idle frame later.
    let entered_loop = game.step(IDLE).unwrap();
    assert_eq!(entered_loop.fighters[0].action, Action::SpecialNLoop);
    assert!(spawned_this_frame(entered_loop, 0));

    // A fresh B press during Loop arms the repeat.
    game.step(input(0, press_b())).unwrap();
    let repeated = game.step(IDLE).unwrap();
    assert_eq!(repeated.fighters[0].action, Action::SpecialNLoop);
    assert!(
        spawned_this_frame(repeated, 0),
        "a repeated Loop cycle fires another shot"
    );
}

#[test]
fn no_repeat_press_ends_the_move_and_returns_to_wait() {
    let mut game = Match::new(data(), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut saw_loop = false;
    let mut saw_end = false;
    let mut reached_wait = false;
    for _ in 0..20 {
        let state = game.step(IDLE).unwrap();
        match state.fighters[0].action {
            Action::SpecialNLoop => saw_loop = true,
            Action::SpecialNEnd => saw_end = true,
            Action::Wait if saw_loop && saw_end => {
                reached_wait = true;
                break;
            }
            _ => {}
        }
    }
    assert!(saw_loop, "the move must enter Loop at least once");
    assert!(
        saw_end,
        "no repeat press must end the move through SpecialNEnd"
    );
    assert!(reached_wait, "SpecialNEnd must return to Wait");
}

#[test]
fn the_laser_travels_before_hitting_and_despawns_on_contact() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // The Start clip already runs out one idle frame later than a naive
    // frame count would suggest -- see
    // `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s own
    // comment.
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));
    assert_eq!(spawn_state.projectiles.len(), 1);
    let spawn_position = spawn_state.projectiles[0].position;

    let mut hit_frame = None;
    for _ in 0..10 {
        let state = game.step(IDLE).unwrap().clone();
        if hit_this_frame(&state, 0, 1) {
            hit_frame = Some(state);
            break;
        }
    }
    let hit_frame = hit_frame.expect("the laser must eventually hit fighter 1");
    assert!(hit_frame.fighters[1].percent > 0.0);
    assert!(hit_frame.projectiles.is_empty(), "no piercing");
    assert_ne!(
        spawn_position[0], hit_frame.fighters[1].position[0],
        "the hit happens after travel, not at the spawn position"
    );
    // `ftColl_8007925C` (the victim's own per-item hurtbox scan,
    // `ftcoll.c:1999-2270`) stages an item-inflicted hit onto the victim
    // alone; nothing in that path (or `Item_8026A294`'s own reaction to it)
    // reaches into the item's *owner* Fighter_GObj, unlike a direct
    // fighter-vs-fighter hitbox contact (`ftColl_800763C0`), which gives
    // both GObjs hitlag from the same contact -- a projectile's owner is a
    // separate GObj from the projectile itself, so it takes no hitlag from
    // its own shot connecting.
    assert_eq!(
        hit_frame.fighters[0].hitlag, 0.0,
        "a projectile's owner takes no hitlag from its own shot connecting"
    );
}

/// `drain_pending_shot`'s own doc has the full citation: the laser's real
/// spawn position is the `RThumbNb` hold joint's own world position, not
/// the ECB vertical midpoint this move fell back to before `specials.
/// neutral.laser.muzzle_bone` was exported. This pins the wiring (a
/// present `muzzle_bone` routes through the bone-based branch instead of
/// the ECB-midpoint fallback) against this fixture's own existing bone 1;
/// exact real-recording numbers (Fox's own bone `67`, offset `(0,
/// 1.2325000762939453, 4.263599872589111)`) are verified separately, by
/// re-measuring `fox-fd-2.slp`/`fox-fd-3.slp` against the real gameplay
/// export (`docs/fox-neutral-special.md`), not by a unit test against a
/// two-bone synthetic skeleton that cannot reproduce Fox's own animated
/// pose.
fn spawn_position_with_muzzle_bone(bone_translation_y: Option<f32>) -> [f32; 3] {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    if let Some(y) = bone_translation_y {
        resource.fighters[0].bones[1].translation = [0.0, y, 0.0];
        if let Some(skirmish::characters::Specials::Fox {
            neutral: Some(neutral),
            ..
        }) = &mut resource.fighters[0].specials
        {
            neutral.laser.muzzle_bone = Some(1);
        } else {
            panic!("fixture's own Fox neutral special must be present");
        }
    }
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));
    spawn_state.projectiles[0].position
}

#[test]
fn muzzle_bone_spawn_position_differs_from_the_ecb_midpoint_fallback() {
    let fallback = spawn_position_with_muzzle_bone(None);
    let bone_based = spawn_position_with_muzzle_bone(Some(20.0));
    assert!(fallback[0].is_finite() && fallback[1].is_finite());
    assert!(bone_based[0].is_finite() && bone_based[1].is_finite());
    // A present `muzzle_bone` must route through the bone-based branch, not
    // the ECB-midpoint fallback: load-bearing evidence that
    // `drain_pending_shot` actually reads `laser.muzzle_bone`/the fighter's
    // own pose, not a coincidence of the fixture's own fixed ECB. (This
    // fixture's own generic pose fallback does not vary a bone's *own*
    // world position with its `bones[_].translation` the way a real
    // animated skeleton would -- exact real numbers, Fox's own bone `67`
    // against a real animated pose, are verified separately by
    // re-measuring `fox-fd-2.slp`/`fox-fd-3.slp`, not by this unit test.)
    assert_ne!(
        fallback[1], bone_based[1],
        "a present muzzle_bone must be used for the spawn position, not the ECB \
         midpoint fallback"
    );
}

/// Pins `drain_pending_shot`'s own literal-order `transform_point` call
/// against a rotated (not just translated) muzzle bone, where a wrongly
/// swapped offset and the correct, decomp-literal one produce clearly
/// different -- and independently hand-derivable -- world positions,
/// unlike the translation-only fixture above (whose identity-rotation
/// bone cannot distinguish the two: swapping two offset components that
/// both pass through an identity rotation unchanged changes nothing a
/// Y-only assertion would catch).
///
/// `simulation::pose` reads a fired frame's own bones from `data.attack`'s
/// sampled `AttackFrame.bones` (this fixture's `start`/`loop_phase`
/// ground/air phases all embed the same invented two-bone pose, per
/// `fox_neutral_special::profile`'s own doc), not from `FighterData.bones`
/// -- so every phase/frame's own bone 1 is mutated uniformly here, not
/// just the base skeleton the fixture above touches (which this move's own
/// pose selection never actually reads). With bone 1 parented directly to
/// the identity bone 0 and its own translation left at zero, `bones::srt`'s
/// Euler composition (row0 = `[cos_y, 0, sin_y, 0]`, row1 = `[0, 1, 0, 0]`,
/// row2 = `[-sin_y, 0, cos_y, 0]`) at rotation `(0, pi/2, 0)` reduces to
/// `world_offset = (local.z, local.y, -local.x)`, independent of the
/// fighter's own position/facing root (a pure rotation contributes no
/// translation). Comparing against the same fixture with bone 1 left at
/// its identity rotation (`world_offset = local` unchanged) isolates
/// exactly that rotation's own contribution to the spawn x-coordinate,
/// canceling out the fighter's own position and the projectile's own
/// already-applied first-frame velocity (identical in both runs): the
/// decomp-literal offset `(0, 1.2325000762939453, 4.263599872589111)`
/// (`ftfoxspecialn.c:32-51`) must add its `z` term (`4.2636`) to world x
/// through this rotation, not its `x` term (`0`, what a wrongly swapped
/// `(4.2636, 1.2325, 0)` offset would add instead -- see
/// `drain_pending_shot`'s own doc for the full decomp citation,
/// `lb_8000B1CC`'s literal, unpermuted `MTXMultVec`, for why no axis swap
/// belongs here at all).
#[test]
fn muzzle_bone_rotation_proves_the_offset_is_not_axis_swapped() {
    let spawn_x = |rotation_y: f32| {
        let mut resource = data();
        resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
        if let Some(skirmish::characters::Specials::Fox {
            neutral: Some(neutral),
            ..
        }) = &mut resource.fighters[0].specials
        {
            neutral.laser.muzzle_bone = Some(1);
            for attack in [
                &mut neutral.start.ground,
                &mut neutral.start.air,
                &mut neutral.loop_phase.ground,
                &mut neutral.loop_phase.air,
            ] {
                for frame in &mut attack.frames {
                    frame.bones[1].rotation = [0.0, rotation_y, 0.0];
                }
            }
        } else {
            panic!("fixture's own Fox neutral special must be present");
        }
        let mut game = Match::new(resource, 0).unwrap();
        game.step(input(0, press_b())).unwrap();
        let spawn_state = game.step(IDLE).unwrap().clone();
        assert!(spawned_this_frame(&spawn_state, 0));
        spawn_state.projectiles[0].position[0]
    };
    let identity_x = spawn_x(0.0);
    let rotated_x = spawn_x(core::f32::consts::FRAC_PI_2);
    #[allow(clippy::excessive_precision)]
    let expected_delta = 4.263_599_872_589_111;
    assert!(
        (rotated_x - identity_x - expected_delta).abs() < 1e-3,
        "a pi/2 y-rotation must add the offset's own z-component (4.2636) to world \
         x, not its x-component (0), which a swapped offset would give instead: \
         identity_x={identity_x}, rotated_x={rotated_x}, delta={}",
        rotated_x - identity_x
    );
}

/// `Laser::scale`'s own doc has the full citation: a laser's own hitbox
/// offsets grow from `0.0` toward the pack's `scale` cap over its first
/// few frames (`Item_UpdateRayAnimation`), not the full offset immediately.
#[test]
fn ray_scale_grows_from_zero_and_caps_at_the_pack_value() {
    let mut resource = data();
    // Far enough apart that the shot (speed 7/frame) outlives its own
    // ~5-frame growth window before it could ever reach fighter 1, but
    // still inside this fixture's own +/-100 floor/blast bounds.
    resource.stage.spawns = [[0.0, 0.0], [60.0, 0.0]];
    let cap = 3.0;
    if let Some(skirmish::characters::Specials::Fox {
        neutral: Some(neutral),
        ..
    }) = &mut resource.fighters[0].specials
    {
        neutral.laser.scale = Some(cap);
    } else {
        panic!("fixture's own Fox neutral special must be present");
    }
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));
    let first = spawn_state.projectiles[0]
        .scale
        .expect("scale growth must be modeled when the pack supplies laser.scale");
    assert!(
        first.current > 0.0 && first.current < cap,
        "the spawn frame's own first Anim increment must be strictly between 0 and \
         the cap: got {}",
        first.current
    );
    let mut last = first.current;
    let mut reached_cap = false;
    for _ in 0..20 {
        let state = game.step(IDLE).unwrap();
        let Some(projectile) = state.projectiles.first() else {
            break;
        };
        let current = projectile
            .scale
            .expect("scale stays modeled for this shot")
            .current;
        assert!(
            current >= last,
            "scale must grow monotonically: {current} after {last}"
        );
        assert!(current <= cap, "scale must never exceed the pack's own cap");
        last = current;
        if current == cap {
            reached_cap = true;
            break;
        }
    }
    assert!(
        reached_cap,
        "scale must reach the cap within its own growth window"
    );
}

#[test]
fn staling_reduces_repeated_laser_damage() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    resource.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.5, 0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
        debug_bypass: false,
    });
    for fighter in &mut resource.fighters {
        fighter.jab.move_id = Some(10);
    }
    let mut game = Match::new(resource, 0).unwrap();
    let mut percents = Vec::new();
    for _ in 0..3 {
        game.step(input(0, press_b())).unwrap();
        game.step(IDLE).unwrap();
        game.step(IDLE).unwrap();
        for _ in 0..10 {
            let state = game.step(IDLE).unwrap().clone();
            if hit_this_frame(&state, 0, 1) {
                percents.push(state.fighters[1].percent);
                break;
            }
        }
        // Wait out End before firing again.
        for _ in 0..6 {
            game.step(IDLE).unwrap();
        }
    }
    assert_eq!(percents.len(), 3);
    let first_hit = percents[0];
    let second_hit = percents[1] - percents[0];
    let third_hit = percents[2] - percents[1];
    assert!(
        second_hit < first_hit || third_hit < first_hit,
        "a repeated laser hit must stale toward less damage: {percents:?}"
    );
}

#[test]
fn leaving_the_ground_mid_move_falls_through_to_ordinary_fall() {
    let mut resource = data();
    resource.stage.spawns[0] = [95.0, 0.0];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut walked_off = None;
    for _ in 0..40 {
        let state = game
            .step(input(
                0,
                Controller {
                    stick: [1.0, 0.0],
                    ..Controller::default()
                },
            ))
            .unwrap();
        if !state.fighters[0].grounded {
            walked_off = Some(state.fighters[0].action);
            break;
        }
    }
    // Blaster's own grounded phases have no ground->air conversion of
    // their own (`ft_80083F88`): leaving the ground always falls through
    // to the generic `Action::Fall` fallback, never `SpecialAirN*`.
    assert_eq!(walked_off, Some(Action::Fall));
}

#[test]
fn slippi_ids_cover_all_six_phases() {
    use skirmish::characters;
    for (action, state, animation) in [
        (Action::SpecialNStart, 341, 295),
        (Action::SpecialNLoop, 342, 296),
        (Action::SpecialNEnd, 343, 297),
        (Action::SpecialAirNStart, 344, 298),
        (Action::SpecialAirNLoop, 345, 299),
        (Action::SpecialAirNEnd, 346, 300),
    ] {
        assert_eq!(
            characters::slippi_ids(Some(2), action),
            Some((state, animation))
        );
    }
}

#[test]
fn a_terrain_line_despawns_the_laser_before_it_reaches_the_far_fighter() {
    // A real stage-line raycast (`it_8026E9A4`), not merely the stage's own
    // outer bounding box: place a wall between the two fighters, well
    // inside the blast zone, and confirm the laser despawns against it
    // instead of reaching fighter 1 (see `game::projectile::step`'s own
    // terrain-despawn citation).
    use skirmish::collision::stage;
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    resource.stage.geometry = Some(skirmish::game::data::StageGeometry {
        lines: vec![
            stage::Line {
                start: [-100.0, 0.0],
                end: [100.0, 0.0],
                flags: stage::ENABLED | stage::FLOOR,
                material_flags: stage::LEDGE as u16,
                ..Default::default()
            },
            // A vertical wall at x=7, between the two fighters, spanning
            // well past the laser's own flight height.
            stage::Line {
                start: [7.0, -5.0],
                end: [7.0, 20.0],
                flags: stage::ENABLED | stage::LEFT_WALL,
                ..Default::default()
            },
        ],
        joints: vec![stage::Joint {
            flags: stage::ENABLED,
            bounds_min: [-100.0, -5.0],
            bounds_max: [100.0, 20.0],
            floor: 0..1,
            left_wall: 1..2,
            ..Default::default()
        }],
    });
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    // The Start clip already runs out one idle frame later than a naive
    // frame count would suggest -- see `docs/validation.md`'s entry-advance
    // table and `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s
    // own comment above.
    let spawn_state = game.step(IDLE).unwrap().clone();
    assert!(spawned_this_frame(&spawn_state, 0));

    let mut despawned_at_wall = false;
    for _ in 0..10 {
        let state = game.step(IDLE).unwrap().clone();
        assert!(
            !hit_this_frame(&state, 0, 1),
            "the wall must stop the laser before it ever reaches fighter 1"
        );
        if state.projectiles.is_empty() {
            despawned_at_wall = true;
            break;
        }
        assert!(
            state.projectiles[0].position[0] < 7.5,
            "the laser must not pass through the wall: {:?}",
            state.projectiles[0].position
        );
    }
    assert!(
        despawned_at_wall,
        "the laser must despawn against the wall, not merely at the blast zone"
    );
}

/// Extends the fixture's own `script`-driven coverage: a synthetic
/// `NeutralSpecial::script` shaped like this batch's own exported
/// `fighters/fox.json` real timings (Start 7 frames/`cmd_vars[0]` at
/// frame 4, Loop 10 frames/`cmd_vars[2]` at frame 5, ground and air alike),
/// grafted onto the shared fixture by repeating its own last pose to reach
/// those lengths (the fixture's bone/hitbox data is otherwise irrelevant to
/// these tests). Exercises the full engine end to end, distinct from
/// `tests/neutral_special_differential.rs`'s own oracle-only trace.
mod script_resources {
    use skirmish::characters::{
        Specials,
        fox::{
            neutral::NeutralScript,
            side::{ScriptFrames, ScriptPhase},
        },
    };
    use skirmish::game::data::{Attack, MatchData};

    fn extend(attack: &mut Attack, len: usize) {
        let last = attack
            .frames
            .last()
            .expect("fixture always has a pose")
            .clone();
        while attack.frames.len() < len {
            attack.frames.push(last.clone());
        }
    }

    fn cmd_var_table(len: usize, slot: usize, set_frame: usize, value: u32) -> ScriptFrames {
        let mut cmd_vars = vec![[None; 4]; len];
        for row in &mut cmd_vars[set_frame..] {
            row[slot] = Some(value);
        }
        ScriptFrames {
            cmd_vars,
            allow_interrupt: vec![false; len],
        }
    }

    pub fn profile(mut data: MatchData) -> MatchData {
        data = super::neutral_special_resources::profile(data);
        for fighter in &mut data.fighters {
            let Some(Specials::Fox {
                neutral: Some(neutral),
                ..
            }) = &mut fighter.specials
            else {
                panic!("neutral_special_resources::profile always installs Fox's neutral");
            };
            extend(&mut neutral.start.ground, 7);
            extend(&mut neutral.start.air, 7);
            extend(&mut neutral.loop_phase.ground, 10);
            extend(&mut neutral.loop_phase.air, 10);
            neutral.script = Some(Box::new(NeutralScript {
                start: ScriptPhase {
                    ground: cmd_var_table(7, 0, 4, 1),
                    air: cmd_var_table(7, 0, 4, 1),
                },
                loop_phase: ScriptPhase {
                    ground: cmd_var_table(10, 2, 5, 1),
                    air: cmd_var_table(10, 2, 5, 1),
                },
                end: ScriptPhase {
                    ground: cmd_var_table(neutral.end.ground.frames.len(), 1, 1, 2),
                    air: cmd_var_table(neutral.end.air.frames.len(), 1, 1, 2),
                },
            }));
        }
        data
    }
}

/// A fresh B press during Start's own arm window (`cmd_vars[0]` sets at
/// frame 4) arms the repeat *before* Loop is ever entered -- behavior the
/// pre-script approximation could not produce at all (it hard-coded Start
/// as never-armed). The entry press itself is only "fresh" on entry, so
/// this releases B (`IDLE` steps) before pressing again on the exact tick
/// `action_frame` reaches 4.
#[test]
fn a_press_during_starts_own_arm_window_arms_before_loop_is_entered() {
    let mut game = Match::new(script_resources::profile(data()), 0).unwrap();
    // `action_frame` after each tick (the extra-advance hack starts it at 1,
    // then this port's generic per-tick increment runs once more the same
    // tick -- see `a_fresh_b_press_repeats_the_loop_while_no_press_ends_it`'s
    // own comment): entry -> 2, then +1 per further tick. A fresh press
    // lands on `action_frame == 4` (the script's own arm frame) when issued
    // on the third tick after entry.
    game.step(input(0, press_b())).unwrap(); // entry, action_frame -> 2
    game.step(IDLE).unwrap(); // action_frame 2 processed -> 3
    game.step(IDLE).unwrap(); // action_frame 3 processed -> 4
    let state = game.step(input(0, press_b())).unwrap(); // action_frame 4 processed: fresh press, arms
    assert_eq!(state.fighters[0].action, Action::SpecialNStart);

    // Run out the rest of Start and confirm the move enters Loop and
    // repeats *without any further press*, proving the arm from Start's own
    // window survived the Start->Loop transition. `Action::SpecialNLoop`
    // reports identically for every pass, so a repeat is detected by its
    // own `action_frame` resetting back down mid-Loop (not by any action
    // change) -- watch for a low frame following an already-seen high one,
    // before `SpecialNEnd` (a genuine end, no repeat) is ever reached.
    // (This loop only requires monotonic *eventual* progress, not a fixed
    // tick schedule, since a hit landing this fighter's own shield/damage
    // side could still perturb the exact tick count in other fixtures; a
    // projectile's own owner no longer takes hitlag from its own shot
    // connecting -- `game::damage::apply_hit`'s `attacker_takes_hitlag`.)
    let mut saw_high_frame_in_loop = false;
    let mut saw_repeat = false;
    for _ in 0..40 {
        let state = game.step(IDLE).unwrap();
        let action_frame = state.fighters[0].action_frame;
        match state.fighters[0].action {
            Action::SpecialNLoop if action_frame >= 8 => saw_high_frame_in_loop = true,
            Action::SpecialNLoop if action_frame <= 1 && saw_high_frame_in_loop => {
                saw_repeat = true;
                break;
            }
            Action::SpecialNEnd => break,
            _ => {}
        }
    }
    assert!(
        saw_repeat,
        "the Start-window arm must survive into Loop and repeat it without a fresh press"
    );
}

/// The laser fires on the script's own frame-5 of Loop, not Loop's entry
/// frame (the pre-script approximation's timing).
#[test]
fn the_shot_fires_on_the_scripts_own_frame_not_loop_entry() {
    let mut game = Match::new(script_resources::profile(data()), 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    let mut entered_loop_tick = None;
    let mut fired_tick = None;
    for tick in 0..20 {
        let state = game.step(IDLE).unwrap();
        if entered_loop_tick.is_none() && state.fighters[0].action == Action::SpecialNLoop {
            entered_loop_tick = Some(tick);
        }
        if fired_tick.is_none() && spawned_this_frame(state, 0) {
            fired_tick = Some(tick);
        }
        if fired_tick.is_some() {
            break;
        }
    }
    let entered_loop_tick = entered_loop_tick.expect("the move must enter Loop");
    let fired_tick = fired_tick.expect("the shot must fire");
    assert!(
        fired_tick > entered_loop_tick,
        "entered Loop at tick {entered_loop_tick}, fired at tick {fired_tick}: \
         the script-driven fire must lag Loop's own entry, not coincide with it"
    );
}

/// `NeutralSpecial::validate` (and `SideSpecial::validate`) reject a
/// `script` whose per-frame vectors don't have exactly as many entries as
/// their own phase's own pose count -- checked before a match is even
/// constructed, matching the established `invalid_*_resources_are_
/// rejected_without_constructing_a_match` convention elsewhere in this
/// test suite.
#[test]
fn mismatched_script_frame_counts_are_rejected() {
    use skirmish::characters::Specials;
    use skirmish::game::Match;

    let mut resource = script_resources::profile(data());
    for fighter in &mut resource.fighters {
        let Some(Specials::Fox {
            neutral: Some(neutral),
            ..
        }) = &mut fighter.specials
        else {
            panic!("script_resources::profile always installs Fox's neutral script");
        };
        // Drop one row from Loop-ground's own `cmd_vars`, breaking parity
        // with `loop_phase.ground.frames.len()`.
        neutral
            .script
            .as_mut()
            .expect("script_resources::profile always installs a script")
            .loop_phase
            .ground
            .cmd_vars
            .pop();
    }
    assert!(Match::new(resource, 0).is_err());
}

#[test]
fn checkpoint_round_trip_preserves_the_move_and_in_flight_projectiles() {
    let mut resource = data();
    resource.stage.spawns = [[0.0, 0.0], [15.0, 0.0]];
    let mut game = Match::new(resource, 0).unwrap();
    game.step(input(0, press_b())).unwrap();
    game.step(IDLE).unwrap();
    game.step(IDLE).unwrap();
    let checkpoint = game.checkpoint();
    let expected = game.step(IDLE).unwrap().clone();
    game.restore_checkpoint(&checkpoint).unwrap();
    let actual = game.step(IDLE).unwrap().clone();
    assert_eq!(actual, expected);
}
