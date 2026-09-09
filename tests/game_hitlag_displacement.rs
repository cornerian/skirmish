//! Synthetic native scheduler cases; arithmetic parity uses separate upstream C
//! oracles in the root damage_differential test. No authentic resources implied.
use skirmish::game::{
    BUTTON_A, Controller, Event, Match, State,
    damage::{Armor, HitlagDisplacementRules},
    data::MatchData,
};

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data.rules.hitlag.base = 10.0;
    data.rules.knockback_speed = 0.0;
    data.stage.spawns = [[-1.5, 0.0], [1.5, 0.0]];
    data.stage.floor.left = -100.0;
    data.stage.floor.right = 100.0;
    data.stage.blast = [-200.0, 200.0, -200.0, 200.0];
    data.rules.damage.displacement = Some(HitlagDisplacementRules {
        axis_thresholds: [0.5; 2],
        minimum_stick_magnitude: 0.5,
        sdi_window: 3,
        sdi_distance: 2.0,
        asdi_distance: 0.75,
    });
    data
}

fn controls(stick: [f32; 2]) -> [Controller; 2] {
    [
        Controller::default(),
        Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            stick,
            buttons: 0,
        },
    ]
}

fn hit(data: MatchData, prior: [f32; 2]) -> Match {
    let mut game = Match::new(data, 19).unwrap();
    let mut input = controls(prior);
    input[0].buttons = BUTTON_A;
    game.step(input).unwrap();
    for _ in 0..5 {
        game.step(controls(prior)).unwrap();
        if game.state().fighters[1].percent > 0.0 {
            return game;
        }
    }
    panic!("synthetic jab did not connect: {:?}", game.state());
}

fn step(game: &mut Match, stick: [f32; 2]) -> State {
    game.step(controls(stick)).unwrap().clone()
}

fn ages(state: &State) -> [u8; 2] {
    [
        state.fighters[1].locomotion.tilt_x_age,
        state.fighters[1].locomotion.tilt_y_age,
    ]
}

fn bits(state: &State) -> String {
    // f32 -> f64 conversion preserves each finite f32 exactly, including zero sign.
    fn encode(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Number(number) if number.is_f64() => {
                *value = serde_json::Value::String(format!(
                    "{:016x}",
                    number.as_f64().unwrap().to_bits()
                ));
            }
            serde_json::Value::Array(values) => values.iter_mut().for_each(encode),
            serde_json::Value::Object(values) => values.values_mut().for_each(encode),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(state).unwrap();
    encode(&mut value);
    value.to_string()
}

#[test]
fn fresh_threshold_moves_once_holding_does_not_and_reversal_rearms() {
    let mut game = hit(data(), [0.0; 2]);
    let before = game.state().fighters[1].clone();
    assert_eq!(before.locomotion.jumps_used, 1);
    let below = step(&mut game, [f32::from_bits(0.5_f32.to_bits() - 1), 0.0]);
    assert_eq!(below.fighters[1].position, before.position);
    let first = step(&mut game, [0.5, 0.0]);
    assert_eq!(
        first.fighters[1].position,
        [before.position[0] + 1.0, before.position[1]]
    );
    assert_eq!(ages(&first), [254; 2]);
    let held = step(&mut game, [0.5, 0.0]);
    assert_eq!(held.fighters[1].position, first.fighters[1].position);
    let reverse = step(&mut game, [-0.5, 0.0]);
    assert_eq!(reverse.fighters[1].position, before.position);
    step(&mut game, [0.0; 2]);
    let reentered = step(&mut game, [0.5, 0.0]);
    assert_eq!(reentered.fighters[1].position, first.fighters[1].position);
    assert_eq!(reentered.fighters[1].action_frame, before.action_frame);
    assert_eq!(reentered.fighters[1].hitstun, before.hitstun);
    assert!(reentered.fighters[1].hitlag > 0.0);
}

#[test]
fn magnitude_threshold_and_exclusive_input_age_window_are_independent() {
    let mut resource = data();
    let profile = resource.rules.damage.displacement.as_mut().unwrap();
    profile.axis_thresholds = [0.25; 2];
    profile.minimum_stick_magnitude = 0.75;
    profile.sdi_window = 2;
    let original = hit(resource, [0.0; 2]);
    let position = original.state().fighters[1].position;
    let mut early = original.clone();
    let first = step(&mut early, [0.5, 0.0]);
    assert_eq!(first.fighters[1].position, position);
    assert_eq!(ages(&first), [0, 254]);
    let qualifies = step(&mut early, [0.75, 0.0]);
    assert_eq!(qualifies.fighters[1].position[0], position[0] + 1.5);
    let mut late = original;
    step(&mut late, [0.5, 0.0]);
    step(&mut late, [0.5, 0.0]);
    let expired = step(&mut late, [0.75, 0.0]);
    assert_eq!(expired.fighters[1].position, position);
    assert_eq!(ages(&expired), [2, 254]);
}

#[test]
fn held_before_damage_is_not_fresh_and_attacker_hitlag_has_no_damage_callback() {
    let mut game = hit(data(), [0.0, 1.0]);
    let before = game
        .state()
        .fighters
        .each_ref()
        .map(|fighter| fighter.position);
    let mut input = controls([0.0, 1.0]);
    input[0].stick = [1.0, 1.0];
    let next = game.step(input).unwrap();
    assert_eq!(
        next.fighters.each_ref().map(|fighter| fighter.position),
        before
    );
    assert_eq!(ages(next), [254; 2]);
    assert!(next.fighters[1].di_pending);
    assert!(!next.fighters[0].di_pending);
}

#[test]
fn final_tick_applies_asdi_even_when_held_and_does_not_also_apply_fresh_sdi() {
    let mut game = hit(data(), [0.0; 2]);
    while game.state().fighters[1].hitlag > 1.0 {
        step(&mut game, [1.0, 0.0]);
    }
    let checkpoint = game.checkpoint();
    let before = game.state().fighters[1].clone();
    let held = step(&mut game, [1.0, 0.0]);
    assert_eq!(held.fighters[1].position[0], before.position[0] + 0.75);
    assert_eq!(held.fighters[1].hitlag, 0.0);
    assert!(!held.fighters[1].di_pending);
    assert_eq!(held.fighters[1].hitstun, before.hitstun);
    game.restore_checkpoint(&checkpoint).unwrap();
    let fresh = step(&mut game, [-1.0, 0.0]);
    assert_eq!(fresh.fighters[1].position[0], before.position[0] - 0.75);
    // The input callback still samples the final frozen tick. ASDI does not
    // consume the fresh timer as OnEveryHitlag would have done.
    assert_eq!(ages(&fresh), [0, 254]);
    let next = step(&mut game, [-1.0, 0.0]);
    assert_eq!(next.fighters[1].position[0], fresh.fighters[1].position[0]);
}

#[test]
fn upward_sdi_moves_both_coordinates_and_downward_sdi_resolves_the_floor() {
    let original = hit(data(), [0.0; 2]);
    let mut upward = original.clone();
    let before = upward.state().fighters[1].position;
    let above = step(&mut upward, [0.5, 0.5]);
    assert_eq!(
        above.fighters[1].position,
        [before[0] + 1.0, before[1] + 1.0]
    );
    assert!(!above.fighters[1].grounded);
    let mut downward = original;
    let below = step(&mut downward, [0.5, -0.5]);
    assert_eq!(below.fighters[1].position[0], before[0] + 1.0);
    assert!(below.fighters[1].position[1] >= 0.0);
    assert!(below.fighters[1].grounded);
    assert!(below.fighters[1].ground_line.is_some());
    assert!(below.fighters[1].hitlag > 0.0);
    let grounded_up = step(&mut downward, [0.5, 0.5]);
    assert!(grounded_up.fighters[1].grounded);
    assert_eq!(
        grounded_up.fighters[1].position[1],
        below.fighters[1].position[1]
    );
}

#[test]
fn checkpoint_restores_unconsumed_window_previous_stick_and_full_suffix_bits() {
    let mut resource = data();
    let profile = resource.rules.damage.displacement.as_mut().unwrap();
    profile.axis_thresholds = [0.25; 2];
    profile.minimum_stick_magnitude = 0.75;
    let mut game = hit(resource, [0.0; 2]);
    step(&mut game, [0.5, 0.0]);
    assert_eq!(ages(game.state()), [0, 254]);
    let checkpoint = game.checkpoint();
    let script = [
        [0.75, 0.0],
        [0.75, 0.0],
        [0.0, 0.0],
        [-1.0, 0.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [0.0, 1.0],
        [0.0, 0.0],
    ];
    let expected = script.map(|stick| bits(&step(&mut game, stick)));
    game.restore_checkpoint(&checkpoint).unwrap();
    for (stick, expected) in script.into_iter().zip(expected) {
        assert_eq!(bits(&step(&mut game, stick)), expected);
    }
    game.restore_checkpoint(&checkpoint).unwrap();
    let mut branch = game.clone();
    let moved = step(&mut game, [0.75, 0.0]);
    let still = step(&mut branch, [0.5, 0.0]);
    assert_ne!(moved.fighters[1].position, still.fighters[1].position);
    assert_eq!(branch.state().rng_seed, game.state().rng_seed);
}

fn effective_knockback(state: &State) -> f32 {
    state
        .events
        .iter()
        .find_map(|event| match event {
            Event::Hit { knockback, .. } => Some(*knockback),
            _ => None,
        })
        .expect("setup produces hit event")
}

#[test]
fn armor_uses_maximum_channel_then_minimum_without_reducing_percent() {
    let baseline = hit(data(), [0.0; 2]);
    let raw = effective_knockback(baseline.state());
    assert!(raw > 8.0);
    for channels in [[8.0, 2.0], [2.0, 8.0], [8.0, 8.0]] {
        let mut resource = data();
        resource.fighters[1].armor = Some(Armor {
            armor0: channels[0],
            armor1: channels[1],
            minimum_knockback: 1.0,
        });
        let armored = hit(resource, [0.0; 2]);
        assert_eq!(effective_knockback(armored.state()), raw - 8.0);
        assert_eq!(
            armored.state().fighters[1].percent,
            baseline.state().fighters[1].percent
        );
        assert!(armored.state().fighters[1].hitstun < baseline.state().fighters[1].hitstun);
    }
    let mut resource = data();
    resource.fighters[1].armor = Some(Armor {
        armor0: raw + 1.0,
        armor1: 0.0,
        minimum_knockback: 2.0,
    });
    assert_eq!(effective_knockback(hit(resource, [0.0; 2]).state()), 2.0);
    let mut zero = data();
    for frame in &mut zero.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.base = 0;
            hit.growth = 0;
        }
    }
    zero.fighters[1].armor = Some(Armor {
        armor0: 10.0,
        armor1: 1.0,
        minimum_knockback: 5.0,
    });
    assert_eq!(effective_knockback(hit(zero, [0.0; 2]).state()), 0.0);
}

#[test]
fn crouch_scales_only_the_victims_hitlag_and_damage_consumes_the_ground_jump() {
    let mut resource = data();
    resource.rules.hitlag.crouch_multiplier = 0.5;
    let parameters: skirmish::game::locomotion::Parameters =
        serde_json::from_str(include_str!("fixtures/game/locomotion.json")).unwrap();
    let crouch_frames = parameters.crouch_animation_frames;
    resource
        .rules
        .damage
        .displacement
        .as_mut()
        .unwrap()
        .axis_thresholds = [
        parameters.horizontal_smash_deadzone,
        parameters.vertical_smash_deadzone,
    ];
    resource.fighters[1].locomotion = Some(parameters);
    let mut game = Match::new(resource, 19).unwrap();
    for _ in 0..crouch_frames + 2 {
        step(&mut game, [0.0, -1.0]);
    }
    assert_eq!(
        game.state().fighters[1].action,
        skirmish::game::Action::SquatWait
    );
    assert_eq!(game.state().fighters[1].locomotion.jumps_used, 0);
    let mut attack = controls([0.0, -1.0]);
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    let contact = step(&mut game, [0.0, -1.0]);
    assert!(contact.fighters[1].percent > 0.0);
    // Synthetic hitlag = trunc(10 damage * 0.1 + 10 base) = 11;
    // victim crouch then truncates 11 * 0.5 to 5. Attacker stays at 11.
    assert_eq!(contact.fighters[0].hitlag, 11.0);
    assert_eq!(contact.fighters[1].hitlag, 5.0);
    assert_eq!(contact.fighters[1].locomotion.jumps_used, 1);
    assert!(contact.fighters[1].di_pending);
}

#[test]
fn invalid_profiles_fail_and_unusable_displacement_rolls_back_all_hitlag_history() {
    for (field, value) in [
        ("sdi_window", serde_json::json!(255)),
        ("axis_thresholds", serde_json::json!([0.0, 0.5])),
        ("sdi_distance", serde_json::json!(-1.0)),
    ] {
        let mut resource = serde_json::to_value(data()).unwrap();
        resource["rules"]["damage"]["displacement"][field] = value;
        let resource = serde_json::from_value(resource).unwrap();
        assert!(Match::new(resource, 0).is_err());
    }
    let mut resource = data();
    resource.fighters[1].armor = Some(Armor {
        armor0: -1.0,
        armor1: 0.0,
        minimum_knockback: 0.0,
    });
    assert!(Match::new(resource, 0).is_err());
    let mut resource = data();
    resource
        .rules
        .damage
        .displacement
        .as_mut()
        .unwrap()
        .sdi_distance = 1_000_000.0;
    let mut game = hit(resource, [0.0; 2]);
    let before = bits(game.state());
    let error = game.step(controls([1.0, 0.0])).unwrap_err();
    assert!(error.to_string().contains("subdivision"), "{error}");
    assert_eq!(bits(game.state()), before);
    step(&mut game, [0.0; 2]);
}
