// Shield arithmetic and shield callbacks from ftCo_Guard.c and
// Fighter_ProcessHit_8006D1EC. Host operation order is retained; this does
// not certify paired-single or animation-pipeline equivalence.

use crate::collision::bones;
use crate::game::{Action, Controller, Error, Fighter, data::FighterData};
use serde::{Deserialize, Serialize};

#[inline]
const fn registered_shield_entry_actions() -> (Action, Action) {
    (Action::Guard, Action::GuardOn)
}

fn blend(amount: f32, endpoints: [f32; 2]) -> f32 {
    amount * (endpoints[1] - endpoints[0]) + endpoints[0]
}

/// ftcoll.c getEnvDmg, for bounded finite stored HitCapsule damage. Nonzero
/// fractions that truncate to zero become one for shield damage and hitlag.
pub fn environment_damage(damage: f32) -> i32 {
    if damage == 0.0 {
        0
    } else if damage as i32 == 0 {
        1
    } else {
        damage as i32
    }
}

/// ftCo_800921DC/800925A4: released analog input retains the previous strength.
pub fn strength(trigger: f32, deadzone: f32, previous: f32) -> f32 {
    let value = (trigger - deadzone) / (1.0 - deadzone);
    if value < 0.0 { previous } else { value }
}

/// ftCo_Guard.c inlineB0, ordinary non-Yoshi branch.
pub fn radius(
    health: f32,
    maximum: f32,
    amount: f32,
    sizes: [f32; 2],
    minimum: f32,
    initial: f32,
) -> f32 {
    let n1 = (health / maximum) * blend(amount, sizes);
    let n2 = 1.0 - minimum;
    (n2 * n1 + minimum) * initial
}

/// ftCo_800925A4: zero is not broken until a later subtraction goes below zero.
pub fn drain(health: f32, amount: f32, scales: [f32; 2], rate: f32) -> (f32, bool) {
    let health = health - rate * blend(amount, scales);
    if health < 0.0 {
        (0.0, true)
    } else {
        (health, false)
    }
}

/// Fighter_ProcessHit_8006D1EC, including its per-process base subtraction.
pub fn damage_loss(damage: i32, amount: f32, scales: [f32; 2], multiplier: f32, base: f32) -> f32 {
    multiplier * (damage as f32 * (1.0 - blend(amount, scales))) + base
}

/// ftCo_80092ED8. Stun scales an animation's playback rate; it is not rounded
/// to an integer countdown by the original routine.
pub fn stun(damage: i32, amount: f32, scales: [f32; 2], multiplier: f32, base: f32) -> f32 {
    multiplier * (damage as f32 * (1.0 - blend(amount, scales))) + base
}

/// ftCo_80092F2C, ordinary non-parry/non-electric shield response.
pub fn response(
    stun: f32,
    animation_end: f32,
    push_scale: f32,
    normal_multiplier: f32,
    maximum: f32,
    hit_facing: f32,
) -> (f32, f32) {
    let rate = (0.1 + animation_end) / stun;
    let mut velocity = stun * push_scale;
    velocity *= normal_multiplier;
    if velocity > maximum {
        velocity = maximum;
    }
    (
        rate,
        if hit_facing < 0.0 {
            velocity
        } else {
            -velocity
        },
    )
}

/// ftCo_80093BC0: advance the independent reflector and damage-immunity
/// windows. A zero initial timer remains active through its entry frame and is
/// cleared by the first animation callback because the source tests `< 0`.
pub fn powershield_tick(
    just_started: &mut bool,
    reflecting: &mut bool,
    powershield: &mut bool,
    reflect_timer: &mut f32,
    powershield_timer: &mut f32,
) {
    if *just_started {
        *just_started = false;
    }
    if *reflecting {
        *reflect_timer -= 1.0;
        if *reflect_timer < 0.0 {
            *reflecting = false;
        }
    }
    if *powershield {
        *powershield_timer -= 1.0;
        if *powershield_timer < 0.0 {
            *powershield = false;
        }
    }
}

/// ftCo_80093240/800932DC, horizontal displacement along the floor tangent.
pub fn displacement(
    position: &mut [f32; 2],
    floor_normal: [f32; 3],
    stick_x: f32,
    minimum: f32,
    distance: f32,
    multiplier: f32,
) -> bool {
    if stick_x.abs() >= minimum {
        let scale = multiplier * (stick_x * distance);
        position[0] += floor_normal[1] * scale;
        position[1] += -floor_normal[0] * scale;
        true
    } else {
        false
    }
}

/// ftCommon_GrabMash's ordinary non-shake branch. Neutral input retains the
/// previous directional bucket; simultaneous button and direction changes count
/// twice, while any number of pressed buttons counts once.
pub fn mash(
    timer: &mut f32,
    directions: &mut [i8; 2],
    stick: [f32; 2],
    pressed: u16,
    threshold: f32,
    amount: f32,
) -> bool {
    let mut result = false;
    if pressed & (0x100 | 0x200 | 0x400 | 0x800 | 0x20 | 0x40) != 0 {
        *timer -= amount;
        result = true;
    }
    let previous = *directions;
    for axis in 0..2 {
        if stick[axis] < -threshold {
            directions[axis] = -1;
        }
        if stick[axis] > threshold {
            directions[axis] = 1;
        }
    }
    if previous != *directions {
        *timer -= amount;
        result = true;
    }
    result
}

// Shield callbacks with caller-supplied native common/character data. Yoshi
// shields, shield-tilt animation, shield grabs and reflected-projectile motion
// are separate unported branches; rolls and spot dodges live in `escape`.
// Shield break uses explicit native animation durations rather than guessed
// character timing or render state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    pub maximum_health: f32,
    pub minimum_radius_scale: f32,
    pub size_scales: [f32; 2],
    pub analog_deadzone: f32,
    pub minimum_hold_frames: f32,
    pub drain_rate: f32,
    pub drain_scales: [f32; 2],
    pub regeneration: f32,
    pub damage_scale: f32,
    pub damage_base: f32,
    pub damage_scales: [f32; 2],
    pub stun_scale: f32,
    pub stun_base: f32,
    pub stun_scales: [f32; 2],
    pub push_scale: f32,
    pub push_multiplier: f32,
    pub push_maximum: f32,
    pub attacker_push_scale: f32,
    pub attacker_push_base: f32,
    pub attacker_air_decay: f32,
    pub attacker_ground_friction_multiplier: f32,
    pub hitlag_maximum: f32,
    pub sdi_horizontal_multiplier: f32,
    pub break_health: f32,
    pub dizzy_percent_base: f32,
    pub dizzy_base_frames: f32,
    pub dizzy_frame_decay: f32,
    pub dizzy_mash_rate: f32,
    pub mash_stick_threshold: f32,
    pub powershield_input_window: u8,
    pub powershield_reflect_frames: f32,
    pub powershield_frames: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    pub bone: usize,
    pub offset: [f32; 3],
    pub initial_radius: f32,
    pub raise_frames: f32,
    pub release_frames: u32,
    pub stun_animation_end: f32,
    pub break_initial_velocity: f32,
    pub break_fly_frames: u32,
    pub break_down_frames: u32,
    pub break_stand_frames: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct ShieldState {
    pub health: f32,
    pub strength: f32,
    pub minimum_hold: f32,
    pub release_latched: bool,
    pub raise_progress: f32,
    pub stun_progress: f32,
    pub stun_rate: f32,
    pub attacker_push: [f32; 2],
    pub attacker_ground_push: f32,
    pub dizzy_timer: f32,
    pub mash_directions: [i8; 2],
    pub powershield_just_started: bool,
    pub reflecting: bool,
    pub powershield: bool,
    pub reflect_timer: f32,
    pub powershield_timer: f32,
    /// Per-frame x221C_b5 inert-hitbox overlap signal.
    pub touched: bool,
    /// Guard `x24` dash-grab buffer armed by `ftCo_80091B9C`.
    pub dash_grab_buffer: f32,
}

pub(crate) fn validate(r: &Rules, fighter: &FighterData) -> Result<(), Error> {
    let nonnegative = [
        r.maximum_health,
        r.minimum_hold_frames,
        r.drain_rate,
        r.regeneration,
        r.damage_scale,
        r.damage_base,
        r.stun_scale,
        r.stun_base,
        r.push_scale,
        r.push_multiplier,
        r.push_maximum,
        r.attacker_push_scale,
        r.attacker_push_base,
        r.attacker_air_decay,
        r.attacker_ground_friction_multiplier,
        r.hitlag_maximum,
        r.sdi_horizontal_multiplier,
        r.break_health,
        r.dizzy_percent_base,
        r.dizzy_base_frames,
        r.dizzy_frame_decay,
        r.dizzy_mash_rate,
        r.powershield_reflect_frames,
        r.powershield_frames,
    ];
    if !nonnegative
        .into_iter()
        .chain(r.size_scales)
        .chain(r.drain_scales)
        .all(|v| (0.0..=1_000_000.0).contains(&v))
        || !r
            .damage_scales
            .into_iter()
            .chain(r.stun_scales)
            .all(|v| (0.0..=1.0).contains(&v))
        || !(0.0..1.0).contains(&r.analog_deadzone)
        || !(0.0..=1.0).contains(&r.minimum_radius_scale)
        || r.minimum_radius_scale == 0.0
        || !(0.0..=1.0).contains(&r.mash_stick_threshold)
        || r.maximum_health == 0.0
        || r.stun_base == 0.0
        || r.dizzy_frame_decay == 0.0
        || r.break_health > r.maximum_health
    {
        return Err(Error::Data(
            "invalid or unsupported ordinary shield rules".into(),
        ));
    }
    if let Some(a) = &fighter.shield {
        if a.bone >= fighter.bones.len()
            || !a
                .offset
                .into_iter()
                .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            || ![
                a.initial_radius,
                a.raise_frames,
                a.stun_animation_end,
                a.break_initial_velocity,
            ]
            .into_iter()
            .all(|v| v > 0.0 && v <= 1_000_000.0)
            || ![
                a.release_frames,
                a.break_fly_frames,
                a.break_down_frames,
                a.break_stand_frames,
            ]
            .into_iter()
            .all(|v| (1..=1_000_000).contains(&v))
        {
            return Err(Error::Data(
                "invalid shield geometry or animation durations".into(),
            ));
        }
        if fighter.bones[a.bone].scale != [1.0; 3] {
            return Err(Error::Data(
                "shield bone requires unit local scale; its native radius replaces that scale"
                    .into(),
            ));
        }
    }
    Ok(())
}

pub(crate) fn active(f: &Fighter) -> bool {
    f.grounded
        && matches!(
            f.action,
            Action::GuardOn | Action::Guard | Action::GuardSetOff | Action::GuardReflect
        )
}

pub(crate) fn break_invulnerable(action: Action) -> bool {
    matches!(
        action,
        Action::ShieldBreakFly
            | Action::ShieldBreakFall
            | Action::ShieldBreakDown
            | Action::ShieldBreakStand
    )
}

fn breaking(action: Action) -> bool {
    break_invulnerable(action) || action == Action::Furafura
}

pub(crate) fn enter(f: &mut Fighter, action: Action) {
    crate::game::simulation::enter(f, action);
}

/// Fighter_ChangeMotionState clears the Slippi-visible x2218 reflect bit and
/// the x221C_b3 powershield entry latch on every ordinary transition out of a
/// guard state; the x221C_b2 immunity window and its timer persist because
/// only guard callbacks tick them.
pub(crate) fn leave_guard(f: &mut Fighter) {
    f.shield.reflecting = false;
    f.shield.powershield_just_started = false;
}

pub(crate) fn start_break(f: &mut Fighter, a: &Attributes) {
    f.grounded = false;
    f.ground_line = None;
    f.ground_velocity = 0.0;
    f.velocity = [0.0, a.break_initial_velocity];
    f.fast_fall = false;
    f.ecb_lock = 10;
    f.ecb.bottom_locked = true;
    f.locomotion.jumps_used = 1;
    enter(f, Action::ShieldBreakFly);
}

/// Priority-1 shield animation callback. The returned flag selects whether the
/// current destination action owns priority-3 input dispatch on this frame.
pub(crate) fn update_animation(
    f: &mut Fighter,
    data: &FighterData,
    rules: Option<&Rules>,
    input: Controller,
) -> bool {
    let (Some(r), Some(a)) = (rules, data.shield.as_ref()) else {
        return false;
    };
    let own_action = active(f) || matches!(f.action, Action::GuardOff) || breaking(f.action);
    if own_action {
        match f.action {
            Action::ShieldBreakFly if f.action_frame >= a.break_fly_frames => {
                enter(f, Action::ShieldBreakFall)
            }
            Action::ShieldBreakDown if f.action_frame >= a.break_down_frames => {
                enter(f, Action::ShieldBreakStand)
            }
            Action::ShieldBreakStand if f.action_frame >= a.break_stand_frames => {
                enter(f, Action::Furafura);
                let mut percent = r.dizzy_percent_base - f.percent;
                if percent < 0.0 {
                    percent = 0.0;
                }
                f.shield.dizzy_timer = percent + r.dizzy_base_frames;
                f.shield.mash_directions = [0; 2];
                f.shield.health = r.break_health;
            }
            Action::Furafura => {
                f.shield.health = r.break_health;
                f.shield.dizzy_timer -= r.dizzy_frame_decay;
                self::mash(
                    &mut f.shield.dizzy_timer,
                    &mut f.shield.mash_directions,
                    input.stick,
                    input.buttons & !f.previous_input.buttons,
                    r.mash_stick_threshold,
                    r.dizzy_mash_rate,
                );
                if f.shield.dizzy_timer <= 0.0 {
                    enter(f, Action::Wait);
                }
            }
            Action::GuardOff if f.action_frame >= a.release_frames => {
                enter(f, Action::Wait);
            }
            Action::GuardOn | Action::Guard | Action::GuardReflect => {
                if f.action == Action::GuardReflect {
                    self::powershield_tick(
                        &mut f.shield.powershield_just_started,
                        &mut f.shield.reflecting,
                        &mut f.shield.powershield,
                        &mut f.shield.reflect_timer,
                        &mut f.shield.powershield_timer,
                    );
                }
                // `Fighter_8006A360` (priority 1, the anim_cb host) calls
                // `ftCo_GuardOn_Anim`/`ftCo_Guard_Anim` -- and so
                // `ftCo_800925A4`'s own `fp->input.triggers[0]` read --
                // before `Fighter_Spaghetti_8006AD10` (priority 3) refreshes
                // `fp->input.{lstick,cstick,triggers,held_buttons}[0]` for
                // this frame (`fighter.c:1790-1839`, `HSD_GObj_SetupProc`
                // priorities `fighter.c:898,900`): the passive shield-drain
                // arithmetic always reads last frame's trigger, one frame
                // stale, unlike the IASA/action-transition checks that run
                // after the refresh. `f.previous_input` already holds that
                // value here, set from this match's own raw input only
                // later in `simulation::advance`.
                f.shield.strength = self::strength(
                    f.previous_input.shield_pressure(),
                    r.analog_deadzone,
                    f.shield.strength,
                );
                let (health, broken) = self::drain(
                    f.shield.health,
                    f.shield.strength,
                    r.drain_scales,
                    r.drain_rate,
                );
                f.shield.health = health;
                if broken {
                    start_break(f, a);
                    return true;
                }
                if f.shield.minimum_hold > 0.0 {
                    f.shield.minimum_hold = (f.shield.minimum_hold - 1.0).max(0.0);
                }
                f.shield.raise_progress += 1.0;
                if matches!(f.action, Action::GuardOn | Action::GuardReflect)
                    && f.shield.raise_progress >= a.raise_frames
                {
                    enter(f, Action::Guard);
                }
                f.shield.release_latched |= !input.shield_held();
                if f.shield.release_latched && f.shield.minimum_hold == 0.0 && !f.shield.reflecting
                {
                    enter(f, Action::GuardOff);
                    return true;
                }
            }
            Action::GuardSetOff => {
                self::powershield_tick(
                    &mut f.shield.powershield_just_started,
                    &mut f.shield.reflecting,
                    &mut f.shield.powershield,
                    &mut f.shield.reflect_timer,
                    &mut f.shield.powershield_timer,
                );
                f.shield.stun_progress += f.shield.stun_rate;
                if f.shield.stun_progress >= a.stun_animation_end {
                    enter(
                        f,
                        if f.shield.release_latched {
                            Action::GuardOff
                        } else {
                            Action::Guard
                        },
                    );
                }
                return true;
            }
            _ => {}
        }
        return active(f) || matches!(f.action, Action::GuardOff) || breaking(f.action);
    }
    false
}

/// Priority-3 shield input callback. Entry honors the existing supported attack
/// priority; the shield-grab path remains explicitly missing.
pub(crate) fn update_actions(
    f: &mut Fighter,
    data: &FighterData,
    rules: &crate::game::data::Rules,
    input: Controller,
    own_action: bool,
) -> Result<bool, Error> {
    let (Some(r), Some(_a)) = (rules.shield.as_ref(), data.shield.as_ref()) else {
        return Ok(false);
    };
    if own_action {
        if matches!(
            f.action,
            Action::GuardOn | Action::Guard | Action::GuardOff | Action::GuardReflect
        ) {
            let pressed = input.buttons & !f.previous_input.buttons;
            if f.action == Action::GuardOn
                && f.shield.raise_progress < f32::from(r.powershield_input_window)
                && pressed & (crate::game::BUTTON_L | crate::game::BUTTON_R) != 0
                && f.locomotion.trigger_age < r.powershield_input_window
            {
                start_powershield(f, data, r, false, input);
                return Ok(true);
            }
            // Every guard IASA chain checks ftCo_8009980C before ftCo_8009917C;
            // GuardOff_IASA offers only the spot dodge and the jump dispatcher.
            if crate::fighter::escape::try_spot_dodge(f, data, rules.escape.as_ref(), input)? {
                return Ok(true);
            }
            if f.action != Action::GuardOff
                && crate::fighter::escape::try_roll(f, data, rules.escape.as_ref(), input)?
            {
                return Ok(true);
            }
            // ftCo_800D8B9C and ftCo_Catch_CheckInput precede the jump chain
            // in GuardOn, Guard and GuardReflect; GuardOff skips both.
            if f.action != Action::GuardOff
                && crate::game::grab::update_shield_actions(f, data, rules.grab.as_ref(), input)
            {
                return Ok(true);
            }
            if let Some(source) = data
                .locomotion
                .as_ref()
                .and_then(|p| crate::fighter::locomotion::shield_jump_input(f, p, input))
            {
                f.short_hop = false;
                f.locomotion.jump_input = source;
                leave_guard(f);
                enter(f, Action::JumpSquat);
            }
        }
        return Ok(true);
    }
    if f.grounded
        && (matches!(
            f.action,
            Action::Wait | Action::Walk | Action::Turn | Action::Squat | Action::SquatWait
        )
            // Dash/Run's own phase-gated shield entry lives entirely in
            // dash::update_dash_or_run when rules.dash is present (Early
            // phase never checks shield at all); this ordinary, phase-blind
            // check only applies to them when that module is absent.
            || (rules.dash.is_none() && matches!(f.action, Action::Dash | Action::Run))
            // ftCo_AppealS_IASA reaches ftCo_80091A4C too (see taunt.rs);
            // Chain::DownTilt's own real chain has no shield check.
            || matches!(
                crate::fighter::tilt::interrupt_chain(f, data),
                Some(crate::fighter::tilt::Chain::Wait) | Some(crate::fighter::tilt::Chain::Taunt)
            ))
        && !crate::fighter::smash::a_pressed(f, input)
        && crate::fighter::smash::select(f, data, rules.smash.as_ref(), input, f.facing).is_none()
    {
        // Run_IASA and Dash_IASA call ftCo_80091B9C after either shield entry.
        let dash_grab_buffer = crate::game::grab::shield_entry_buffer(f, rules.grab.as_ref());
        return Ok(enter_from_neutral(f, data, r, input, dash_grab_buffer));
    }
    Ok(false)
}

/// `ftCo_80091A4C`/`ftCo_80091AD8`'s shared entry tail: a fresh L/R press
/// inside the powershield window starts a powershield, otherwise a held
/// shoulder with remaining shield health raises an ordinary guard.
/// `dash_grab_buffer` is `ftCo_80091B9C`'s guard `x24` arm, already gated by
/// the caller (`grab::shield_entry_buffer`); Dash's own middle- and
/// late-phase dispatch reuses this directly.
pub(crate) fn enter_from_neutral(
    f: &mut Fighter,
    data: &FighterData,
    r: &Rules,
    input: Controller,
    dash_grab_buffer: f32,
) -> bool {
    let pressed = input.buttons & !f.previous_input.buttons;
    if pressed & (crate::game::BUTTON_L | crate::game::BUTTON_R) != 0
        && f.locomotion.trigger_age < r.powershield_input_window
    {
        start_powershield(f, data, r, true, input);
        f.shield.dash_grab_buffer = dash_grab_buffer;
        return true;
    }
    if !input.shield_held() || f.shield.health == 0.0 {
        return false;
    }
    f.shield.strength = self::strength(input.shield_pressure(), r.analog_deadzone, 0.0);
    f.shield.minimum_hold = r.minimum_hold_frames;
    f.shield.release_latched = false;
    f.shield.raise_progress = 0.0;
    clear_powershield(f);
    // A registered ordinary shield move is canonically `Guard`, but native
    // entry still has to expose the distinct `GuardOn` raise phase. Custom
    // registrations remain authoritative through the selector's
    // `default_entry`/`native_variant` comparison.
    let (default_entry, native_variant) = registered_shield_entry_actions();
    let selected = crate::game::script::move_selection::select_native_move_variant(
        f,
        data,
        crate::game::script::move_registry::MoveGroup::Defense,
        crate::game::script::move_registry::MoveSlot::Shield,
        default_entry,
        native_variant,
    );
    if matches!(
        selected,
        crate::game::script::move_selection::NativeMoveSelection::CallbackDriven { .. }
    ) {
        return true;
    }
    f.shield.dash_grab_buffer = dash_grab_buffer;
    true
}

fn clear_powershield(f: &mut Fighter) {
    f.shield.powershield_just_started = false;
    f.shield.reflecting = false;
    f.shield.powershield = false;
    f.shield.reflect_timer = 0.0;
    f.shield.powershield_timer = 0.0;
}

fn start_powershield(
    f: &mut Fighter,
    data: &FighterData,
    r: &Rules,
    initialize: bool,
    input: Controller,
) {
    if initialize {
        f.shield.strength = self::strength(input.shield_pressure(), r.analog_deadzone, 0.0);
        f.shield.minimum_hold = r.minimum_hold_frames;
        f.shield.release_latched = false;
        f.shield.raise_progress = 0.0;
    }
    f.locomotion.trigger_age = 254;
    f.shield.powershield_just_started = true;
    f.shield.reflecting = true;
    f.shield.powershield = true;
    f.shield.reflect_timer = r.powershield_reflect_frames;
    f.shield.powershield_timer = r.powershield_frames;
    let _ = crate::game::script::move_selection::select_native_move_variant(
        f,
        data,
        crate::game::script::move_registry::MoveGroup::Defense,
        crate::game::script::move_registry::MoveSlot::Shield,
        Action::GuardOn,
        Action::GuardReflect,
    );
}

/// ProcessHit regenerates outside guard, and subtracts its base cost even with
/// zero shield-damage accumulation. Hit damage is applied by
/// `game::hit_resolution::apply_shield_contact`.
/// `was_active` is ordinarily just `active(f)` as of this call; its caller
/// additionally folds in whether this exact frame converted a still-active
/// `GuardOn`/`Guard` straight into `Pass` (`simulation::advance`'s own
/// `shield_active_into_pass`), since `fox-bf.slp` shows no regeneration
/// lands on that conversion frame even though `begin_pass`'s own
/// `Fighter_ChangeMotionState` call already cleared `x221A_b7` (the flag
/// `Fighter_ProcessHit_8006D1EC` gates regeneration on) earlier the same
/// frame.
pub(crate) fn finish_frame(
    f: &mut Fighter,
    rules: Option<&Rules>,
    shield_contact: bool,
    was_active: bool,
) {
    let Some(r) = rules else { return };
    if shield_contact {
        return;
    }
    if was_active {
        f.shield.health -= r.damage_base;
        if f.shield.health < 0.0 {
            f.shield.health = r.break_health;
        }
    } else if f.shield.health < r.maximum_health {
        f.shield.health += r.regeneration;
        if f.shield.health > r.maximum_health {
            f.shield.health = r.maximum_health;
        }
    }
}

pub(crate) fn hitlag(
    f: &mut Fighter,
    input: Controller,
    rules: &crate::game::data::Rules,
    exiting: bool,
) {
    let (Some(r), Some(d)) = (&rules.shield, &rules.damage.displacement) else {
        return;
    };
    if f.action != Action::GuardSetOff || !f.grounded {
        return;
    }
    if (exiting || f.locomotion.tilt_x_age < d.sdi_window)
        && self::displacement(
            &mut f.position,
            f.floor_normal,
            input.stick[0],
            d.minimum_stick_magnitude,
            if exiting {
                d.asdi_distance
            } else {
                d.sdi_distance
            },
            r.sdi_horizontal_multiplier,
        )
        && !exiting
    {
        f.locomotion.tilt_x_age = 254;
    }
}

// Source attacker recoil remains separate from self velocity and knockback.
/// Includes the original airborne decay typo that clears knockback Y rather
/// than shield-recoil Y when the vector is smaller than the decay amount.
pub(crate) fn recoil(f: &mut Fighter, data: &FighterData, rules: Option<&Rules>) {
    let Some(r) = rules else { return };
    let [x, y] = f.shield.attacker_push;
    if x == 0.0 && y == 0.0 {
        return;
    }
    if f.grounded {
        if f.shield.attacker_ground_push == 0.0 {
            f.shield.attacker_ground_push = x;
        }
        f.shield.attacker_ground_push = crate::fighter::decrement_toward_zero(
            f.shield.attacker_ground_push,
            data.movement.ground_friction * r.attacker_ground_friction_multiplier,
        );
        f.shield.attacker_push = [
            f.floor_normal[1] * f.shield.attacker_ground_push,
            -f.floor_normal[0] * f.shield.attacker_ground_push,
        ];
    } else {
        let angle = crate::compat::math::trig::atan2f(y, x);
        if libm::sqrtf(x * x + y * y) < r.attacker_air_decay {
            f.shield.attacker_push[0] = 0.0;
            f.knockback[1] = 0.0;
        } else {
            f.shield.attacker_push[0] -=
                r.attacker_air_decay * crate::compat::math::trig::cosf(angle);
            f.shield.attacker_push[1] -=
                r.attacker_air_decay * crate::compat::math::trig::sinf(angle);
        }
        f.shield.attacker_ground_push = 0.0;
    }
}

/// Matrix supplied to the original shield narrowphase. The shield bone's scale
/// receives inlineB0's radius, while its center follows the supplied bone pose.
pub(crate) fn geometry(
    f: &Fighter,
    data: &FighterData,
    r: &Rules,
    pose: &bones::Pose,
) -> Result<([f32; 3], bones::Matrix), Error> {
    let a = data
        .shield
        .as_ref()
        .ok_or_else(|| Error::Physics("missing shield geometry".into()))?;
    let mut matrix = *pose
        .world_matrix(a.bone)
        .map_err(|e| Error::Physics(e.to_string()))?;
    let radius = self::radius(
        f.shield.health,
        r.maximum_health,
        f.shield.strength,
        r.size_scales,
        r.minimum_radius_scale,
        a.initial_radius,
    );
    for row in &mut matrix {
        for value in &mut row[..3] {
            *value *= radius;
        }
    }
    let center = bones::transform_point(&matrix, a.offset);
    if center
        .into_iter()
        .chain(matrix.into_iter().flatten())
        .any(|v| !v.is_finite())
    {
        return Err(Error::NonFinite);
    }
    Ok((center, matrix))
}

#[cfg(test)]
mod tests {
    use super::{Attributes, Rules, powershield_tick};
    use crate::game::{Action, Controller, Match};

    #[test]
    fn neutral_shield_enters_guard_on_then_progresses_to_guard() {
        #[derive(serde::Deserialize)]
        struct ShieldFixture {
            rules: Rules,
            attributes: Attributes,
        }

        let mut data: crate::game::data::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap();
        let shield: ShieldFixture =
            serde_json::from_str(include_str!("../../tests/fixtures/game/shield.json")).unwrap();
        data.rules.countdown_frames = 0;
        data.rules.shield = Some(shield.rules);
        data.fighters[0].shield = Some(shield.attributes);

        let mut game = Match::new(data, 0).unwrap();
        let held = Controller {
            trigger: 1.0,
            ..Controller::default()
        };
        let first = game.step([held, Controller::default()]).unwrap();
        assert_eq!(first.fighters[0].action, Action::GuardOn);

        let second = game.step([held, Controller::default()]).unwrap();
        assert!(matches!(
            second.fighters[0].action,
            Action::GuardOn | Action::Guard
        ));
        let third = game.step([held, Controller::default()]).unwrap();
        assert_eq!(third.fighters[0].action, Action::Guard);
    }

    #[test]
    fn zero_windows_remain_active_on_entry_and_clear_on_the_first_tick() {
        let (mut fresh, mut reflect, mut shield) = (true, true, true);
        let (mut reflect_timer, mut shield_timer) = (0.0, 0.0);
        powershield_tick(
            &mut fresh,
            &mut reflect,
            &mut shield,
            &mut reflect_timer,
            &mut shield_timer,
        );
        assert_eq!(
            (fresh, reflect, shield, reflect_timer, shield_timer),
            (false, false, false, -1.0, -1.0)
        );
    }
}
