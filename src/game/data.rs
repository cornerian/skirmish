//! Native, explicit data for the experimental two-player match slice.
//! Frame samples are physics poses, not rendering assets or HSD animation bytecode.
use crate::{
    collision::{bones, ecb, stage},
    fighter::{Attributes, combat},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MatchData {
    pub schema: u32,
    /// Every current profile is experimental; this is not a compatibility toggle.
    pub profile: Profile,
    pub provenance: String,
    pub stage: Stage,
    pub fighters: [FighterData; 2],
    pub rules: Rules,
    /// Per-player match settings (`Player_GetHandicap`,
    /// `docs/grab-escape-timer.md`), indexed like `fighters`. `None` keeps
    /// every player at the handicap-off default of 9, matching every
    /// fixture that predates this field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub players: Option<[PlayerSettings; 2]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlayerSettings {
    /// `Player_GetHandicap`/`Player_SetHandicap` (`pl/player.c:855-870`):
    /// 1..=9, where 9 is both the maximum slider value and the value every
    /// slot holds whenever the handicap rule itself is off
    /// (`mn/mncharsel.c:4290`, `gm/gm_1601.c:3502`, `gm/gmmain_lib.c:833`).
    #[serde(default = "default_handicap")]
    pub handicap: u8,
}

fn default_handicap() -> u8 {
    9
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Profile {
    IntegrationFixture,
    Experimental,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stage {
    pub name: String,
    /// Compact floor used when explicit geometry is absent.
    pub floor: Floor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<StageGeometry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub motion: Option<super::stage_motion::Rules>,
    /// Left, right, bottom, top. Top eligibility is configured in `Rules`.
    pub blast: [f32; 4],
    pub spawns: [[f32; 2]; 2],
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StageGeometry {
    pub lines: Vec<stage::Line>,
    pub joints: Vec<stage::Joint>,
}

/// Environmental collision samples are physics data, independent of hurtboxes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CollisionBox {
    Fixed {
        source: ecb::FixedSource,
    },
    Bones {
        indices: [usize; 6],
        parameters: ecb::JointParameters,
        /// Unused: `mpColl_LoadECB_inline`'s flags argument varies per
        /// collision entry point in the source (`mpColl_LoadECB_inline(coll,
        /// 6)` airborne, `(coll, 5)` grounded, ...), not per resource pack.
        /// `game::collision::sample` computes the authoritative value from
        /// the collision path itself (see `docs/ecb-load-flags.md`); this
        /// field is kept only so existing/exported packs still deserialize.
        flags: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Floor {
    pub left: f32,
    pub right: f32,
    pub y: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staling: Option<crate::fighter::stale::Rules>,
    pub stocks: u8,
    pub countdown_frames: u32,
    pub time_limit_frames: u32,
    pub respawn_frames: u32,
    pub respawn_invincibility_frames: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebirth: Option<super::rebirth::Rules>,
    /// The match-start warp-in (Entry/EntryStart/EntryEnd, `docs/
    /// match-start.md`). `None` keeps every fighter spawning directly into
    /// `Action::Fall`, unchanged from before this resource existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<super::entry::EntryRules>,
    pub friction_above_walk: f32,
    pub walk_accel_taper_gain: f32,
    pub fast_fall_threshold: f32,
    /// `ftCommonData+0x8C` (`ftcommon.c:498-499`, `ftCommon_CheckFallFast`):
    /// the stick-timer window `fighter.locomotion.tilt_y_age` must stay
    /// under for a fresh down-press to trigger fast-fall. `None` keeps the
    /// previous, approximate `previous_input`-edge heuristic (`game::
    /// simulation::move_fighter`) for packs that don't export this constant
    /// yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fast_fall_window: Option<u32>,
    pub knockback_decay: f32,
    pub knockback_speed: f32,
    pub hitstun_scale: f32,
    /// Common-data x4F0: airborne top KOs require upward knockback strictly
    /// above this value. None retains the original synthetic fixture's rule.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_ko_min_knockback: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub death: Option<super::death::Rules>,
    pub knockback: KnockbackData,
    pub hitlag: HitlagData,
    pub damage: super::damage::CombatRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<super::shield::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clank: Option<super::clank::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge: Option<super::nudge::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grab: Option<super::grab::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<super::ledge::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_jump: Option<super::wall_jump::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<super::escape::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_air: Option<super::escape_air::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specials: Option<crate::characters::fox::side::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilt: Option<super::tilt::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smash: Option<super::smash::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<super::dash::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge: Option<super::edge::Rules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk: Option<super::locomotion::WalkRules>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockbackData {
    pub weight_scale: f32,
    pub weight_base: f32,
    pub maximum: f32,
    pub percent_scale: f32,
    pub damage_percent_scale: f32,
    pub fixed_damage: f32,
    pub growth_scale: f32,
    pub growth_base: f32,
}

impl KnockbackData {
    pub(crate) fn physics(&self) -> combat::KnockbackRules {
        combat::KnockbackRules {
            weight_scale: self.weight_scale,
            weight_base: self.weight_base,
            maximum: self.maximum,
            percent_scale: self.percent_scale,
            damage_percent_scale: self.damage_percent_scale,
            fixed_damage: self.fixed_damage,
            growth_scale: self.growth_scale,
            growth_base: self.growth_base,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitlagData {
    pub damage_scale: f32,
    pub base: f32,
    pub crouch_multiplier: f32,
}

impl HitlagData {
    pub(crate) fn physics(&self) -> combat::HitlagRules {
        combat::HitlagRules {
            damage_scale: self.damage_scale,
            base: self.base,
            crouch_multiplier: self.crouch_multiplier,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FighterData {
    /// Optional Luau behavior program for this fighter. Programs are embedded
    /// in the native match resource so replay hashes and checkpoints remain
    /// independent of the host filesystem.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub script: Option<super::script::Program>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rebound: Option<super::clank::Animation>,
    pub name: String,
    pub movement: MovementData,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locomotion: Option<super::locomotion::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub armor: Option<super::damage::Armor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub floor_tech: Option<super::damage::FloorTechAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knockdown: Option<super::damage::KnockdownAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_response: Option<super::damage::SurfaceResponseAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface_tech: Option<super::damage::SurfaceTechAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_jump: Option<super::wall_jump::Attributes>,
    /// `co_attrs.trophy_scale` (`ft/types.h:748`, `ftCo_DatAttrs` +0x110).
    /// Read only at the Entry -> EntryStart transition
    /// (`rules.entry.is_some()`); absent contributes no amplitude (`0.0`),
    /// matching an unset optional entry-animation source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trophy_scale: Option<f32>,
    /// The EntryStart figatree's own frame count, distinct from
    /// `rules.entry`'s action-duration timers (`docs/match-start.md`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<super::entry::EntryAnimation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub damage_poses: Option<super::damage::DamagePoseAttributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shield: Option<super::shield::Attributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nudge: Option<super::nudge::Attributes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grab: Option<super::grab::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ledge: Option<super::ledge::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specials: Option<crate::characters::Specials>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape: Option<super::escape::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escape_air: Option<super::escape_air::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle: Option<super::idle::IdleAnimations>,
    pub weight: f32,
    pub collision_box: CollisionBox,
    pub bones: Vec<Bone>,
    /// Per-frame physics poses for movement (non-attack) actions, keyed by
    /// the sub-motion they come from (`docs/movement-poses.md`). Absent
    /// entirely, or absent for a given field, keeps the pre-batch behavior
    /// for that action: the static rest pose (`bones` above) throughout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement_poses: Option<MovementPoses>,
    pub hurtboxes: Vec<Hurtbox>,
    pub jab: Attack,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aerials: Option<super::aerial::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tilts: Option<super::tilt::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smashes: Option<super::smash::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jab_combo: Option<super::jab::Parameters>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash_attack: Option<super::dash::DashAttack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub teeter: Option<super::edge::Teeter>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taunt: Option<super::taunt::Taunt>,
}

impl FighterData {
    pub(crate) fn attack(
        &self,
        action: super::Action,
        prone: Option<super::damage::ProneOrientation>,
        slow_ledge: bool,
    ) -> Option<&Attack> {
        if action == super::Action::Jab {
            return Some(&self.jab);
        }
        if super::tilt::owns_action(action) {
            return super::tilt::attack(self.tilts.as_ref()?, action);
        }
        if super::smash::owns_action(action) {
            return super::smash::attack(self.smashes.as_ref()?, action);
        }
        if super::jab::owns_action(action) {
            return super::jab::attack(self.jab_combo.as_ref()?, action);
        }
        if super::dash::owns_action(action) {
            return Some(super::dash::attack(self.dash_attack.as_ref()?));
        }
        if action == super::Action::CliffAttack {
            return Some(super::ledge::attack(self.ledge.as_ref()?, slow_ledge));
        }
        if action == super::Action::DownAttack {
            return Some(&self.knockdown.as_ref()?.variant(prone)?.attack);
        }
        if let Some(attack) = crate::characters::common::attack(action, self) {
            return Some(attack);
        }
        let index = super::aerial::attack_index(action)?;
        Some(&self.aerials.as_ref()?.moves[index].attack)
    }
}

/// Per-frame physics poses for movement (non-attack) actions
/// (`docs/movement-poses.md`), one field per sub-motion, laid out exactly
/// like `Attack.frames[i].bones`: a `Vec<Bone>` per animation frame, same
/// bone count and topology as `FighterData.bones` (validated by
/// `game::validation::validate_movement_poses`, reusing
/// `validate_animation_pose`). Every field is independently optional: a
/// sub-motion this pack does not (yet) carry keeps `simulation::pose`'s
/// static rest-pose fallback for the actions it would have covered, the
/// same convention `landing_poses`/`AttackFrame` already establish for a
/// missing resource. Bone 0 (the root joint)'s translation is `[0.0; 3]` in
/// every supplied frame, exactly like `AttackFrame.bones[0]`: TransN root
/// motion is applied to `Fighter.position` separately (root_translations,
/// or a profile's own per-frame delta) and must not be baked into the pose
/// itself, or `simulation::pose`'s `evaluate_with_root` would apply it
/// twice.
///
/// Field names match the `ftCo_Submotion` id each one samples (verify
/// against `src/melee/ft/kinds/ftCommon/forward.h`): `wait` (Wait1_0 only;
/// every other idle sub-motion this codebase's `idle` profile can cycle to
/// keeps the rest pose, since it has no track of its own here). `walk_slow`/
/// `walk_middle`/`walk_fast` (WalkSlow/Middle/Fast). `turn` (Turn), `turn_run`
/// (TurnRun), `dash` (Dash), `run` (Run), `run_brake` (RunBrake), `knee_bend`
/// (KneeBend, i.e. `Action::JumpSquat`). `jump_f`/`jump_b` (JumpF/JumpB),
/// `jump_aerial_f`/`jump_aerial_b` (JumpAerialF/B). `fall`/`fall_f`/`fall_b`
/// and `fall_aerial`/`fall_aerial_f`/`fall_aerial_b` (Fall/FallF/FallB and
/// their FallAerial counterparts): `ftCo_Fall_Anim_Inner`'s continuous
/// air-drift blend between the neutral/forward/backward figatrees
/// (`ftCo_Fall.c:110-172`) is not modeled by this batch, so only `fall`/
/// `fall_aerial` (the neutral track) is ever selected; the F/B fields are
/// validated if supplied but reserved for a future blend batch.
/// `fall_special`/`fall_special_f`/`fall_special_b` (FallSpecial and its
/// animation-only blend, same caveat). `landing`/`landing_fall_special`
/// (the ordinary and FallSpecial Landing). `squat`/`squat_wait`/`squat_rv`
/// (Squat/SquatWait/SquatRv). `pass` (Pass). `ottotto`/`ottotto_wait`
/// (Ottotto/OttottoWait). `entry_start` (EntryStart's own figatree, distinct
/// from `EntryRules`'s action-duration timers, `docs/match-start.md`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementPoses {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wait: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_slow: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_middle: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_fast: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_run: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_brake: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knee_bend: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_f: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_b: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_aerial_f: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jump_aerial_b: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_f: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_b: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_aerial: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_aerial_f: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_aerial_b: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_special: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_special_f: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fall_special_b: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub landing_fall_special: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squat: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squat_wait: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squat_rv: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pass: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ottotto: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ottotto_wait: Option<Vec<Vec<Bone>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_start: Option<Vec<Vec<Bone>>>,
}

impl MovementPoses {
    /// Every field paired with its own name, for validation and for
    /// `game::movement::pose`'s selection (kept in one place so a new field
    /// only needs to be added here once).
    pub(crate) fn fields(&self) -> [(&'static str, Option<&Vec<Vec<Bone>>>); 32] {
        [
            ("wait", self.wait.as_ref()),
            ("walk_slow", self.walk_slow.as_ref()),
            ("walk_middle", self.walk_middle.as_ref()),
            ("walk_fast", self.walk_fast.as_ref()),
            ("turn", self.turn.as_ref()),
            ("turn_run", self.turn_run.as_ref()),
            ("dash", self.dash.as_ref()),
            ("run", self.run.as_ref()),
            ("run_brake", self.run_brake.as_ref()),
            ("knee_bend", self.knee_bend.as_ref()),
            ("jump_f", self.jump_f.as_ref()),
            ("jump_b", self.jump_b.as_ref()),
            ("jump_aerial_f", self.jump_aerial_f.as_ref()),
            ("jump_aerial_b", self.jump_aerial_b.as_ref()),
            ("fall", self.fall.as_ref()),
            ("fall_f", self.fall_f.as_ref()),
            ("fall_b", self.fall_b.as_ref()),
            ("fall_aerial", self.fall_aerial.as_ref()),
            ("fall_aerial_f", self.fall_aerial_f.as_ref()),
            ("fall_aerial_b", self.fall_aerial_b.as_ref()),
            ("fall_special", self.fall_special.as_ref()),
            ("fall_special_f", self.fall_special_f.as_ref()),
            ("fall_special_b", self.fall_special_b.as_ref()),
            ("landing", self.landing.as_ref()),
            ("landing_fall_special", self.landing_fall_special.as_ref()),
            ("squat", self.squat.as_ref()),
            ("squat_wait", self.squat_wait.as_ref()),
            ("squat_rv", self.squat_rv.as_ref()),
            ("pass", self.pass.as_ref()),
            ("ottotto", self.ottotto.as_ref()),
            ("ottotto_wait", self.ottotto_wait.as_ref()),
            ("entry_start", self.entry_start.as_ref()),
        ]
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MovementData {
    pub ground_max_horizontal_velocity: f32,
    pub air_max_horizontal_velocity: f32,
    pub air_drift_stick_mul: f32,
    pub aerial_drift_base: f32,
    pub air_drift_max: f32,
    pub aerial_friction: f32,
    pub gravity: f32,
    pub terminal_velocity: f32,
    pub fast_fall_velocity: f32,
    pub ground_friction: f32,
    pub walk_acceleration_mul: f32,
    pub walk_acceleration_base: f32,
    pub walk_max_velocity: f32,
    pub jump_startup_frames: u32,
    pub jump_vertical_velocity: f32,
    pub short_hop_vertical_velocity: f32,
    pub jump_horizontal_velocity: f32,
    pub jump_horizontal_max: f32,
    pub jump_momentum_multiplier: f32,
    pub landing_frames: u32,
    /// `ftCo_DatAttrs.normal_landing_lag` (fp+1F4): the ordinary Landing
    /// interrupt window floor consulted by `ftCo_Landing_IASA`. `None` keeps
    /// a chainless Landing that never reaches the Wait chain.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_landing_lag: Option<f32>,
    /// Paired with `Rules.walk`. Absent keeps a single Walk state (Slippi
    /// 15) with an integer `action_frame`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walk_animation: Option<super::locomotion::WalkAnimation>,
    /// Unlike `walk_animation`, not paired with any `Rules` entry: Run has
    /// no kind-selection thresholds, just a figatree length and animation
    /// scaling. Absent keeps Run's pre-batch integer `action_frame` and
    /// rate 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_animation: Option<super::locomotion::RunAnimation>,
}

impl MovementData {
    pub(crate) fn physics(&self) -> Attributes {
        Attributes {
            ground_max_horizontal_velocity: self.ground_max_horizontal_velocity,
            air_max_horizontal_velocity: self.air_max_horizontal_velocity,
            air_drift_stick_mul: self.air_drift_stick_mul,
            aerial_drift_base: self.aerial_drift_base,
            air_drift_max: self.air_drift_max,
            aerial_friction: self.aerial_friction,
            gravity: self.gravity,
            terminal_velocity: self.terminal_velocity,
            fast_fall_velocity: self.fast_fall_velocity,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bone {
    pub parent: Option<usize>,
    pub classical_scale: bool,
    pub translation: [f32; 3],
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

impl Bone {
    pub(crate) fn physics(&self) -> bones::Bone {
        bones::Bone {
            parent: self.parent,
            classical_scale: self.classical_scale,
            local: bones::LocalTransform {
                translation: self.translation,
                rotation: self.rotation,
                scale: self.scale,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capsule {
    pub bone: usize,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
}

impl Capsule {
    pub(crate) fn physics(&self) -> bones::BoneCapsule {
        bones::BoneCapsule {
            bone: self.bone,
            start: self.start,
            end: self.end,
            radius: self.radius,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HurtboxState {
    #[default]
    Enabled,
    Disabled,
    Intangible,
}

impl HurtboxState {
    /// `lbColl_80007ECC` accepts only `HurtCapsule_Enabled` for ordinary hits
    /// and grabboxes.
    pub const fn accepts_contact(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Fighter-wide `Fighter::x1988` collision state written by subaction body
/// scripts. Slippi serializes these exact values (0, 1, 2) before the timed
/// game-induced counters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BodyState {
    #[default]
    Normal,
    Invincible,
    Intangible,
}

impl BodyState {
    /// `ftcoll.c` skips every hurt capsule while the fighter is intangible or
    /// invincible; the invincible branch registers no damage in this profile.
    pub const fn accepts_contact(self) -> bool {
        matches!(self, Self::Normal)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hurtbox {
    pub bone: usize,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub radius: f32,
    #[serde(default)]
    pub state: HurtboxState,
    #[serde(default = "default_true")]
    pub grabbable: bool,
}

impl Hurtbox {
    pub(crate) fn physics(&self) -> bones::BoneCapsule {
        bones::BoneCapsule {
            bone: self.bone,
            start: self.start,
            end: self.end,
            radius: self.radius,
        }
    }
}

const fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attack {
    /// Native move-table identity. Sentinel1 is exempt from stale-move damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub move_id: Option<u16>,
    /// Exactly one physics-pose sample per simulation frame, including recovery.
    /// No implicit interpolation or fallback for missing samples.
    pub frames: Vec<AttackFrame>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttackFrame {
    pub bones: Vec<Bone>,
    pub hitboxes: Vec<Hitbox>,
    /// Complete hurtbox eligibility sample for this frame. An empty sample
    /// preserves legacy resources by inheriting each hurtbox's base state.
    #[serde(default)]
    pub hurtbox_states: Vec<HurtboxState>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitElement {
    #[default]
    Normal,
    Inert,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hitbox {
    /// Ordinary collision-script clank/rebound bits. False retains older fixtures.
    #[serde(default)]
    pub clank: bool,
    #[serde(default)]
    pub rebound: bool,
    /// Collision element branch. Unsupported elements are rejected by serde.
    #[serde(default)]
    pub element: HitElement,
    /// Same-group hitboxes share victim history while active. The legacy profile
    /// without clank data retains its simpler per-attack group mask.
    pub group: u8,
    pub bone: usize,
    pub center: [f32; 3],
    pub radius: f32,
    pub damage: u32,
    /// Additional integer shield damage (HitCapsule::x34), before clamping.
    #[serde(default)]
    pub shield_damage: i32,
    /// Integral ordinary launch angles, 361 with explicit coefficients, or the
    /// contact-relative 362 angle used by fighter hitboxes.
    pub angle_degrees: f32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
}

#[cfg(test)]
mod tests {
    use super::HurtboxState;

    #[test]
    fn ordinary_contact_accepts_only_the_source_enabled_state() {
        assert!(HurtboxState::Enabled.accepts_contact());
        assert!(!HurtboxState::Disabled.accepts_contact());
        assert!(!HurtboxState::Intangible.accepts_contact());
    }
}
