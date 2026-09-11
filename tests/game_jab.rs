//! End-to-end jab combos (the first/second/third chain, the buffered
//! follow-up from Wait, interruptible-pose smash/tilt/jab priority, the
//! rapid jab's entry count/loop/exit, root motion and checkpoint restore) in
//! an explicitly synthetic native world.
#[path = "support/grab.rs"]
mod grab_support;
#[path = "support/jab.rs"]
mod jab_support;
#[path = "support/smash.rs"]
mod smash_support;
#[path = "support/tilt.rs"]
mod tilt_support;

use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, BUTTON_Z, Controller, Match, State, data::MatchData,
    jab::Stage, shield,
};

#[derive(serde::Deserialize)]
struct ShieldProfile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

/// Fighter 0 (facing +X at -2) attacks; fighter 1 (facing -X at +20) idles
/// out of reach so hitlag never freezes the attacker's samples.
fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: ShieldProfile =
        serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.rules.knockback_speed = 0.0;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [20.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
        fighter.jab.move_id = Some(10);
    }
    jab_support::profile(smash_support::profile(tilt_support::profile(
        grab_support::profile(data),
    )))
}

/// Both fighters within the jab hitboxes' reach.
fn close() -> MatchData {
    let mut data = data();
    data.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
    data.rules.knockback_speed = 0.15;
    data
}

fn staling(mut data: MatchData) -> MatchData {
    data.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02, 0.01],
        debug_bypass: false,
    });
    data
}

fn input(buttons: u16, stick: [f32; 2]) -> Controller {
    Controller {
        buttons,
        stick,
        ..Default::default()
    }
}

fn step(game: &mut Match, attacker: Controller) -> State {
    step_both(game, attacker, Controller::default())
}

fn step_both(game: &mut Match, attacker: Controller, defender: Controller) -> State {
    game.step([attacker, defender]).unwrap().clone()
}

/// press, release, press, hold: reaches the second jab's entry pose. A
/// release (count 1) then a fresh press (count 2) latches the follow-up
/// without firing it (follow_up_ready isn't raised until pose 3); holding
/// through pose 3 (where it is) fires the latch. Three presses/releases
/// would instead reach the rapid window, but this sequence only reaches
/// two before the hold, so the ordinary follow-up wins.
fn to_second_jab(game: &mut Match) -> State {
    step(game, input(BUTTON_A, [0.0, 0.0]));
    step(game, input(0, [0.0, 0.0]));
    step(game, input(BUTTON_A, [0.0, 0.0]));
    step(game, input(BUTTON_A, [0.0, 0.0]))
}

/// Continues from `to_second_jab` with a release then a fresh press, which
/// fires the third jab the moment the second jab's own follow-up flag
/// (pose 2) is reached.
fn to_third_jab(game: &mut Match) -> State {
    to_second_jab(game);
    step(game, input(0, [0.0, 0.0]));
    step(game, input(BUTTON_A, [0.0, 0.0]))
}

#[test]
fn first_jab_from_wait_then_the_second_and_third_follow_up() {
    // A fresh press from Wait starts the first jab; the entry pose's script
    // commands apply immediately, so the rapid flag is already raised.
    let mut game = Match::new(data(), 42).unwrap();
    let entry = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(entry.fighters[0].action, Action::Jab);
    assert_eq!(entry.fighters[0].action_frame, 1);
    assert_eq!(entry.fighters[0].jab.last, Some(Stage::First));
    assert_eq!(entry.fighters[0].jab.window, 6.0);
    assert!(entry.fighters[0].jab.rapid);

    // A release (count 1), then a fresh press (count 2) on an
    // uninterruptible pose latches the follow-up without firing it yet.
    let released = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(released.fighters[0].action_frame, 2);
    assert!(!released.fighters[0].jab.buffered);
    let latched = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(latched.fighters[0].action, Action::Jab);
    assert_eq!(latched.fighters[0].action_frame, 3);
    assert!(latched.fighters[0].jab.buffered);

    // Holding through pose 3 (allow_interrupt and follow_up_ready both
    // raised there) fires the latched follow-up; the second jab's own entry
    // pose turns the rapid flag back off.
    let second = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(second.fighters[0].action, Action::Attack12);
    assert_eq!(second.fighters[0].action_frame, 1);
    assert_eq!(second.fighters[0].jab.last, Some(Stage::Second));
    assert_eq!(second.fighters[0].jab.window, 6.0);
    assert!(!second.fighters[0].jab.buffered);
    assert!(!second.fighters[0].jab.rapid);

    // A release then a fresh press fires the third the moment the second
    // jab's own follow-up flag (pose 2) is reached.
    let released2 = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(released2.fighters[0].action, Action::Attack12);
    let third = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(third.fighters[0].action, Action::Attack13);
    assert_eq!(third.fighters[0].action_frame, 1);
    // doAttack13 (unlike checkAttack11 and doAttack12Normal) never writes
    // unk_msid, so the remembered jab stays Second through the third jab.
    assert_eq!(third.fighters[0].jab.last, Some(Stage::Second));

    // The third jab has no follow-up: it plays out to Wait on its own.
    let mut last = third;
    for _ in 0..7 {
        last = step(&mut game, input(0, [0.0, 0.0]));
    }
    assert_eq!(last.fighters[0].action, Action::Wait);
}

#[test]
fn follow_up_reaches_from_wait_inside_the_window_and_expires() {
    // Let the first jab play out to Wait; the window (still positive because
    // Wait keeps it) accepts a follow-up press there.
    let mut game = Match::new(data(), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    let mut waited = step(&mut game, input(0, [0.0, 0.0]));
    let mut in_wait_after = 0;
    while waited.fighters[0].action != Action::Wait {
        waited = step(&mut game, input(0, [0.0, 0.0]));
        in_wait_after += 1;
    }
    assert!(waited.fighters[0].jab.window > 0.0, "{in_wait_after}");
    let followed = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(followed.fighters[0].action, Action::Attack12);
    assert_eq!(followed.fighters[0].action_frame, 1);

    // Once the window decays to zero without a press, the same press instead
    // starts a fresh first jab.
    let mut expiring = Match::new(data(), 42).unwrap();
    step(&mut expiring, input(BUTTON_A, [0.0, 0.0]));
    let mut state = step(&mut expiring, input(0, [0.0, 0.0]));
    while state.fighters[0].action != Action::Wait {
        state = step(&mut expiring, input(0, [0.0, 0.0]));
    }
    while state.fighters[0].jab.window > 0.0 {
        state = step(&mut expiring, input(0, [0.0, 0.0]));
    }
    assert_eq!(state.fighters[0].jab.window, 0.0);
    let fresh = step(&mut expiring, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(fresh.fighters[0].action, Action::Jab);
    assert_eq!(fresh.fighters[0].jab.last, Some(Stage::First));

    // A press after any other motion (JumpSquat here) resets the timer to
    // zero immediately, rather than letting it decay gradually as Wait does.
    let mut jumped = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut jumped);
    let squat = step(&mut jumped, input(BUTTON_X, [0.0, 0.0]));
    assert_eq!(squat.fighters[0].action, Action::JumpSquat);
    assert_eq!(squat.fighters[0].jab.window, 0.0);

    // Reaching that same zeroed window from a different, jab-eligible motion
    // (crouch-rise) gives a fresh first jab rather than a follow-up.
    let mut resetting = Match::new(data(), 42).unwrap();
    step(&mut resetting, input(BUTTON_A, [0.0, 0.0]));
    let mut state = step(&mut resetting, input(0, [0.0, 0.0]));
    while state.fighters[0].action != Action::Wait {
        state = step(&mut resetting, input(0, [0.0, 0.0]));
    }
    assert!(state.fighters[0].jab.window > 0.0);
    for _ in 0..7 {
        state = step(&mut resetting, input(0, [0.0, -1.0]));
    }
    assert_eq!(state.fighters[0].action, Action::SquatWait);
    assert_eq!(state.fighters[0].jab.window, 0.0);
    let rising = step(&mut resetting, input(0, [0.0, 0.0]));
    assert_eq!(rising.fighters[0].action, Action::SquatRv);
    let fresh_from_squat = step(&mut resetting, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(fresh_from_squat.fighters[0].action, Action::Jab);
    assert_eq!(fresh_from_squat.fighters[0].jab.last, Some(Stage::First));
}

/// Drive the fighter to the first jab's interruptible pose (3) with no press
/// pending, so the next input exercises the interruptible chain alone.
fn to_interruptible_jab(game: &mut Match) {
    step(game, input(BUTTON_A, [0.0, 0.0]));
    for _ in 0..2 {
        step(game, input(0, [0.0, 0.0]));
    }
}

#[test]
fn attack11_and_attack12_chains_prioritize_smash_then_tilt_then_locomotion() {
    let mut jumping = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut jumping);
    let jumped = step(&mut jumping, input(BUTTON_X, [0.0, 0.0]));
    assert_eq!(jumped.fighters[0].action, Action::JumpSquat);

    let mut dashing = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut dashing);
    let dashed = step(&mut dashing, input(0, [1.0, 0.0]));
    assert_eq!(dashed.fighters[0].action, Action::Dash);

    let mut smashing = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut smashing);
    let smashed = step(&mut smashing, input(BUTTON_A, [1.0, 0.0]));
    assert_eq!(smashed.fighters[0].action, Action::AttackS4S);

    // A stick aged past the dash window answers as a tilt instead.
    let mut tilting = Match::new(data(), 42).unwrap();
    step(&mut tilting, input(BUTTON_A, [0.0, 0.0]));
    for _ in 0..6 {
        step(&mut tilting, input(0, [0.6, 0.0]));
    }
    let tilted = step(&mut tilting, input(BUTTON_A, [1.0, 0.0]));
    assert_eq!(tilted.fighters[0].action, Action::AttackS3S);

    // L: the jab's chain never reaches shield.
    let mut shielding = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut shielding);
    let held_l = step(&mut shielding, input(BUTTON_L, [0.0, 0.0]));
    assert_eq!(held_l.fighters[0].action, Action::Jab);

    // Z with a neutral stick is the logical A press: it latches the
    // follow-up (no grab chain reached from the jab).
    let mut catching = Match::new(data(), 42).unwrap();
    to_interruptible_jab(&mut catching);
    let zed = step(&mut catching, input(BUTTON_Z, [0.0, 0.0]));
    assert_eq!(zed.fighters[0].action, Action::Attack12);
}

#[test]
fn attack13_interruptible_pose_reaches_the_wait_chain() {
    // Drive through first and second jabs into the third's interruptible
    // pose (5) with no pending press.
    fn to_interruptible_attack13(game: &mut Match) {
        let third = to_third_jab(game);
        assert_eq!(third.fighters[0].action, Action::Attack13);
        for _ in 0..4 {
            step(game, input(0, [0.0, 0.0]));
        }
    }

    let mut catching = Match::new(data(), 42).unwrap();
    to_interruptible_attack13(&mut catching);
    let caught = step(&mut catching, input(BUTTON_Z, [0.0, 0.0]));
    assert_eq!(caught.fighters[0].action, Action::Catch);

    let mut guarding = Match::new(data(), 42).unwrap();
    to_interruptible_attack13(&mut guarding);
    let guarded = step(&mut guarding, input(BUTTON_L, [0.0, 0.0]));
    assert_eq!(guarded.fighters[0].action, Action::GuardOn);

    let mut jabbing = Match::new(data(), 42).unwrap();
    to_interruptible_attack13(&mut jabbing);
    let jabbed = step(&mut jabbing, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(jabbed.fighters[0].action, Action::Jab);
    assert_eq!(jabbed.fighters[0].jab.last, Some(Stage::First));
}

#[test]
fn rapid_jab_counts_fresh_presses_and_releases_into_the_loop() {
    // The rapid flag is already raised at entry (the entry pose's script
    // commands apply immediately). ftCo_Attack_800D6A50 runs from the jab's
    // own IASA chain, not from the Wait entry check, so the press that
    // starts the first jab is not itself counted; every fresh press or
    // release from then on is, and it wins over the ordinary follow-up
    // because the rapid check runs first each frame.
    let mut game = Match::new(staling(data()), 42).unwrap();
    let entry = step(&mut game, input(BUTTON_A, [0.0, 0.0])); // enters Jab; uncounted
    assert_eq!(entry.fighters[0].jab.presses, 0);
    assert!(entry.fighters[0].jab.rapid);
    let released = step(&mut game, input(0, [0.0, 0.0])); // fresh release: 1
    assert_eq!(released.fighters[0].jab.presses, 1);
    let pressed = step(&mut game, input(BUTTON_A, [0.0, 0.0])); // fresh press: 2
    assert_eq!(pressed.fighters[0].jab.presses, 2);
    assert_eq!(pressed.fighters[0].action, Action::Jab);
    let started = step(&mut game, input(0, [0.0, 0.0])); // fresh release: 3 -> rapid
    assert_eq!(started.fighters[0].action, Action::Attack100Start);
    assert_eq!(started.fighters[0].action_frame, 1);
    assert_eq!(started.fighters[0].jab.presses, 3);
    let start_id = started.fighters[0].action_instance.id;

    // Holding does not add fresh presses or releases: two held frames still
    // read as a single unchanged press count.
    let mut holding = Match::new(data(), 42).unwrap();
    step(&mut holding, input(BUTTON_A, [0.0, 0.0]));
    let a = step(&mut holding, input(BUTTON_A, [0.0, 0.0])).fighters[0]
        .jab
        .presses;
    let b = step(&mut holding, input(BUTTON_A, [0.0, 0.0])).fighters[0]
        .jab
        .presses;
    assert_eq!(a, b);

    // Attack100Start plays out into Attack100Loop, keeping the action
    // instance across the SkipAttackCount transition; the loop's own frame
    // zero then restarts the stale identity and allocates a fresh action
    // instance id (ft_800892A0/ft_80089824, two identity-0 allocations).
    let stale_before = started.fighters[0].staling.identity.attack_instance;
    for _ in 0..2 {
        let in_start = step(&mut game, input(0, [0.0, 0.0]));
        assert_eq!(in_start.fighters[0].action, Action::Attack100Start);
        assert_eq!(in_start.fighters[0].action_instance.id, start_id);
    }
    let looping = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(looping.fighters[0].action, Action::Attack100Loop);
    assert_eq!(looping.fighters[0].action_frame, 1);
    assert_ne!(looping.fighters[0].action_instance.id, start_id);
    assert_ne!(
        looping.fighters[0].staling.identity.attack_instance,
        stale_before
    );

    // No further input at the loop check (pose 3, one full four-pose cycle
    // later) ends the loop into Wait.
    for _ in 0..2 {
        step(&mut game, input(0, [0.0, 0.0]));
    }
    let ended = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(ended.fighters[0].action, Action::Attack100End);
    let mut waited = ended;
    for _ in 0..3 {
        waited = step(&mut game, input(0, [0.0, 0.0]));
    }
    assert_eq!(waited.fighters[0].action, Action::Wait);
}

#[test]
fn rapid_jab_tapping_keeps_the_loop_going_and_rehits_each_cycle() {
    // The victim is in reach, so the connecting jab hitboxes freeze the
    // attacker's own frames in hitlag too; drive by observed action rather
    // than a fixed frame count, and alternate the held button every step so
    // a fresh edge is always queued for whichever frame next runs live.
    // Zero knockback (unlike `close()`) keeps the victim in reach for every
    // cycle instead of drifting off after the first hit.
    let mut nearby = data();
    nearby.stage.spawns = [[-1.0, 0.0], [1.0, 0.0]];
    let mut game = Match::new(nearby, 42).unwrap();
    let mut held = true;
    let mut state = step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    for _ in 0..60 {
        if state.fighters[0].action == Action::Attack100Loop {
            break;
        }
        held = !held;
        state = step(
            &mut game,
            input(if held { BUTTON_A } else { 0 }, [0.0, 0.0]),
        );
    }
    assert_eq!(state.fighters[0].action, Action::Attack100Loop);
    let early_percent = state.fighters[1].percent;
    assert!(early_percent > 0.0);
    for _ in 0..120 {
        held = !held;
        state = step(
            &mut game,
            input(if held { BUTTON_A } else { 0 }, [0.0, 0.0]),
        );
    }
    assert_eq!(state.fighters[0].action, Action::Attack100Loop);
    assert!(
        state.fighters[1].percent > early_percent,
        "{} vs {early_percent}",
        state.fighters[1].percent
    );
}

#[test]
fn second_jab_follows_its_root_motion() {
    let mut game = Match::new(data(), 42).unwrap();
    let started = to_second_jab(&mut game);
    assert_eq!(started.fighters[0].action, Action::Attack12);
    let roots = [0.0_f32, 0.5, 1.0, 0.5, 0.0, 0.0];
    let mut expected = started.fighters[0].position[0];
    for (sample, root) in roots.iter().enumerate().skip(1) {
        let state = step(&mut game, input(0, [0.0, 0.0]));
        expected += root;
        assert_eq!(
            state.fighters[0].action,
            Action::Attack12,
            "sample {sample}"
        );
        assert_eq!(state.fighters[0].position[0], expected, "sample {sample}");
    }

    // The mirrored fighter (facing -1) steps toward -X.
    let mut mirrored = Match::new(data(), 42).unwrap();
    step_both(
        &mut mirrored,
        Controller::default(),
        input(BUTTON_A, [0.0, 0.0]),
    );
    step_both(&mut mirrored, Controller::default(), input(0, [0.0, 0.0]));
    step_both(
        &mut mirrored,
        Controller::default(),
        input(BUTTON_A, [0.0, 0.0]),
    );
    let start = step_both(
        &mut mirrored,
        Controller::default(),
        input(BUTTON_A, [0.0, 0.0]),
    );
    assert_eq!(start.fighters[1].action, Action::Attack12);
    let base = start.fighters[1].position[0];
    for _ in 1..6 {
        step_both(&mut mirrored, Controller::default(), Controller::default());
    }
    let total: f32 = roots.iter().sum();
    assert_eq!(mirrored.state().fighters[1].position[0], base - total);
}

#[test]
fn rapid_false_on_a_later_pose_turns_the_flag_off() {
    // Pose 1 turns the rapid flag the entry pose raised back off, before the
    // count of 3 is reached. Once the count fails to enter the rapid jab at
    // pose 3, the already-buffered press still fires the ordinary follow-up
    // instead (buffered latches on pose 2's press regardless of the flag).
    let mut data = data();
    for fighter in &mut data.fighters {
        fighter.jab_combo.as_mut().unwrap().first.flags[1].rapid = Some(false);
    }
    let mut game = Match::new(data, 42).unwrap();
    let entry = step(&mut game, input(BUTTON_A, [0.0, 0.0])); // enter; rapid true at entry
    assert!(entry.fighters[0].jab.rapid);
    let released = step(&mut game, input(0, [0.0, 0.0])); // release: 1, pose 1 turns rapid off
    assert!(!released.fighters[0].jab.rapid);
    let pressed = step(&mut game, input(BUTTON_A, [0.0, 0.0])); // press: 2, latches
    assert!(!pressed.fighters[0].jab.rapid);
    let state = step(&mut game, input(0, [0.0, 0.0])); // release: 3, rapid still off
    assert_eq!(state.fighters[0].jab.presses, 3);
    assert!(!state.fighters[0].jab.rapid);
    assert_eq!(state.fighters[0].action, Action::Attack12);
}

#[test]
fn clear_hits_lets_third_jabs_group_hit_the_victim_twice() {
    fn build(enable_clear: bool) -> u32 {
        let mut data = close();
        // Neutralize the first and second jabs' hitboxes so only the third
        // jab's pair (2..=3) is in play, keeping the run deterministic up
        // to the moment under test.
        for frame in &mut data.fighters[0].jab.frames {
            frame.hitboxes.clear();
        }
        {
            let combo = data.fighters[0].jab_combo.as_mut().unwrap();
            let second = combo.second.as_mut().unwrap();
            for frame in &mut second.attack.frames {
                frame.hitboxes.clear();
            }
            second.script.root_translations = None;
            let third = combo.third.as_mut().unwrap();
            // Minimize knockback so the victim is still in reach one frame
            // later, isolating the clear_hits question from displacement.
            for frame in &mut third.attack.frames {
                for hit in &mut frame.hitboxes {
                    hit.growth = 0;
                    hit.base = 1;
                }
            }
            if !enable_clear {
                third.script.flags[3].clear_hits = false;
            }
        }
        let mut game = Match::new(data, 42).unwrap();
        let mut state = to_third_jab(&mut game);
        while state.fighters[0].action != Action::Wait {
            state = step(&mut game, input(0, [0.0, 0.0]));
        }
        state.fighters[1].percent as u32
    }
    let without_clear = build(false);
    let with_clear = build(true);
    assert!(without_clear > 0);
    assert!(
        with_clear > without_clear,
        "{with_clear} vs {without_clear}"
    );
}

#[test]
fn presses_persist_into_attack12_and_reset_on_a_new_first_jab() {
    // Reaching Attack12 mid-count keeps the accumulated presses; the second
    // jab's own entry pose turns the rapid flag off, so further presses grow
    // the count without ever reaching a rapid entry.
    let mut game = Match::new(data(), 42).unwrap();
    let started = to_second_jab(&mut game);
    assert_eq!(started.fighters[0].action, Action::Attack12);
    assert_eq!(started.fighters[0].jab.presses, 2);
    assert!(!started.fighters[0].jab.rapid);
    let released = step(&mut game, input(0, [0.0, 0.0])); // fresh release: 3
    assert_eq!(released.fighters[0].action, Action::Attack12);
    assert_eq!(released.fighters[0].jab.presses, 3);
    assert!(!released.fighters[0].jab.rapid);

    // A fresh first-jab entry (via the follow-up window from Wait) resets
    // the press count to zero.
    let mut fresh_entry = Match::new(data(), 42).unwrap();
    let entry = step(&mut fresh_entry, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(entry.fighters[0].jab.presses, 0);
    let mut state = step(&mut fresh_entry, input(0, [0.0, 0.0])); // count 1
    while state.fighters[0].action != Action::Wait {
        state = step(&mut fresh_entry, input(0, [0.0, 0.0]));
    }
    while state.fighters[0].jab.window > 0.0 {
        state = step(&mut fresh_entry, input(0, [0.0, 0.0]));
    }
    let refreshed = step(&mut fresh_entry, input(BUTTON_A, [0.0, 0.0]));
    assert_eq!(refreshed.fighters[0].action, Action::Jab);
    assert_eq!(refreshed.fighters[0].jab.last, Some(Stage::First));
    assert_eq!(refreshed.fighters[0].jab.presses, 0);
}

fn assert_checkpoint_matches(game: &mut Match, phase: &str) {
    let checkpoint = game.checkpoint();
    let inputs: Vec<_> = (0..14)
        .map(|frame| {
            [
                input(
                    if frame % 3 == 0 { BUTTON_A } else { 0 },
                    if frame % 5 == 0 {
                        [1.0, 0.0]
                    } else {
                        [0.0, 0.0]
                    },
                ),
                Controller::default(),
            ]
        })
        .collect();
    let expected: Vec<_> = inputs
        .iter()
        .map(|&i| serde_json::to_vec(game.step(i).unwrap()).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    for (i, exp) in inputs.into_iter().zip(expected) {
        assert_eq!(
            serde_json::to_vec(game.step(i).unwrap()).unwrap(),
            exp,
            "{phase}"
        );
    }
}

#[test]
fn checkpoints_restore_every_jab_phase() {
    let mut game = Match::new(staling(data()), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    assert_checkpoint_matches(&mut game, "first jab, uninterruptible");

    let mut game = Match::new(staling(data()), 42).unwrap();
    to_interruptible_jab(&mut game);
    assert_checkpoint_matches(&mut game, "first jab, interruptible");

    let mut game = Match::new(staling(data()), 42).unwrap();
    let entered_second = to_second_jab(&mut game);
    assert_eq!(entered_second.fighters[0].action, Action::Attack12);
    assert_checkpoint_matches(&mut game, "second jab");

    let mut game = Match::new(staling(data()), 42).unwrap();
    let entered_third = to_third_jab(&mut game);
    assert_eq!(entered_third.fighters[0].action, Action::Attack13);
    assert_checkpoint_matches(&mut game, "third jab");

    // press, release, press, release: three counted events reach the rapid
    // window before the ordinary follow-up (pose 3) is ever checked.
    let mut game = Match::new(staling(data()), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    let entered_start = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(entered_start.fighters[0].action, Action::Attack100Start);
    assert_checkpoint_matches(&mut game, "rapid start");

    let mut game = Match::new(staling(data()), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    let entered_loop = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(entered_loop.fighters[0].action, Action::Attack100Loop);
    assert_checkpoint_matches(&mut game, "rapid loop");

    let mut game = Match::new(staling(data()), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    step(&mut game, input(0, [0.0, 0.0]));
    let entered_end = step(&mut game, input(0, [0.0, 0.0]));
    assert_eq!(entered_end.fighters[0].action, Action::Attack100End);
    assert_checkpoint_matches(&mut game, "rapid end");

    let mut game = Match::new(staling(data()), 42).unwrap();
    step(&mut game, input(BUTTON_A, [0.0, 0.0]));
    let mut state = step(&mut game, input(0, [0.0, 0.0]));
    while state.fighters[0].action != Action::Wait {
        state = step(&mut game, input(0, [0.0, 0.0]));
    }
    assert!(state.fighters[0].jab.window > 0.0);
    assert_checkpoint_matches(&mut game, "wait, window still open");
}

#[test]
fn invalid_jab_resources_are_rejected_without_constructing_a_match() {
    fn rejected(edit: impl FnOnce(&mut MatchData)) {
        let mut resource = data();
        edit(&mut resource);
        assert!(Match::new(resource, 0).is_err());
    }
    assert!(Match::new(data(), 0).is_ok());
    assert!(Match::new(staling(data()), 0).is_ok());
    // `jab_combo = None` keeps the old, chainless behaviour.
    let mut without_combo = data();
    without_combo.fighters[0].jab_combo = None;
    assert!(Match::new(without_combo, 0).is_ok());

    rejected(|d| {
        d.fighters[0].jab_combo.as_mut().unwrap().first.flags.pop();
    });
    rejected(|d| {
        d.fighters[0].jab_combo.as_mut().unwrap().first.flags[0].loop_check = true;
    });
    rejected(|d| {
        d.fighters[0].jab_combo.as_mut().unwrap().second = None;
    });
    rejected(|d| {
        d.fighters[0].jab_combo.as_mut().unwrap().rapid = None;
    });
    rejected(|d| {
        let combo = d.fighters[0].jab_combo.as_mut().unwrap();
        for flags in &mut combo.rapid.as_mut().unwrap().cycle.script.flags {
            flags.loop_check = false;
        }
    });
    rejected(|d| {
        d.fighters[0]
            .jab_combo
            .as_mut()
            .unwrap()
            .attributes
            .rapid_window = -1;
    });
    rejected(|d| {
        d.fighters[0]
            .jab_combo
            .as_mut()
            .unwrap()
            .attributes
            .second_window = f32::NAN;
    });
    rejected(|d| {
        d.fighters[0]
            .jab_combo
            .as_mut()
            .unwrap()
            .second
            .as_mut()
            .unwrap()
            .script
            .root_translations = Some(vec![0.0, 1.0]);
    });
    rejected(|d| {
        d.rules.staling = staling(data()).rules.staling;
        d.fighters[0]
            .jab_combo
            .as_mut()
            .unwrap()
            .rapid
            .as_mut()
            .unwrap()
            .end
            .attack
            .move_id = Some(99);
    });
    rejected(|d| d.fighters[1].locomotion = None);
}
