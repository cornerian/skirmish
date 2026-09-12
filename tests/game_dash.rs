//! End-to-end Dash-phase dispatch (`ftCo_Dash_IASA`), Run's dash-attack/
//! shield arms (`ftCo_Run_IASA`) and AttackDash (`ftCo_AttackDash.c`) in an
//! explicitly synthetic native world.
#[path = "support/dash.rs"]
mod dash_support;
#[path = "support/escape.rs"]
mod escape_support;
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::fighter::dash::{apply_friction, transition_friction};
use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, Controller, Event, Match, State,
    dash::Rules as DashRules, data::MatchData, grab::ShieldGrabRules, shield,
};
use skirmish_replay::{observation, slippi::Port};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

/// Fighter 0 (facing +X, far from fighter 1) dashes; fighter 1 idles out of
/// reach so hitlag never freezes the dasher's frame count.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let mut profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    // The default fixture's powershield_input_window is 0 (fresh presses can
    // never qualify); the middle/late shield tests need a real window.
    profile.rules.powershield_input_window = 3;
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-5.0, 0.0], [20.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
    }
    let data = smash_support::profile(tilt_support::profile(grab_support::profile(
        escape_support::profile(data),
    )));
    dash_support::profile(data)
}

/// Both fighters within the dash attack's forward hitbox reach.
fn close() -> MatchData {
    let mut data = data();
    data.stage.spawns = [[-1.0, 0.0], [7.0, 0.0]];
    data.rules.knockback_speed = 0.15;
    data
}

fn stick(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn buttons(buttons: u16) -> Controller {
    stick(buttons, [0.0, 0.0])
}

fn step(game: &mut Match, controller: Controller) -> State {
    game.step([controller, Controller::default()])
        .unwrap()
        .clone()
}

/// Step once with a fresh forward stick, entering Dash from Wait.
fn enter_dash(game: &mut Match) -> State {
    let state = step(game, stick(0, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Dash);
    assert_eq!(state.fighters[0].action_frame, 1);
    assert!(state.fighters[0].locomotion.dash_from_input);
    state
}

/// Step `frames` times holding neutral, staying in Dash throughout (no stick
/// magnitude means neither the run transition nor try_dash can fire).
fn hold_neutral(game: &mut Match, frames: u32) -> State {
    let mut state = step(game, buttons(0));
    for _ in 1..frames {
        state = step(game, buttons(0));
        assert_eq!(state.fighters[0].action, Action::Dash);
    }
    state
}

/// `fox-fd.slp` reports P1's `state_age` as 1.0 on the exact frame Dash is
/// entered from a fresh stick press (frame -37), not 0.0 like an ordinary
/// entry (`docs/parity.md`'s entry for this batch, `docs/validation.md`):
/// `ftCo_Dash_Enter` (`ftCo_Dash.c:48-63`) calls `ftAnim_8006EBA4(gobj)`
/// immediately after `Fighter_ChangeMotionState`, an extra explicit
/// animation advance most `_Enter`s (confirmed absent from `ftCo_Fall_
/// Enter`/`ftCo_Landing_Enter`/`ftCo_Run_Enter_Full`/`ftCo_KneeBend_Enter`)
/// don't make.
#[test]
fn entering_dash_from_a_fresh_press_reports_the_replay_verified_age_of_one() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_dash(&mut game);
    let observed = observation::observe(&game, [Port::P1, Port::P4], [2, 2]);
    assert_eq!(observed.fighters[0].action_age, 1.0);
}

#[test]
fn early_phase_forward_smash_keeps_facing_and_flips_with_the_cstick() {
    // Frame 1 is inside the early phase (<=3). The stick that entered the
    // dash is already "stale" (x670 reset to 254 by ftCo_Dash_Enter), so the
    // dash-specific check (no age window) fires where the ordinary
    // Wait-chain smash could not.
    let mut same = Match::new(data(), 42).unwrap();
    enter_dash(&mut same);
    let state = step(&mut same, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AttackS4S);
    assert_eq!(state.fighters[0].facing, 1.0);

    let mut flip = Match::new(data(), 42).unwrap();
    enter_dash(&mut flip);
    let cstick_flip = Controller {
        stick: [1.0, 0.0],
        cstick: [-1.0, 0.0],
        ..Default::default()
    };
    let state = step(&mut flip, cstick_flip);
    assert_eq!(state.fighters[0].action, Action::AttackS4S);
    assert_eq!(state.fighters[0].facing, -1.0);
}

#[test]
fn early_phase_forward_roll_only_lasts_through_the_roll_limit() {
    // Frame 1 <= roll_frames (2): held shoulder rolls immediately.
    let mut early = Match::new(data(), 42).unwrap();
    enter_dash(&mut early);
    let state = step(&mut early, buttons(BUTTON_L));
    assert_eq!(state.fighters[0].action, Action::EscapeF);

    // Frame 3 is still early (<=3) but past the roll limit (2): the shoulder
    // is not consulted at all here, and nothing else fires either.
    let mut late_early = Match::new(data(), 42).unwrap();
    enter_dash(&mut late_early);
    hold_neutral(&mut late_early, 2); // frame 2, neutral: nothing fires.
    let state = step(&mut late_early, buttons(BUTTON_L)); // frame 3.
    assert_eq!(state.fighters[0].action, Action::Dash);

    // Frame 4 is the middle phase: the shoulder now raises an ordinary
    // guard, without the dash-grab buffer (frame 4 <= the 6.0 limit).
    let state = step(&mut late_early, buttons(BUTTON_L)); // frame 4.
    assert_eq!(state.fighters[0].action, Action::GuardOn);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 0.0);
}

#[test]
fn dash_attack_fires_from_the_middle_phase_and_from_run_but_not_early_or_late() {
    // Early phase: A with a neutral stick does nothing dash-attack related.
    let mut early = Match::new(data(), 42).unwrap();
    enter_dash(&mut early);
    let state = step(&mut early, buttons(BUTTON_A));
    assert_eq!(state.fighters[0].action, Action::Dash);

    // Middle phase (frame 4): a fresh A enters AttackDash and arms the
    // catch buffer to the shared shield-grab x68 value.
    let mut middle = Match::new(data(), 42).unwrap();
    enter_dash(&mut middle);
    hold_neutral(&mut middle, 3); // frames 2..=4 (early tail into middle).
    let state = step(&mut middle, buttons(BUTTON_A)); // frame 4.
    assert_eq!(state.fighters[0].action, Action::AttackDash);
    assert_eq!(
        state.fighters[0].dash.grab_buffer,
        dash_support::BUFFER.dash_buffer_frames
    );

    // Run: the same fresh-A entry, also arming the buffer.
    let mut run = Match::new(data(), 42).unwrap();
    for _ in 0..9 {
        let state = step(&mut run, stick(0, [1.0, 0.0]));
        assert!(matches!(
            state.fighters[0].action,
            Action::Dash | Action::Run
        ));
    }
    assert_eq!(run.state().fighters[0].action, Action::Run);
    let state = step(&mut run, stick(BUTTON_A, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::AttackDash);
    assert_eq!(
        state.fighters[0].dash.grab_buffer,
        dash_support::BUFFER.dash_buffer_frames
    );

    // Late phase (frame 7): A no longer enters AttackDash at all.
    let mut late = Match::new(data(), 42).unwrap();
    enter_dash(&mut late);
    hold_neutral(&mut late, 6); // frames 2..=7 (early, middle, into late).
    let state = step(&mut late, buttons(BUTTON_A)); // frame 7: late phase.
    assert_ne!(state.fighters[0].action, Action::AttackDash);
}

/// `fox-fd.slp`'s dash-dance rally reports P1's `state_age` as 1.0 on the
/// exact frame Turn is entered from Dash's dash-back check (frame -30, and
/// again at -25), not 0.0 like an ordinary entry: `ftCo_Turn_Enter_Smash`
/// (`ftCo_Turn.c:173-188`, reached here through `ftCo_Dash_CheckInput`'s
/// `start_turn(f, p, true)`) calls `ftAnim_8006EBA4(gobj)` immediately after
/// `Fighter_ChangeMotionState`, the same extra animation advance
/// `ftCo_Dash_Enter` makes (`docs/parity.md`'s -30 divergence, fixed by this
/// batch).
#[test]
fn entering_a_smash_turn_from_dash_reports_the_replay_verified_age_of_one() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_dash(&mut game);
    hold_neutral(&mut game, 3); // frames 2..=4: middle phase.
    let state = step(&mut game, stick(0, [-1.0, 0.0])); // frame 4: dash-back.
    assert_eq!(state.fighters[0].action, Action::Turn);
    let observed = observation::observe(&game, [Port::P1, Port::P4], [2, 2]);
    assert_eq!(observed.fighters[0].action_age, 1.0);
}

#[test]
fn middle_phase_dash_back_enters_a_smash_turn_on_the_opposite_stick_only() {
    // Early phase: an opposite stick matches neither the forward-smash nor
    // the roll checks, so nothing dash-back-related happens.
    let mut early = Match::new(data(), 42).unwrap();
    enter_dash(&mut early);
    let state = step(&mut early, stick(0, [-1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Dash);

    // Middle phase (frame 4): a fresh opposite stick inside the dash window
    // enters a smash Turn instead of restarting the dash.
    let mut middle = Match::new(data(), 42).unwrap();
    enter_dash(&mut middle);
    hold_neutral(&mut middle, 3); // frames 2..=4.
    let state = step(&mut middle, stick(0, [-1.0, 0.0])); // frame 4.
    assert_eq!(state.fighters[0].action, Action::Turn);
    assert_eq!(
        state.fighters[0].facing, 1.0,
        "Turn flips only once it resolves"
    );
}

/// `fox-fd.slp` reports P1 already in Run at the frame whose *pre-increment*
/// `action_frame` is one less than `dash_run_frame` (12): holding forward
/// through Dash frames -24..-14 (`action_age` 1..11) enters Run already at
/// -13, not -12 (`docs/parity.md`'s frame -13 divergence, fixed by this
/// batch). `game::dash::update_dash_or_run`'s own comment on this check
/// explains why: the generic per-frame animation advance that lands
/// decomp's `cur_anim_frame` on this frame's own count runs before
/// `ftCo_Dash_IASA` reads it, while Skirmish's shared end-of-frame
/// `action_frame += 1` has not yet run at the point this check reads
/// `action_frame`, so the comparison needs `action_frame + 1`. This fixture
/// pins the same relationship with its own `dash_run_frame` (8, `tests/
/// fixtures/game/locomotion.json`): Run is entered once `action_frame`
/// reaches 7, one frame before the unadjusted `action_frame >= 8` would
/// have fired.
#[test]
fn holding_forward_through_dash_enters_run_one_frame_before_the_unadjusted_threshold() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_dash(&mut game); // action_frame 1.
    for expected_frame in 2..=7 {
        let state = step(&mut game, stick(0, [1.0, 0.0]));
        assert_eq!(state.fighters[0].action, Action::Dash);
        assert_eq!(state.fighters[0].action_frame, expected_frame);
    }
    let state = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Run);
}

#[test]
fn late_phase_redash_restarts_input_entered_dash_only_in_the_late_phase() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_dash(&mut game);
    hold_neutral(&mut game, 6); // frames 2..=7: early tail, middle, into late.
    assert_eq!(game.state().fighters[0].action, Action::Dash);
    // Frame 7 is the late phase; a fresh same-direction stick restarts Dash.
    let state = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Dash);
    assert_eq!(state.fighters[0].action_frame, 1);
    assert!(state.fighters[0].locomotion.dash_from_input);
}

#[test]
fn late_phase_redash_ground_velocity_reflects_the_transition_friction_tail() {
    let mut with_rules = Match::new(data(), 42).unwrap();
    enter_dash(&mut with_rules);
    let primed = hold_neutral(&mut with_rules, 6); // lands right before frame 7.
    let gr_vel_before = primed.fighters[0].ground_velocity;
    let redashed = step(&mut with_rules, stick(0, [1.0, 0.0])); // frame 7: re-dash.
    assert_eq!(redashed.fighters[0].action, Action::Dash);
    assert_eq!(redashed.fighters[0].action_frame, 1);
    // ftCo_Dash_Enter computes dash.x0 = facing*dash_initial_velocity - gr_vel
    // from the PRE-tail ground_velocity (start_dash runs before
    // apply_transition_friction), so the entry frame's ground movement adds
    // that accel to the tail-reduced velocity: dash_initial_velocity -
    // gr_vel_before * x54 (facing is 1.0 throughout this scenario).
    let dash_initial_velocity = 2.0; // tests/fixtures/game/locomotion.json
    let expected = dash_initial_velocity - gr_vel_before * dash_support::RULES.transition_friction;
    assert_eq!(
        redashed.fighters[0].ground_velocity.to_bits(),
        expected.to_bits()
    );

    // rules.dash = None has no tail, and try_dash is unreachable from within
    // an already-ongoing Dash in that path (only Wait/Walk/SquatWait reach
    // it), so the same fresh press just continues ordinary Dash physics --
    // one frame earlier than the `with_rules` scenario above, to stay clear
    // of `dash_run_frame`'s own boundary (the fixture's 8, checked against
    // `action_frame + 1`, the real-replay parity loop's Dash-to-Run timing
    // fix: `game::dash::update_dash_or_run`'s own comment). Six frames of
    // neutral here would instead land exactly on that boundary and enter
    // Run, which this scenario isn't testing.
    let mut without_data = data();
    without_data.rules.dash = None;
    for fighter in &mut without_data.fighters {
        fighter.dash_attack = None;
    }
    let mut without_rules = Match::new(without_data, 42).unwrap();
    enter_dash(&mut without_rules);
    hold_neutral(&mut without_rules, 5);
    let continued = step(&mut without_rules, stick(0, [1.0, 0.0]));
    assert_ne!(continued.fighters[0].action_frame, 1);
    assert_ne!(
        continued.fighters[0].ground_velocity.to_bits(),
        expected.to_bits()
    );
}

#[test]
fn late_phase_guard_on_ground_velocity_reflects_the_transition_friction_tail() {
    // The shield-grab limit equals the early-phase limit so there is no
    // middle window: a shoulder held since the early phase reaches its
    // first (late-phase) check still "held, not fresh" there, matching
    // `middle_and_late_phase_shield_entries_differ_only_by_the_grab_buffer`.
    let mut late_data = data();
    late_data.rules.grab.as_mut().unwrap().shield_grab = Some(ShieldGrabRules {
        dash_buffer_frame_limit: dash_support::RULES.early_frames,
        ..dash_support::BUFFER
    });
    let mut with_rules = Match::new(late_data, 42).unwrap();
    enter_dash(&mut with_rules);
    hold_neutral(&mut with_rules, 2); // frames 1, 2 neutral.
    let held = step(&mut with_rules, buttons(BUTTON_L)); // frame 3 (early): primes "held".
    let gr_vel_before = held.fighters[0].ground_velocity;
    let entered = step(&mut with_rules, buttons(BUTTON_L)); // frame 4: now late.
    assert_eq!(entered.fighters[0].action, Action::GuardOn);
    // GuardOn's own ground physics (ordinary friction, `ft_80084F3C`-style)
    // runs after dash::update_dash_or_run's tail already reduced gr_vel this
    // same frame, so the two compose: apply_friction mirrors GuardOn's own
    // `Movement::friction_ground`, which is bit-for-bit the same formula.
    let ground_friction = 0.2; // tests/fixtures/game/integration-match.json
    let expected = apply_friction(
        transition_friction(gr_vel_before, dash_support::RULES.transition_friction, 1.0),
        ground_friction,
    );
    assert_eq!(
        entered.fighters[0].ground_velocity.to_bits(),
        expected.to_bits()
    );

    // rules.dash = None has no tail, and the old unconditional shield path
    // checks every Dash frame (no early-phase exclusion), so a bare-first
    // press already enters GuardOn (with the window disabled, since a
    // genuinely fresh press would otherwise open GuardReflect instead)
    // straight from the untouched ground_velocity.
    let mut without_data = data();
    without_data.rules.dash = None;
    for fighter in &mut without_data.fighters {
        fighter.dash_attack = None;
    }
    without_data
        .rules
        .shield
        .as_mut()
        .unwrap()
        .powershield_input_window = 0;
    let mut without_rules = Match::new(without_data, 42).unwrap();
    let after_entry = enter_dash(&mut without_rules);
    let gr_vel_before_without = after_entry.fighters[0].ground_velocity;
    let entered_without = step(&mut without_rules, buttons(BUTTON_L));
    assert_eq!(entered_without.fighters[0].action, Action::GuardOn);
    let expected_without = apply_friction(gr_vel_before_without, ground_friction);
    assert_eq!(
        entered_without.fighters[0].ground_velocity.to_bits(),
        expected_without.to_bits()
    );
}

#[test]
fn middle_and_late_phase_shield_entries_differ_only_by_the_grab_buffer() {
    // "Held" (not fresh) requires the shoulder already down on the strictly
    // preceding frame; since the early phase never consults it at all, a
    // shoulder held since an early frame is still "held, not fresh" on the
    // very first frame a shield check runs, without ever tripping the
    // powershield window (fresh raw button transitions only).
    // Middle phase (frame 4): held L raises an ordinary guard, unbuffered.
    let mut middle = Match::new(data(), 42).unwrap();
    enter_dash(&mut middle);
    hold_neutral(&mut middle, 2); // frames 1, 2 neutral.
    step(&mut middle, buttons(BUTTON_L)); // frame 3 (early): primes "held".
    let state = step(&mut middle, buttons(BUTTON_L)); // frame 4 (middle).
    assert_eq!(state.fighters[0].action, Action::GuardOn);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 0.0);

    // Middle phase: a fresh press inside the powershield window instead
    // opens GuardReflect, arming the same (here zero) buffer.
    let mut middle_press = Match::new(data(), 42).unwrap();
    enter_dash(&mut middle_press);
    hold_neutral(&mut middle_press, 3);
    let state = step(&mut middle_press, buttons(BUTTON_L));
    assert_eq!(state.fighters[0].action, Action::GuardReflect);
    assert_eq!(state.fighters[0].shield.dash_grab_buffer, 0.0);

    // Late phase: with the shield-grab limit equal to the early-phase limit
    // there is no middle window, so a shoulder held since the early phase
    // reaches its first (late-phase) check still "held, not fresh" there.
    let mut late_data = data();
    late_data.rules.grab.as_mut().unwrap().shield_grab = Some(ShieldGrabRules {
        dash_buffer_frame_limit: dash_support::RULES.early_frames,
        ..dash_support::BUFFER
    });
    let mut late = Match::new(late_data, 42).unwrap();
    enter_dash(&mut late);
    hold_neutral(&mut late, 2); // frames 1, 2 neutral (frame 2 is still
    // inside the roll limit, so a held shoulder there would roll instead).
    step(&mut late, buttons(BUTTON_L)); // frame 3 (early, past the roll
    // limit): primes "held" without rolling or checking the shield yet.
    let state = step(&mut late, buttons(BUTTON_L)); // frame 4: now late.
    assert_eq!(state.fighters[0].action, Action::GuardOn);
    assert_eq!(
        state.fighters[0].shield.dash_grab_buffer,
        dash_support::BUFFER.dash_buffer_frames
    );

    // Late phase (frame 7 under the ordinary profile): a fresh press also
    // opens GuardReflect, arming the buffer this phase would have armed.
    let mut late_press = Match::new(data(), 42).unwrap();
    enter_dash(&mut late_press);
    hold_neutral(&mut late_press, 6);
    let state = step(&mut late_press, buttons(BUTTON_L));
    assert_eq!(state.fighters[0].action, Action::GuardReflect);
    assert_eq!(
        state.fighters[0].shield.dash_grab_buffer,
        dash_support::BUFFER.dash_buffer_frames
    );
}

#[test]
fn an_idle_dash_frame_matches_ground_velocity_with_rules_dash_none() {
    // ftCo_Dash_IASA's x54 friction tail runs only after a taunt entry
    // (ftCo_800DE9D8/ftCo_800DE9B8, D-pad up, ftCo_AppealS.c:35,43), which is
    // unmodeled, so a frame where nothing else fires must leave
    // ground_velocity exactly as the ordinary Dash ground physics computed
    // it, identical to `rules.dash = None`.
    let mut with_rules = Match::new(data(), 42).unwrap();
    let mut without_rules = Match::new(
        {
            let mut d = data();
            d.rules.dash = None;
            for fighter in &mut d.fighters {
                fighter.dash_attack = None;
            }
            d
        },
        42,
    )
    .unwrap();
    enter_dash(&mut with_rules);
    enter_dash(&mut without_rules);
    // Frame 1: nothing fires with a neutral stick in either configuration.
    let with_gr_vel = step(&mut with_rules, buttons(0)).fighters[0].ground_velocity;
    let without_gr_vel = step(&mut without_rules, buttons(0)).fighters[0].ground_velocity;
    assert_eq!(with_gr_vel, without_gr_vel);
}

#[test]
fn attack_dash_catches_interrupts_and_hits_once() {
    // CatchDash: held L within the buffer, no A needed, decrementing when
    // held past a fresh entry.
    let mut catch = Match::new(data(), 42).unwrap();
    enter_dash(&mut catch);
    hold_neutral(&mut catch, 3);
    let state = step(&mut catch, buttons(BUTTON_A)); // frame 4: AttackDash.
    assert_eq!(state.fighters[0].action, Action::AttackDash);
    let state = step(&mut catch, buttons(BUTTON_L));
    assert_eq!(state.fighters[0].action, Action::CatchDash);

    // The buffer expires: enough neutral frames drain it to zero.
    let mut expire = Match::new(data(), 42).unwrap();
    enter_dash(&mut expire);
    hold_neutral(&mut expire, 3);
    step(&mut expire, buttons(BUTTON_A)); // AttackDash, buffer armed.
    for _ in 0..dash_support::BUFFER.dash_buffer_frames as u32 {
        step(&mut expire, buttons(0));
    }
    let state = step(&mut expire, buttons(BUTTON_L));
    assert_ne!(state.fighters[0].action, Action::CatchDash);

    // The Wait chain only opens on flagged poses.
    let mut wait_chain = Match::new(data(), 42).unwrap();
    enter_dash(&mut wait_chain);
    hold_neutral(&mut wait_chain, 3);
    step(&mut wait_chain, buttons(BUTTON_A)); // AttackDash frame 0.
    for _ in 0..dash_support::INTERRUPT_FROM - 1 {
        let state = step(&mut wait_chain, buttons(0));
        assert_eq!(state.fighters[0].action, Action::AttackDash);
    }
    // Now on the flagged pose: a fresh jump input opens the Wait chain.
    let state = step(&mut wait_chain, stick(BUTTON_X, [0.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::JumpSquat);

    // Ends in Wait once the poses are exhausted.
    let mut ends = Match::new(data(), 42).unwrap();
    enter_dash(&mut ends);
    hold_neutral(&mut ends, 3);
    step(&mut ends, buttons(BUTTON_A));
    for _ in 0..dash_support::FRAMES {
        step(&mut ends, buttons(0));
    }
    assert_eq!(ends.state().fighters[0].action, Action::Wait);

    // A close victim is hit exactly once.
    let mut hit = Match::new(close(), 42).unwrap();
    enter_dash(&mut hit);
    hold_neutral(&mut hit, 3);
    step(&mut hit, buttons(BUTTON_A));
    let mut hits = 0;
    for _ in 0..dash_support::FRAMES {
        let state = step(&mut hit, buttons(0));
        hits += state
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::Hit {
                        attacker: 0,
                        victim: 1,
                        ..
                    }
                )
            })
            .count();
    }
    assert_eq!(hits, 1);
}

#[test]
fn checkpoints_restore_dash_and_attack_dash_state() {
    let mut game = Match::new(data(), 42).unwrap();
    enter_dash(&mut game);
    hold_neutral(&mut game, 3);
    step(&mut game, buttons(BUTTON_A)); // AttackDash, buffer armed.
    let checkpoint = game.checkpoint();
    let inputs = [buttons(BUTTON_L), buttons(0), buttons(0)];
    let expected: Vec<_> = inputs
        .iter()
        .map(|&input| serde_json::to_vec(&step(&mut game, input)).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(game.state().fighters[0].action, Action::AttackDash);
    assert_eq!(
        game.state().fighters[0].dash.grab_buffer,
        dash_support::BUFFER.dash_buffer_frames
    );
    for (input, expected) in inputs.into_iter().zip(expected) {
        assert_eq!(
            serde_json::to_vec(&step(&mut game, input)).unwrap(),
            expected
        );
    }
}

#[test]
fn invalid_dash_resources_are_rejected() {
    for early in [-1.0, f32::NAN, f32::INFINITY] {
        let mut resource = data();
        resource.rules.dash = Some(DashRules {
            early_frames: early,
            ..dash_support::RULES
        });
        assert!(Match::new(resource, 0).is_err(), "early {early}");
    }
    // Dash rules require the grab shield-grab buffer/limit.
    let mut without_shield_grab = data();
    without_shield_grab.rules.grab.as_mut().unwrap().shield_grab = None;
    assert!(Match::new(without_shield_grab, 0).is_err());

    // A repeat flag is rejected.
    let mut repeat_flagged = data();
    for fighter in &mut repeat_flagged.fighters {
        fighter.dash_attack.as_mut().unwrap().flags[0].repeat_ready = true;
    }
    assert!(Match::new(repeat_flagged, 0).is_err());

    // Mismatched root-motion length is rejected.
    let mut bad_roots = data();
    for fighter in &mut bad_roots.fighters {
        fighter.dash_attack.as_mut().unwrap().root_translations = Some(vec![0.0; 2]);
    }
    assert!(Match::new(bad_roots, 0).is_err());

    // Missing locomotion is rejected.
    let mut no_locomotion = data();
    for fighter in &mut no_locomotion.fighters {
        fighter.locomotion = None;
    }
    assert!(Match::new(no_locomotion, 0).is_err());

    // Dash rules without a per-fighter attack, and vice versa.
    let mut rules_only = data();
    for fighter in &mut rules_only.fighters {
        fighter.dash_attack = None;
    }
    assert!(Match::new(rules_only, 0).is_err());
    let mut attack_only = data();
    attack_only.rules.dash = None;
    assert!(Match::new(attack_only, 0).is_err());
}

#[test]
fn rules_dash_none_keeps_match_new_working_and_the_original_dash_behaviour() {
    let mut without = data();
    without.rules.dash = None;
    for fighter in &mut without.fighters {
        fighter.dash_attack = None;
    }
    let mut game = Match::new(without, 42).unwrap();
    let state = step(&mut game, stick(0, [1.0, 0.0]));
    assert_eq!(state.fighters[0].action, Action::Dash);
    assert_eq!(state.fighters[0].action_frame, 1);
}
