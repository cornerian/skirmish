//! Whole preserved C callbacks, with explicit entity and floor environments.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
#[path = "support/nudge.rs"]
mod support;

use proptest::prelude::*;
use skirmish::fighter::nudge::{self, Body, Error, Neighbors, Rules};
use support::{body, floor, rules};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_nudge(
        count: u32,
        subject: u32,
        numbers: *const f32,
        metadata: *const i32,
        neighbors: *const i32,
        rules: *const f32,
        output: *mut f32,
    );
}

fn flags(body: &mut Body, flags: u8) {
    body.inactive = flags & 1 != 0;
    body.holds_victim = flags & 2 != 0;
    body.nudge_disabled = flags & 4 != 0;
    body.hitlag = flags & 8 != 0;
    body.overlap_disabled = flags & 16 != 0;
}

fn compare(bodies: &[Body], neighbors: &[Neighbors], rules: &Rules) {
    assert!(bodies.len() <= 16);
    let numbers: Vec<_> = bodies
        .iter()
        .flat_map(|b| {
            b.position.into_iter().chain(b.deferred_position).chain([
                b.facing,
                b.center_offset,
                b.half_width,
            ])
        })
        .collect();
    let metadata: Vec<_> = bodies
        .iter()
        .flat_map(|b| {
            [
                i32::from(b.player_id),
                b.floor.map_or(-1, |i| i as i32),
                b.follower_of.map_or(-1, |i| i as i32),
                i32::from(b.inactive)
                    | (i32::from(b.holds_victim) << 1)
                    | (i32::from(b.nudge_disabled) << 2)
                    | (i32::from(b.hitlag) << 3)
                    | (i32::from(b.overlap_disabled) << 4),
            ]
        })
        .collect();
    let neighbors_c: Vec<_> = neighbors
        .iter()
        .flat_map(|n| {
            [
                n.previous.map_or(-1, |i| i as i32),
                n.next.map_or(-1, |i| i as i32),
            ]
        })
        .collect();
    let coefficients = [
        rules.horizontal_step,
        rules.depth_step,
        rules.depth_limit,
        rules.follower_depth_step,
        rules.follower_depth_limit,
    ];
    for (subject, body) in bodies.iter().enumerate() {
        let mut expected = [123.0; 5];
        // SAFETY: flattened buffers match the adapter's row widths; valid
        // references and bounded world sizes are constructed by these tests.
        unsafe {
            oracle_nudge(
                bodies.len() as u32,
                subject as u32,
                numbers.as_ptr(),
                metadata.as_ptr(),
                neighbors_c.as_ptr(),
                coefficients.as_ptr(),
                expected.as_mut_ptr(),
            )
        };
        assert_eq!(
            body.effective_position().map(f32::to_bits),
            expected[2..]
                .iter()
                .copied()
                .map(f32::to_bits)
                .collect::<Vec<_>>()
                .as_slice()
        );
        let actual = nudge::velocity(subject, bodies, neighbors, rules);
        if expected[..2].iter().all(|v| v.is_finite())
            && bodies
                .iter()
                .all(|b| b.effective_position().into_iter().all(f32::is_finite))
        {
            assert_eq!(
                actual.unwrap().map(f32::to_bits),
                [expected[0].to_bits(), expected[1].to_bits()],
                "subject {subject}, bodies={bodies:?}, rules={rules:?}"
            );
        } else {
            assert_eq!(actual, Err(Error::NonFinite));
        }
    }
}

#[test]
fn all_eligibility_flags_and_follower_owner_branches_match_original_c() {
    for a in 0..32 {
        for b in 0..32 {
            for x in [-2.0, -1.0, 0.0, 1.0, 2.0] {
                let mut bodies = [body(0, 0.0), body(1, x), body(0, 0.5)];
                flags(&mut bodies[0], a);
                flags(&mut bodies[1], b);
                bodies[2].follower_of = Some(0);
                compare(&bodies, &floor(), &rules());
                // The owner's holds-victim flag does not exclude its follower
                // bias; the same flag does exclude ordinary targets.
                bodies[0].holds_victim = true;
                bodies[1].floor = None;
                compare(&bodies, &floor(), &rules());
            }
        }
    }
}

#[test]
fn strict_ties_signed_zeros_and_depth_overshoot_order_match_original_c() {
    for mask in 0..4096 {
        let mut bodies = [body(0, 0.0), body(1, 0.0)];
        for (i, value) in bodies
            .iter_mut()
            .flat_map(|b| b.position.iter_mut().chain(&mut b.deferred_position))
            .enumerate()
        {
            *value = if mask & (1 << i) != 0 { -0.0 } else { 0.0 };
        }
        let mut coefficients = rules();
        coefficients.horizontal_step = if mask & 1 != 0 { -0.0 } else { 0.0 };
        coefficients.depth_step = if mask & 2 != 0 { -0.0 } else { 0.0 };
        coefficients.depth_limit = if mask & 4 != 0 { -0.0 } else { 0.0 };
        compare(&bodies, &floor(), &coefficients);
    }
    for depth in [-10.0, -0.5, -0.25, -0.0, 0.0, 0.25, 0.5, 10.0] {
        for step in [0.0, 0.125, 0.25, 1.0, 20.0] {
            for limit in [0.0, 0.125, 0.5, 5.0] {
                let mut b = body(0, 0.0);
                b.position[2] = depth;
                let r = Rules {
                    depth_step: step,
                    depth_limit: limit,
                    ..rules()
                };
                compare(&[b], &floor(), &r);
            }
        }
    }
}

#[test]
fn finite_input_overflow_is_explicit_instead_of_publishing_nonfinite_nudges() {
    let bodies = [body(0, 0.0), body(1, 1.0), body(2, 1.0)];
    compare(
        &bodies,
        &floor(),
        &Rules {
            horizontal_step: f32::MAX,
            ..rules()
        },
    );
    let mut overflow = body(0, f32::MAX);
    overflow.deferred_position[0] = f32::MAX;
    compare(&[overflow], &floor(), &rules());
}

proptest! {
    #![proptest_config(ProptestConfig { rng_seed: proptest::test_runner::RngSeed::Fixed(0x7e0e4), ..ProptestConfig::with_cases(4096) })]
    #[test]
    fn ordered_worlds_and_resolved_floor_neighbors_match_c(
        rows in prop::collection::vec((prop::array::uniform3(-8_f32..8_f32), prop::array::uniform3(-4_f32..4_f32), any::<bool>(), -3_f32..3_f32, 0_f32..4_f32, 0u8..4, 0u8..4, 0u8..64), 1..9),
        links in prop::array::uniform6(0u8..4),
        coefficients in prop::array::uniform5(0_f32..4_f32),
    ) {
        let mut bodies: Vec<_> = rows.into_iter().map(|(position, deferred_position, left, center_offset, half_width, player_id, floor, mask)| {
            let mut b = body(player_id, 0.0);
            b.position = position; b.deferred_position = deferred_position;
            b.facing = if left { -1.0 } else { 1.0 };
            b.center_offset = center_offset; b.half_width = half_width;
            b.floor = (floor < 3).then_some(usize::from(floor));
            flags(&mut b, mask);
            if mask & 32 != 0 { b.follower_of = Some(0); }
            b
        }).collect();
        bodies[0].follower_of = None;
        let owner_player = bodies[0].player_id;
        for b in &mut bodies[1..] { if b.follower_of.is_some() { b.player_id = owner_player; } }
        let neighbors = core::array::from_fn::<_,3,_>(|i| Neighbors { previous: (links[2*i]<3).then_some(usize::from(links[2*i])), next: (links[2*i+1]<3).then_some(usize::from(links[2*i+1])) });
        let [horizontal_step,depth_step,depth_limit,follower_depth_step,follower_depth_limit] = coefficients;
        compare(&bodies, &neighbors, &Rules { horizontal_step,depth_step,depth_limit,follower_depth_step,follower_depth_limit });
    }
}
