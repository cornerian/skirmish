//! Special angle-362 contact geometry checked against pinned source statements.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::fighter::damage::positional_launch;

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_positional_launch(
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        contact_x: f32,
        contact_y: f32,
        direction: *mut f32,
        angle: *mut i32,
    );
}

fn compare(values: [f32; 6]) {
    let [ax, ay, bx, by, contact_x, contact_y] = values;
    let actual = positional_launch([ax, ay, 0.0], [bx, by, 0.0], [contact_x, contact_y, 0.0]);
    let mut direction = 0.0;
    let mut angle = 0;
    // SAFETY: the oracle accepts six scalars and two valid output pointers.
    unsafe {
        oracle_positional_launch(
            ax,
            ay,
            bx,
            by,
            contact_x,
            contact_y,
            &mut direction,
            &mut angle,
        );
    }
    assert_eq!(actual.direction.to_bits(), direction.to_bits());
    assert_eq!(actual.angle_degrees, angle);
}

#[test]
fn branch_and_conversion_are_verbatim_in_pinned_sources() {
    let adapter = include_str!("oracle/positional_angle.c");
    let block = adapter
        .split("/* BEGIN VERBATIM POSITIONAL LAUNCH */\n")
        .nth(1)
        .unwrap()
        .split("/* END VERBATIM POSITIONAL LAUNCH */")
        .next()
        .unwrap();
    assert!(include_str!("oracle/original/combat_knockback.c").contains(block));
    assert!(
        include_str!("oracle/original/sdk_mtx.h")
            .contains("#define MTXRadToDeg(a) ((a) * 57.29577951f)")
    );
}

#[test]
fn quadrants_and_vertical_threshold_match() {
    for values in [
        [0.0, 0.0, 0.0, 0.0, -1.0, -1.0],
        [0.0, 0.0, 0.0, 0.0, 1.0, -1.0],
        [0.0, 0.0, 0.0, 0.0, -1.0, 1.0],
        [0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
        [0.0, 0.00001, 0.0, 0.00001, -0.00001, 0.0],
    ] {
        compare(values);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn arbitrary_finite_geometry_matches(values in any::<[i32; 6]>()) {
        compare(values.map(|value| value as f32 / 4096.0));
    }
}
