//! Synthetic end-to-end damage timing; no character/common-data defaults.
use skirmish::game::{Action, BUTTON_A, Controller, Event, Match, data::MatchData};

const IDLE: [Controller; 2] = [Controller {
    cstick: [0.0; 2],
    trigger: 0.0,
    buttons: 0,
    stick: [0.0; 2],
}; 2];

fn data() -> MatchData {
    let mut data: MatchData =
        serde_json::from_str(include_str!("fixtures/game/integration-match.json")).unwrap();
    data.rules.countdown_frames = 0;
    data
}

fn hit(data: MatchData) -> Match {
    let mut game = Match::new(data, 42).unwrap();
    let mut attack = IDLE;
    attack[0].buttons = BUTTON_A;
    game.step(attack).unwrap();
    game.step(IDLE).unwrap();
    assert!(game.state().events.iter().any(|event| matches!(
        event,
        Event::Hit {
            attacker: 0,
            victim: 1,
            ..
        }
    )));
    game
}

#[test]
fn zero_knockback_ticks_damage_without_forcing_any_reaction() {
    // `ftCo_8008EC90` (the ordinary ground/air damage-reaction entry,
    // `ftCo_Damage.c:838`): `if (fp->x2220_b3 || fp->x2220_b4 ||
    // !fp->dmg.kb_applied) { inlineB2(gobj); return; }` skips its entire
    // motion-state transition (`ftCo_8008E908` -> `ftCo_8008DCE0`) whenever
    // the computed knockback is exactly zero -- Fox/Falco's own Blaster
    // laser is a real example (zero growth/base/weight-independent,
    // `docs/fox-neutral-special.md`), confirmed against `fox-bf.slp`: P1
    // takes a Blaster hit mid-JumpSquat and stays in JumpSquat, uninterrupted,
    // only its damage percent ticking up (`docs/parity.md`). The attacker's
    // own last-move-landed/combo-count bookkeeping (`combo::record`) is a
    // separate, always-unconditional mechanism (the attacker's own
    // accounting, set independently of whether the victim reacts), so it
    // still updates; the victim's own `last_hit_by` and both fighters' own
    // hitlag, however, are part of the same skipped reaction.
    let mut resource = data();
    for frame in &mut resource.fighters[0].jab.frames {
        for hitbox in &mut frame.hitboxes {
            hitbox.growth = 0;
            hitbox.fixed = 0;
            hitbox.base = 0;
        }
    }
    let game = hit(resource);
    let attacker = &game.state().fighters[0];
    let victim = &game.state().fighters[1];
    assert_eq!(victim.action, Action::Wait);
    assert_eq!(victim.percent, 10.0);
    assert_eq!(victim.velocity, [0.0, 0.0]);
    assert_eq!(victim.hitstun, 0);
    assert_eq!(victim.hitlag, 0.0);
    assert_eq!(victim.combo.last_hit_by, None);
    assert_eq!(attacker.hitlag, 0.0);
    assert_eq!(attacker.combo.last_attack_landed, 1);
    assert_eq!(attacker.combo.count, 1);
}

#[test]
fn di_uses_the_final_hitlag_input_once_and_checkpoint_retains_pending_work() {
    let game = hit(data());
    let checkpoint = game.checkpoint();
    let original = &game.state().fighters[1];
    assert!(original.di_pending);
    assert_eq!(original.damage_elapsed, 0);
    let mut up = game.clone();
    let mut down = game.clone();
    let mut input_up = IDLE;
    input_up[1].stick[1] = 1.0;
    let mut input_down = IDLE;
    input_down[1].stick[1] = -1.0;
    let ticks = original.hitlag as usize;
    for tick in 0..ticks {
        // Contrary inputs before the final tick must have no DI effect.
        up.step(if tick + 1 == ticks {
            input_up
        } else {
            input_down
        })
        .unwrap();
        down.step(if tick + 1 == ticks {
            input_down
        } else {
            input_up
        })
        .unwrap();
        for branch in [&up, &down] {
            assert_eq!(branch.state().fighters[1].position, original.position);
            assert_eq!(branch.state().fighters[1].damage_elapsed, 0);
            if tick + 1 < ticks {
                assert_eq!(
                    branch.state().fighters[1].knockback.map(f32::to_bits),
                    original.knockback.map(f32::to_bits)
                );
            }
        }
    }
    let up_velocity = up.state().fighters[1].knockback;
    let down_velocity = down.state().fighters[1].knockback;
    assert!(up_velocity[1] > down_velocity[1]);
    assert!(up_velocity[0] < down_velocity[0]);
    assert!(!up.state().fighters[1].di_pending);
    assert!(!down.state().fighters[1].di_pending);
    let expected = skirmish::fighter::damage::decay_air_knockback(
        up_velocity,
        up.data().rules.knockback_decay,
    );
    up.step(input_up).unwrap();
    assert_eq!(
        up.state().fighters[1].knockback.map(f32::to_bits),
        expected.map(f32::to_bits)
    );
    assert!(up.state().fighters[1].damage_elapsed > 0);
    up.restore_checkpoint(&checkpoint).unwrap();
    assert!(up.state().fighters[1].di_pending);
    for _ in 0..ticks {
        up.step(input_down).unwrap();
    }
    assert_eq!(
        up.state().fighters[1].knockback.map(f32::to_bits),
        down_velocity.map(f32::to_bits)
    );
}

#[test]
fn zero_hitlag_has_no_di_exit_callback_and_361_uses_ground_or_air_coefficients() {
    let mut no_lag = data();
    no_lag.rules.hitlag.base = 0.0;
    no_lag.rules.hitlag.damage_scale = 0.0;
    let no_lag = hit(no_lag);
    assert!(!no_lag.state().fighters[1].di_pending);
    assert_eq!(no_lag.state().fighters[1].hitlag, 0.0);

    let mut ground_data = data();
    for frame in &mut ground_data.fighters[0].jab.frames {
        for hitbox in &mut frame.hitboxes {
            hitbox.angle_degrees = 361.0;
        }
    }
    let ground = hit(ground_data.clone());
    let mut air_data = ground_data;
    air_data.stage.spawns[1][1] = 1.5;
    let air = hit(air_data);
    let ground_velocity = ground.state().fighters[1].knockback;
    let air_velocity = air.state().fighters[1].knockback;
    let ground_angle = libm::atan2f(ground_velocity[1], ground_velocity[0]);
    let air_angle = libm::atan2f(air_velocity[1], air_velocity[0]);
    let expected_ground =
        ground.data().rules.damage.angle_361_grounded_max_degrees * f32::from_bits(0x3c8e_fa35);
    assert!((ground_angle - expected_ground).abs() < 0.000001);
    assert!((air_angle - air.data().rules.damage.angle_361_airborne_radians).abs() < 0.000001);
    assert!((ground_angle - air_angle).abs() > 0.01);
}
