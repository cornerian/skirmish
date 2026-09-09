//! Explicit synthetic kernel compositions, not an implemented match scheduler.
use skirmish::fighter::{
    clank::{self, Fighter, Hit, ReboundRules, Response, Rules, Victim, Victims},
    combat, stale,
};

fn fighters(damage: [f32; 2]) -> [Fighter; 2] {
    core::array::from_fn(|side| Fighter {
        id: side as u32 + 1,
        grounded: true,
        x: side as f32 * 4.0 - 2.0,
        hits: core::array::from_fn(|slot| Hit {
            enabled: slot == 0,
            group: slot as u32,
            damage: damage[side],
            clank: true,
            rebound: true,
            hits_grounded: true,
            ..Default::default()
        }),
        response: Response::default(),
    })
}
fn rules() -> Rules {
    Rules {
        damage_gap: 9,
        duration_scale: 0.5,
        duration_base: 2.0,
    }
}

#[test]
fn staling_changes_priority_and_clashes_feed_own_hitlag_and_rebound_without_percent_damage() {
    let mut fresh = fighters([20.0, 10.0]);
    let mut mask = [true; 4];
    assert!(clank::eligible(&fresh, [0, 0]));
    assert!(!clank::resolve_pair(&mut fresh, [0, 0], &mut mask, &rules()).unwrap());
    assert_eq!(fresh[0].response.damage, 0);
    assert_eq!(fresh[1].response.damage, 10);
    let mut queue = stale::Queue::default();
    queue.record(
        stale::Entry {
            move_id: 10,
            attack_instance: 1,
        },
        false,
    );
    let mut penalties = [0.0; 9];
    penalties[0] = 0.5;
    let cached = queue.damage(
        10,
        20.0,
        &stale::Rules {
            penalties,
            debug_bypass: false,
        },
    );
    let mut repeated = fighters([cached, 10.0]);
    let checkpoint = repeated.clone();
    assert!(clank::resolve_pair(&mut repeated, [0, 0], &mut [true; 4], &rules()).unwrap());
    assert_eq!(repeated.each_ref().map(|f| f.response.damage), [10; 2]);
    assert!(!clank::eligible(&repeated, [0, 0]));
    for (side, f) in repeated.iter().enumerate() {
        assert_eq!(f.response.rebound_duration, 7.0);
        assert_eq!(
            combat::hitlag(
                f.response.damage,
                false,
                1.0,
                &combat::HitlagRules {
                    damage_scale: 0.2,
                    base: 2.0,
                    crouch_multiplier: 1.0
                }
            )
            .unwrap(),
            4.0
        );
        let rebound = clank::rebound(
            f.response.rebound_duration,
            f.response.towards,
            &ReboundRules {
                animation_length: 13.9,
                push_scale: 0.2,
                push_base: 0.6,
                surface_friction_multiplier: 0.5,
            },
        )
        .unwrap();
        assert_eq!(rebound.animation_rate, 2.0);
        assert_eq!(rebound.impulse, if side == 0 { -2.0 } else { 2.0 });
        assert_eq!(
            rebound.ground_acceleration,
            if side == 0 { -1.0 } else { 1.0 }
        );
    }
    assert_eq!(queue.next(), 1, "clashes do not write the stale queue");
    let mut restored = checkpoint;
    clank::resolve_pair(&mut restored, [0, 0], &mut [true; 4], &rules()).unwrap();
    assert_eq!(
        restored, repeated,
        "all response and group history is restorable"
    );
}

#[test]
fn scan_order_suppresses_groups_but_stronger_slots_can_continue() {
    let mut pair = fighters([20.0, 10.0]);
    pair[1].hits[1] = Hit {
        enabled: true,
        damage: 25.0,
        group: 1,
        ..pair[1].hits[0]
    };
    pair[1].hits[2] = Hit {
        enabled: true,
        group: 0,
        clank: false,
        ..pair[1].hits[0]
    };
    pair[1].hits[3] = Hit {
        enabled: false,
        group: 0,
        ..pair[1].hits[0]
    };
    let mut mask = [true; 4];
    assert!(!clank::resolve_pair(&mut pair, [0, 0], &mut mask, &rules()).unwrap());
    assert_eq!(mask, [false, true, false, true]);
    assert!(pair[1].hits[2].victims.contains(1));
    assert!(!pair[1].hits[3].victims.contains(1));
    assert!(clank::eligible(&pair, [0, 1]));
    assert!(clank::resolve_pair(&mut pair, [0, 1], &mut mask, &rules()).unwrap());
    assert_eq!(pair[0].response.damage, 20);
    assert_eq!(pair[1].response.damage, 25);
    assert_eq!(mask, [false, false, false, true]);
}

#[test]
fn rebound_flag_and_prior_response_are_independent_of_suppression() {
    let mut pair = fighters([0.5, 0.5]);
    pair[0].hits[0].rebound = false;
    pair[0].response = Response {
        damage: 0,
        rebound_duration: 7.0,
        towards: -0.0,
    };
    pair[1].response = Response {
        damage: 1,
        rebound_duration: 12.0,
        towards: 0.25,
    };
    clank::resolve_pair(&mut pair, [0, 0], &mut [true; 4], &rules()).unwrap();
    assert_eq!(pair[0].response.damage, 1);
    assert_eq!(pair[0].response.rebound_duration, 7.0);
    assert_eq!(pair[0].response.towards.to_bits(), (-0.0f32).to_bits());
    assert_eq!(
        pair[1].response,
        Response {
            damage: 1,
            rebound_duration: 12.0,
            towards: 0.25
        }
    );
    assert!(pair.iter().all(|f| f.hits[0].victims.contains(3 - f.id)));
    for field in 0..4 {
        let mut rejected = fighters([10.0; 2]);
        match field {
            0 => rejected[0].grounded = false,
            1 => rejected[0].hits[0].enabled = false,
            2 => rejected[0].hits[0].clank = false,
            _ => rejected[0].hits[0].hits_grounded = false,
        }
        assert!(!clank::eligible(&rejected, [0, 0]));
    }
}

#[test]
fn victim_ring_uses_holes_then_wraps_and_duplicates_preserve_timer() {
    let mut entries = core::array::from_fn(|i| Victim {
        id: i as u32 + 1,
        remaining: 99,
    });
    entries[3].id = 0;
    let mut victims = Victims::from_parts(entries, 11).unwrap();
    assert!(!victims.record(2).unwrap());
    assert_eq!(victims.entries()[1].remaining, 99);
    assert!(victims.record(20).unwrap());
    assert_eq!(victims.next(), 11);
    assert_eq!(
        victims.entries()[3],
        Victim {
            id: 20,
            remaining: 0
        }
    );
    victims.record(21).unwrap();
    assert_eq!(victims.next(), 0);
    assert_eq!(victims.entries()[11].id, 21);
    victims.record(22).unwrap();
    assert_eq!(victims.next(), 1);
    assert_eq!(victims.entries()[0].id, 22);
}

#[test]
fn undefined_numeric_inputs_and_overflow_preserve_all_outputs() {
    for invalid in [f32::NAN, f32::INFINITY, -1.0, 2_147_483_648.0] {
        let mut pair = fighters([invalid, 10.0]);
        let before = pair.clone();
        let mut mask = [true; 4];
        assert!(clank::resolve_pair(&mut pair, [0, 0], &mut mask, &rules()).is_err());
        assert_eq!(
            pair[0].hits[0].damage.to_bits(),
            before[0].hits[0].damage.to_bits()
        );
        assert_eq!(pair[0].response, before[0].response);
        assert_eq!(pair[1], before[1]);
        assert_eq!(mask, [true; 4]);
    }
    let mut pair = fighters([10.0; 2]);
    let before = pair.clone();
    let mut mask = [true; 4];
    assert!(
        clank::resolve_pair(
            &mut pair,
            [0, 0],
            &mut mask,
            &Rules {
                duration_scale: f32::MAX,
                ..rules()
            }
        )
        .is_err()
    );
    assert_eq!(pair, before);
    assert_eq!(mask, [true; 4]);
    assert!(Victims::default().record(0).is_err());
}
