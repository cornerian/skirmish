//! End-to-end ordinary shield branches in an explicitly synthetic native world.
use skirmish::game::data::HitElement;
use skirmish::game::{
    Action, BUTTON_A, BUTTON_L, BUTTON_X, Controller, Event, Match, State, data::MatchData, shield,
};

#[derive(serde::Deserialize)]
struct Profile {
    rules: shield::Rules,
    attributes: shield::Attributes,
}

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    let profile: Profile = serde_json::from_str(include_str!("fixtures/game/shield.json")).unwrap();
    data.rules.shield = Some(profile.rules);
    data.rules.countdown_frames = 0;
    data.rules.time_limit_frames = 9999;
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    for fighter in &mut data.fighters {
        fighter.shield = Some(profile.attributes.clone());
        fighter.locomotion =
            Some(serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap());
    }
    data
}
fn held() -> Controller {
    Controller {
        buttons: BUTTON_L,
        ..Default::default()
    }
}
fn input(attacker: u16, defender: Controller) -> [Controller; 2] {
    [
        Controller {
            buttons: attacker,
            ..Default::default()
        },
        defender,
    ]
}
fn step(game: &mut Match, attacker: u16, defender: Controller) -> State {
    game.step(input(attacker, defender)).unwrap().clone()
}
fn shield_hit(data: MatchData) -> Match {
    let mut game = Match::new(data, 42).unwrap();
    step(&mut game, BUTTON_A, held());
    let contact = step(&mut game, 0, held());
    assert!(
        contact
            .events
            .iter()
            .any(|e| matches!(e, Event::ShieldHit { .. })),
        "{contact:?}"
    );
    game
}

fn powershield_data() -> MatchData {
    let mut data = data();
    let rules = data.rules.shield.as_mut().unwrap();
    rules.powershield_input_window = 3;
    rules.powershield_reflect_frames = 3.0;
    rules.powershield_frames = 2.0;
    rules.push_multiplier = 0.25;
    for fighter in &mut data.fighters {
        fighter.shield.as_mut().unwrap().raise_frames = 10.0;
    }
    data
}

fn inert_data() -> MatchData {
    let mut data = data();
    for frame in &mut data.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.element = HitElement::Inert;
        }
    }
    data
}
fn until(
    game: &mut Match,
    input: [Controller; 2],
    limit: usize,
    condition: impl Fn(&State) -> bool,
) -> State {
    for _ in 0..limit {
        if condition(game.state()) {
            return game.state().clone();
        }
        game.step(input).unwrap();
    }
    assert!(
        condition(game.state()),
        "condition not reached: {:?}",
        game.state()
    );
    game.state().clone()
}

#[test]
fn raising_holding_latched_release_and_regeneration_form_a_complete_cycle() {
    let mut game = Match::new(data(), 42).unwrap();
    assert_eq!(
        step(&mut game, 0, held()).fighters[1].action,
        Action::GuardOn
    );
    let release = step(&mut game, 0, Controller::default());
    assert_eq!(release.fighters[1].action, Action::GuardOn);
    assert!(release.fighters[1].shield.release_latched);
    // Repress cannot cancel the release latched inside the minimum hold timer.
    step(&mut game, 0, held());
    assert_eq!(
        step(&mut game, 0, held()).fighters[1].action,
        Action::GuardOff
    );
    let health = game.state().fighters[1].shield.health;
    let waiting = until(&mut game, input(0, Controller::default()), 10, |s| {
        s.fighters[1].action == Action::Wait
    });
    assert!(waiting.fighters[1].shield.health > health);
    assert!(waiting.fighters[1].shield.health <= 50.0);
}

#[test]
fn analog_strength_changes_health_decay_and_releasing_keeps_previous_strength() {
    let mut hard = Match::new(data(), 42).unwrap();
    let mut light = Match::new(data(), 42).unwrap();
    let analog = Controller {
        trigger: 0.2,
        ..Default::default()
    };
    for _ in 0..3 {
        step(&mut hard, 0, held());
        step(&mut light, 0, analog);
    }
    assert_eq!(hard.state().fighters[1].shield.strength, 1.0);
    assert_eq!(light.state().fighters[1].shield.strength, 0.0);
    assert!(light.state().fighters[1].shield.health > hard.state().fighters[1].shield.health);
    let released = step(&mut hard, 0, Controller::default());
    assert_eq!(released.fighters[1].shield.strength, 1.0);
}

#[test]
fn blocked_hit_reduces_only_shield_health_freezes_stun_then_pushes_both_players() {
    let mut game = shield_hit(data());
    let contact = game.state().clone();
    assert_eq!(contact.fighters[1].action, Action::GuardSetOff);
    assert_eq!(contact.fighters[1].percent, 0.0);
    assert_eq!(contact.fighters[1].shield.health, 39.8);
    assert!(contact.fighters.iter().all(|f| f.hitlag > 0.0));
    let position = contact.fighters.each_ref().map(|f| f.position);
    while game.state().fighters[1].hitlag > 0.0 {
        let state = step(&mut game, 0, held());
        assert_eq!(state.fighters.each_ref().map(|f| f.position), position);
        assert_eq!(state.fighters[1].shield.stun_progress, 0.0);
        assert!(state.events.is_empty());
    }
    let pushed = step(&mut game, 0, held());
    assert!(pushed.fighters[0].position[0] < position[0][0]);
    assert!(pushed.fighters[1].position[0] > position[1][0]);
    assert_eq!(pushed.fighters[1].percent, 0.0);
    for _ in 0..20 {
        assert!(
            !step(&mut game, 0, held())
                .events
                .iter()
                .any(|e| matches!(e, Event::ShieldHit { .. }))
        );
    }
    assert_eq!(game.state().fighters[1].action, Action::Guard);
}

#[test]
fn powershield_entry_blocks_health_loss_but_keeps_hitlag_stun_and_recoil() {
    let mut powershield = Match::new(powershield_data(), 42).unwrap();
    let entry = step(&mut powershield, BUTTON_A, held());
    assert_eq!(entry.fighters[1].action, Action::GuardReflect);
    assert!(entry.fighters[1].shield.reflecting);
    assert!(entry.fighters[1].shield.powershield);
    assert_eq!(entry.fighters[1].locomotion.trigger_age, 254);
    let checkpoint = powershield.checkpoint();
    let contact = step(&mut powershield, 0, held());
    assert_eq!(contact.fighters[1].action, Action::GuardSetOff);
    assert_eq!(contact.fighters[1].percent, 0.0);
    assert_eq!(contact.fighters[1].shield.health, 49.8);
    assert!(contact.fighters.iter().all(|fighter| fighter.hitlag > 0.0));
    assert!(contact.events.iter().any(|event| matches!(
        event,
        Event::ShieldHit {
            attacker: 0,
            victim: 1,
            damage: 0.0,
            broken: false
        }
    )));

    let mut ordinary_data = powershield_data();
    ordinary_data
        .rules
        .shield
        .as_mut()
        .unwrap()
        .powershield_input_window = 0;
    let ordinary = shield_hit(ordinary_data);
    assert!(ordinary.state().fighters[1].shield.health < contact.fighters[1].shield.health);
    assert!(
        contact.fighters[1].ground_velocity.abs()
            > ordinary.state().fighters[1].ground_velocity.abs()
    );
    assert_eq!(
        contact.fighters[0].shield.attacker_push,
        ordinary.state().fighters[0].shield.attacker_push
    );

    let expected = serde_json::to_vec(powershield.state()).unwrap();
    powershield.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        serde_json::to_vec(&step(&mut powershield, 0, held())).unwrap(),
        expected
    );
    until(&mut powershield, input(0, held()), 40, |state| {
        !state.fighters[1].shield.reflecting && !state.fighters[1].shield.powershield
    });
}

#[test]
fn analog_guard_can_be_powershielded_only_inside_both_native_entry_windows() {
    let analog = Controller {
        trigger: 0.3,
        ..Default::default()
    };
    let digital = Controller {
        trigger: 0.3,
        buttons: BUTTON_L,
        ..Default::default()
    };
    let mut early = Match::new(powershield_data(), 42).unwrap();
    assert_eq!(
        step(&mut early, 0, analog).fighters[1].action,
        Action::GuardOn
    );
    assert_eq!(
        step(&mut early, 0, digital).fighters[1].action,
        Action::GuardReflect
    );

    let mut late = Match::new(powershield_data(), 42).unwrap();
    step(&mut late, 0, analog);
    for _ in 0..3 {
        step(&mut late, 0, analog);
    }
    let state = step(&mut late, 0, digital);
    assert_eq!(state.fighters[1].action, Action::GuardOn);
    assert!(!state.fighters[1].shield.powershield);
}

#[test]
fn held_cstick_up_jumps_from_shield_with_native_priority_and_release_short_hop() {
    let up = Controller {
        buttons: BUTTON_L,
        cstick: [0.0, 0.8],
        ..Default::default()
    };
    let mut game = Match::new(data(), 42).unwrap();
    assert_eq!(step(&mut game, 0, up).fighters[1].action, Action::GuardOn);
    let jumped = step(&mut game, 0, up);
    assert_eq!(jumped.fighters[1].action, Action::JumpSquat);
    assert_eq!(
        jumped.fighters[1].locomotion.jump_input,
        skirmish::game::locomotion::JumpInput::CStick
    );
    assert!(!jumped.fighters[1].short_hop);
    let checkpoint = game.checkpoint();
    let released = step(
        &mut game,
        0,
        Controller {
            buttons: BUTTON_L,
            ..Default::default()
        },
    );
    assert!(released.fighters[1].short_hop);
    let expected = serde_json::to_vec(&released).unwrap();
    game.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        serde_json::to_vec(&step(
            &mut game,
            0,
            Controller {
                buttons: BUTTON_L,
                ..Default::default()
            },
        ))
        .unwrap(),
        expected
    );

    for (controller, expected) in [
        (
            Controller {
                buttons: BUTTON_L,
                stick: [0.0, 1.0],
                cstick: [0.0, 1.0],
                ..Default::default()
            },
            skirmish::game::locomotion::JumpInput::Stick,
        ),
        (
            Controller {
                buttons: BUTTON_L | BUTTON_X,
                cstick: [0.0, 1.0],
                ..Default::default()
            },
            skirmish::game::locomotion::JumpInput::Buttons,
        ),
    ] {
        let mut game = Match::new(data(), 42).unwrap();
        step(&mut game, 0, held());
        let state = step(&mut game, 0, controller);
        assert_eq!(state.fighters[1].action, Action::JumpSquat);
        assert_eq!(state.fighters[1].locomotion.jump_input, expected);
    }

    let mut ordinary = Match::new(data(), 42).unwrap();
    let no_shield = step(
        &mut ordinary,
        0,
        Controller {
            cstick: [0.0, 1.0],
            ..Default::default()
        },
    );
    assert_eq!(no_shield.fighters[1].action, Action::Wait);
}

#[test]
fn inert_hitboxes_signal_shield_touch_without_damage_stun_or_body_contact() {
    let mut shielded = Match::new(inert_data(), 42).unwrap();
    step(&mut shielded, BUTTON_A, held());
    let touch = step(&mut shielded, 0, held());
    assert!(touch.fighters[1].shield.touched);
    assert_eq!(touch.fighters[1].percent, 0.0);
    assert_eq!(touch.fighters[1].shield.health, 49.8);
    assert_eq!(touch.fighters[1].action, Action::GuardOn);
    assert_eq!(touch.fighters[1].hitlag, 0.0);
    assert_eq!(touch.fighters[0].hit_groups, 0);
    assert!(
        touch
            .events
            .iter()
            .all(|event| !matches!(event, Event::Hit { .. } | Event::ShieldHit { .. }))
    );
    let checkpoint = shielded.checkpoint();
    let expected = serde_json::to_vec(&step(&mut shielded, 0, held())).unwrap();
    shielded.restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(
        serde_json::to_vec(&step(&mut shielded, 0, held())).unwrap(),
        expected
    );
    until(&mut shielded, input(0, held()), 30, |state| {
        !state.fighters[1].shield.touched
    });

    let mut unshielded = Match::new(inert_data(), 42).unwrap();
    step(&mut unshielded, BUTTON_A, Controller::default());
    let overlap = step(&mut unshielded, 0, Controller::default());
    assert!(!overlap.fighters[1].shield.touched);
    assert_eq!(overlap.fighters[1].percent, 0.0);
    assert!(
        overlap
            .events
            .iter()
            .all(|event| !matches!(event, Event::Hit { .. } | Event::ShieldHit { .. }))
    );
}

#[test]
fn shield_stun_blocks_jump_and_attack_until_its_animation_finishes() {
    let mut game = shield_hit(data());
    until(&mut game, input(0, held()), 20, |s| {
        s.fighters[1].hitlag == 0.0
    });
    let blocked = step(
        &mut game,
        0,
        Controller {
            buttons: BUTTON_A | BUTTON_X | BUTTON_L,
            ..Default::default()
        },
    );
    assert_eq!(blocked.fighters[1].action, Action::GuardSetOff);
    until(&mut game, input(0, held()), 30, |s| {
        s.fighters[1].action == Action::Guard
    });
    let jump = step(
        &mut game,
        0,
        Controller {
            buttons: BUTTON_L | BUTTON_X,
            ..Default::default()
        },
    );
    assert_eq!(jump.fighters[1].action, Action::JumpSquat);
}

#[test]
fn depleted_shield_can_be_poked_where_full_shield_blocks() {
    fn profile() -> MatchData {
        let mut d = data();
        d.rules.shield.as_mut().unwrap().drain_rate = 1.0;
        d.fighters[1].bones[1].translation[1] = 3.0;
        d.fighters[1].hurtboxes[0].bone = 0;
        d.fighters[1].hurtboxes[0].start = [0.0, 0.0, 0.0];
        d.fighters[1].hurtboxes[0].end = [0.0, 1.8, 0.0];
        d.fighters[1].shield.as_mut().unwrap().initial_radius = 2.5;
        d
    }
    let mut full = shield_hit(profile());
    assert_eq!(full.state().fighters[1].percent, 0.0);
    // A separate input history drains health without ever damaging the fighter.
    let mut small = Match::new(profile(), 42).unwrap();
    for _ in 0..40 {
        step(&mut small, 0, held());
    }
    step(&mut small, BUTTON_A, held());
    let poke = step(&mut small, 0, held());
    assert!(
        poke.events
            .iter()
            .any(|e| matches!(e, Event::Hit { victim: 1, .. })),
        "{poke:?}"
    );
    assert_eq!(poke.fighters[1].percent, 10.0);
    // Keep full alive through freeze too; this must remain a blocked hit.
    step(&mut full, 0, held());
    assert_eq!(full.state().fighters[1].percent, 0.0);
}

#[test]
fn shield_break_launch_landing_stand_and_mash_recovery_preserve_health_and_stocks() {
    let mut d = data();
    d.rules.shield.as_mut().unwrap().maximum_health = 5.0;
    let mut game = shield_hit(d);
    assert!(
        game.state()
            .events
            .iter()
            .any(|e| matches!(e, Event::ShieldHit { broken: true, .. }))
    );
    assert_eq!(game.state().fighters[1].action, Action::ShieldBreakFly);
    assert!(!game.state().fighters[1].grounded);
    assert_eq!(game.state().fighters[1].percent, 0.0);
    until(&mut game, input(0, Controller::default()), 80, |s| {
        s.fighters[1].action == Action::ShieldBreakDown
    });
    until(&mut game, input(0, Controller::default()), 10, |s| {
        s.fighters[1].action == Action::ShieldBreakStand
    });
    until(&mut game, input(0, Controller::default()), 10, |s| {
        s.fighters[1].action == Action::Furafura
    });
    let checkpoint = game.checkpoint();
    let first = step(
        &mut game,
        0,
        Controller {
            buttons: BUTTON_X,
            stick: [1.0, 0.0],
            ..Default::default()
        },
    );
    assert_eq!(first.fighters[1].shield.dizzy_timer, 19.0);
    let held_frame = step(
        &mut game,
        0,
        Controller {
            buttons: BUTTON_X,
            stick: [1.0, 0.0],
            ..Default::default()
        },
    );
    assert_eq!(held_frame.fighters[1].shield.dizzy_timer, 18.0);
    let mut mashed = 0;
    while game.state().fighters[1].action == Action::Furafura {
        let side = if mashed % 2 == 0 { -1.0 } else { 1.0 };
        step(
            &mut game,
            0,
            Controller {
                stick: [side, 0.0],
                ..Default::default()
            },
        );
        mashed += 1;
    }
    assert_eq!(game.state().fighters[1].stocks, 2);
    // Furafura ends in Anim; the newly installed Wait IASA sees this frame's
    // stick and starts Dash before physics, matching the source scheduler.
    assert_eq!(game.state().fighters[1].action, Action::Dash);
    game.restore_checkpoint(&checkpoint).unwrap();
    for _ in 0..mashed + 2 {
        step(&mut game, 0, Controller::default());
    }
    assert_eq!(game.state().fighters[1].action, Action::Furafura);
}

#[test]
fn exact_zero_damage_health_does_not_break_but_negative_health_does() {
    let mut d = data();
    let r = d.rules.shield.as_mut().unwrap();
    r.maximum_health = 10.0;
    r.drain_rate = 0.0;
    let game = shield_hit(d.clone());
    assert_eq!(game.state().fighters[1].shield.health, 0.0);
    assert_eq!(game.state().fighters[1].action, Action::GuardSetOff);
    for frame in &mut d.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.shield_damage = 1;
        }
    }
    assert_eq!(
        shield_hit(d).state().fighters[1].action,
        Action::ShieldBreakFly
    );
}

#[test]
fn shield_contacts_use_existing_staling_but_do_not_record_a_new_queue_entry() {
    let mut d = data();
    d.rules.knockback_speed = 0.0;
    d.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.2, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        debug_bypass: false,
    });
    for fighter in &mut d.fighters {
        fighter.jab.move_id = Some(10);
    }
    let blocked = shield_hit(d.clone());
    assert_eq!(blocked.state().fighters[0].staling.queue.next(), 0);
    let mut game = Match::new(d, 42).unwrap();
    step(&mut game, BUTTON_A, Controller::default());
    assert_eq!(
        step(&mut game, 0, Controller::default()).fighters[1].percent,
        10.0
    );
    until(&mut game, input(0, Controller::default()), 100, |s| {
        s.fighters
            .iter()
            .all(|f| f.action == Action::Wait && f.hitlag == 0.0)
    });
    assert_eq!(game.state().fighters[0].staling.queue.next(), 1);
    step(&mut game, BUTTON_A, held());
    let stale_shield = step(&mut game, 0, held());
    assert!(
        stale_shield
            .events
            .iter()
            .any(|event| matches!(event,Event::ShieldHit{damage,..}if *damage==8.0))
    );
    assert_eq!(stale_shield.fighters[0].staling.queue.next(), 1);
    assert_eq!(stale_shield.fighters[1].percent, 10.0);
}

#[test]
fn passive_drain_breaks_only_below_zero_and_zero_damage_has_no_stun_callback() {
    let mut d = data();
    let r = d.rules.shield.as_mut().unwrap();
    r.maximum_health = 3.0;
    r.drain_rate = 1.0;
    let mut game = Match::new(d.clone(), 42).unwrap();
    step(&mut game, 0, held());
    for _ in 0..3 {
        step(&mut game, 0, held());
    }
    assert_eq!(game.state().fighters[1].shield.health, 0.0);
    assert_eq!(game.state().fighters[1].action, Action::Guard);
    assert_eq!(
        step(&mut game, 0, held()).fighters[1].action,
        Action::ShieldBreakFly
    );
    d.rules.shield.as_mut().unwrap().drain_rate = 0.0;
    for frame in &mut d.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.damage = 0;
            hit.shield_damage = 5;
        }
    }
    let zero = shield_hit(d);
    assert_eq!(zero.state().fighters[1].action, Action::GuardOn);
    assert_eq!(zero.state().fighters[1].hitlag, 0.0);
    assert_eq!(zero.state().fighters[1].shield.health, 3.0);
}

#[test]
fn shield_hitlag_displacement_is_horizontal_and_uses_shared_input_age() {
    let mut d = data();
    d.rules.damage.displacement = Some(skirmish::game::damage::HitlagDisplacementRules {
        axis_thresholds: [0.3, 0.3],
        minimum_stick_magnitude: 0.5,
        sdi_window: 4,
        sdi_distance: 1.0,
        asdi_distance: 0.5,
    });
    let mut game = shield_hit(d);
    let before = game.state().fighters[1].position;
    let sideways = Controller {
        buttons: BUTTON_L,
        stick: [1.0, 1.0],
        ..Default::default()
    };
    let first = step(&mut game, 0, sideways);
    assert!((first.fighters[1].position[0] - before[0] - 0.5).abs() < 0.000001);
    assert_eq!(first.fighters[1].locomotion.tilt_x_age, 254);
    until(&mut game, input(0, sideways), 20, |s| {
        s.fighters[1].hitlag == 0.0
    });
    let after = game.state().fighters[1].position;
    assert!((after[0] - before[0] - 0.75).abs() < 0.000001);
    assert!((after[1] - before[1]).abs() < 0.0002);
}

#[test]
fn checkpoints_restore_shield_stun_recoil_health_and_release_history() {
    let mut game = shield_hit(data());
    let checkpoint = game.checkpoint();
    let inputs: Vec<_> = (0..30)
        .map(|i| {
            input(
                0,
                if i % 4 == 0 {
                    Controller::default()
                } else {
                    held()
                },
            )
        })
        .collect();
    let expected: Vec<_> = inputs
        .iter()
        .map(|&i| serde_json::to_vec(game.step(i).unwrap()).unwrap())
        .collect();
    game.restore_checkpoint(&checkpoint).unwrap();
    for (input, expected) in inputs.into_iter().zip(expected) {
        assert_eq!(
            serde_json::to_vec(game.step(input).unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn invalid_shield_resources_are_rejected_without_constructing_a_match() {
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let mut d = data();
        d.rules.shield.as_mut().unwrap().drain_rate = value;
        assert!(Match::new(d, 0).is_err());
    }
    let mut d = data();
    d.fighters[0].shield.as_mut().unwrap().bone = 999;
    assert!(Match::new(d, 0).is_err());
    let mut d = data();
    d.rules.shield.as_mut().unwrap().powershield_reflect_frames = f32::NAN;
    assert!(Match::new(d, 0).is_err());
}
