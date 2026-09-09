//! Native ECB arithmetic/state comparisons; stage callbacks are outside scope.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::collision::ecb::*;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_ecb_apply(
        mode: u32,
        storage: *mut f32,
        flags: *mut u32,
        args: *const f32,
        options: u32,
        extra: *mut f32,
    );
}

fn shape(values: &[f32]) -> Shape {
    Shape {
        top: [values[0], values[1]],
        bottom: [values[2], values[3]],
        left: [values[4], values[5]],
        right: [values[6], values[7]],
    }
}

fn save_shape(shape: Shape, output: &mut [f32]) {
    for (slot, value) in output.iter_mut().zip(
        shape
            .top
            .into_iter()
            .chain(shape.bottom)
            .chain(shape.left)
            .chain(shape.right),
    ) {
        *slot = value;
    }
}

fn check(mode: u32, values: [f32; 49], flags: [bool; 5], args: [f32; 15], options: u32) {
    let mut state = State {
        current: shape(&values[..8]),
        previous: shape(&values[8..16]),
        desired: shape(&values[16..24]),
        before_load: shape(&values[24..32]),
        unsqueezed: shape(&values[32..40]),
        clear_on_load: flags[0],
        bottom_locked: flags[1],
        restore_unsqueezed: flags[2],
        uninitialized: flags[3],
        stop: flags[4],
    };
    let mut actual = values;
    let mut position = [values[40], values[41]];
    let mut extra = [0.0; 4];
    match mode {
        0 => state.desired.normalize(),
        1 => state.load_fixed(
            &FixedSource {
                up: args[0],
                down: args[1],
                front: args[2],
                back: args[3],
                angle: args[4],
            },
            options as i32,
        ),
        2 => state.load_joints(
            core::array::from_fn(|i| [args[i * 2], args[i * 2 + 1]]),
            position,
            &JointParameters {
                side_y_offset: args[12],
                height_threshold: args[13],
                width_threshold: args[14],
            },
            options,
        ),
        3 => state.load_external(shape(&args[..8])),
        4 => state.interpolate(args[0]).unwrap(),
        5 => state.squeeze_horizontal(&mut position, args[0], args[1]),
        6 => state.squeeze_vertical(&mut position, options != 0, args[0], args[1]),
        7 => {
            let b = state.bounds(
                position,
                [values[43], values[44]],
                (options & 4 != 0).then_some(LedgeSnap {
                    horizontal: args[0],
                    vertical: args[1],
                    height: args[2],
                }),
            );
            extra = [b.left, b.bottom, b.right, b.top];
        }
        8 => {
            let p = SubstepPlan::new(
                values[46..49].try_into().unwrap(),
                values[40..43].try_into().unwrap(),
                state.current,
                state.desired,
            )
            .unwrap();
            extra = [p.steps as f32, p.velocity[0], p.velocity[1], p.velocity[2]];
        }
        _ => unreachable!(),
    }
    for (i, s) in [
        state.current,
        state.previous,
        state.desired,
        state.before_load,
        state.unsqueezed,
    ]
    .into_iter()
    .enumerate()
    {
        save_shape(s, &mut actual[i * 8..i * 8 + 8]);
    }
    actual[40] = position[0];
    actual[41] = position[1];
    let actual_flags = [
        state.clear_on_load,
        state.bottom_locked,
        state.restore_unsqueezed,
        state.uninitialized,
        state.stop,
    ]
    .map(u32::from);
    let mut expected = values;
    let mut expected_flags = flags.map(u32::from);
    let mut expected_extra = [0.0; 4];
    // SAFETY: the adapter reads/writes49state floats,5flag integers, reads15
    // arguments, and writes4extra floats. Finite bounded inputs avoid C asserts
    // and undefined float-to-i32 conversions in the count-selection excerpt.
    unsafe {
        oracle_ecb_apply(
            mode,
            expected.as_mut_ptr(),
            expected_flags.as_mut_ptr(),
            args.as_ptr(),
            options,
            expected_extra.as_mut_ptr(),
        );
    }
    assert_eq!(actual_flags, expected_flags, "mode{mode}");
    for (index, (a, e)) in actual
        .into_iter()
        .chain(extra)
        .zip(expected.into_iter().chain(expected_extra))
        .enumerate()
    {
        if mode == 1 && args[4] != 0.0 {
            assert!(
                (a - e).abs() <= 4e-6 * a.abs().max(e.abs()).max(1.0),
                "mode{mode} scalar{index}: {a} != {e}"
            );
        } else {
            assert_eq!(
                a.to_bits(),
                e.to_bits(),
                "mode{mode} scalar{index}: {a} != {e}"
            );
        }
    }
}

#[test]
fn load_modes_flags_rotation_lock_and_clear_match_source() {
    let values = core::array::from_fn(|i| (i as f32 - 15.0) * 0.25);
    for flags in 0..32 {
        let state_flags = core::array::from_fn(|i| flags & (1 << i) != 0);
        let mut args = [0.0; 15];
        args[..5].copy_from_slice(&[7.0, 2.0, 5.0, 3.0, 0.0]);
        for facing in [0, 1, u32::MAX] {
            for angle in [0.0, -0.0, core::f32::consts::FRAC_PI_2, -0.3, 1.1] {
                args[4] = angle;
                check(1, values, state_flags, args, facing);
            }
        }
        args = [
            3.0, 1.0, 4.0, 2.0, 5.0, 3.0, 6.0, 4.0, 7.0, 5.0, 8.0, 6.0, 0.25, 10.0, 10.0,
        ];
        for source_flags in 0..32 {
            check(2, values, state_flags, args, source_flags);
        }
        check(3, values, state_flags, args, 0);
    }
}

#[test]
fn squeeze_restore_interpolation_bounds_and_normalization_edges_match() {
    for scale in [0.0, 0.0001, 0.001, 0.5, 1.0, 1.5, 3.0, 10.0] {
        let values = core::array::from_fn(|i| (i as f32 - 15.0) * scale);
        for mode in [0, 4, 5, 6, 7] {
            for options in [0, 1, 4] {
                let mut args = [0.0; 15];
                args[..3].copy_from_slice(&[0.5, -2.0, 8.0]);
                check(mode, values, [false; 5], args, options);
                check(mode, values, [true; 5], args, options);
            }
        }
    }
}

#[test]
fn subdivision_strict_six_boundary_and_omitted_bottom_growth_match() {
    for movement in [
        0.0, 5.9999995, 6.0, 6.0000005, 11.999999, 12.0, 12.000001, 30.0,
    ] {
        for coordinate in [40, 41, 16 + 1, 16 + 4, 16 + 6, 16 + 7] {
            let mut values = [0.0; 49];
            values[coordinate] = movement;
            check(8, values, [false; 5], [0.0; 15], 0);
        }
    }
    let shape = Shape::default();
    let desired = Shape {
        bottom: [0.0, -1000.0],
        ..shape
    };
    assert_eq!(
        SubstepPlan::new([0.0; 3], [0.0; 3], shape, desired)
            .unwrap()
            .steps,
        1
    );
    assert_eq!(
        SubstepPlan::new([0.0; 3], [12.0, 0.0, 0.0], shape, shape)
            .unwrap()
            .steps,
        3
    );
    assert!(SubstepPlan::new([0.0; 3], [f32::INFINITY, 0.0, 0.0], shape, shape).is_err());
    assert!(SubstepPlan::new([0.0; 3], [f32::MAX, 0.0, 0.0], shape, shape).is_err());
}

#[test]
fn substep_oracle_excerpt_is_unchanged_from_the_pinned_source() {
    let wrapper = include_str!("oracle/mpcoll_plan.inc");
    let block = wrapper
        .split("/* BEGIN VERBATIM SUBSTEP PLAN */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM SUBSTEP PLAN */")
        .next()
        .unwrap();
    assert!(include_str!("oracle/original/mpcoll.c").contains(block));
    assert!(block.contains("steps = steps + 1;"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn generated_ecb_state_transitions_match(
        mode in 0_u32..9,
        values in prop::array::uniform::<_,49>(-20_f32..20_f32),
        flags in any::<[bool;5]>(),
        args in prop::array::uniform::<_,15>(-20_f32..20_f32),
        options in 0_u32..32,
    ) { check(mode,values,flags,args,options); }
}
