//! Host-compiled original stage query bodies; scalar normals intentionally use
//! original C_VECNormalize instead of PSVECNormalize's hardware approximation.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::collision::stage::*;

#[repr(C)]
struct OracleLine {
    start: Point,
    end: Point,
    flags: u32,
    material: u32,
    previous: [i32; 2],
    next: [i32; 2],
}

impl From<&Line> for OracleLine {
    fn from(line: &Line) -> Self {
        Self {
            start: line.start,
            end: line.end,
            flags: line.flags,
            material: u32::from(line.material_flags),
            previous: line.previous.map(index),
            next: line.next.map(index),
        }
    }
}

#[repr(C)]
struct OracleJoint {
    id: i32,
    flags: u32,
    bounds_min: Point,
    bounds_max: Point,
    ranges: [i32; 10],
}

impl From<&Joint> for OracleJoint {
    fn from(joint: &Joint) -> Self {
        let ranges = [
            &joint.floor,
            &joint.ceiling,
            &joint.left_wall,
            &joint.right_wall,
            &joint.dynamic,
        ];
        Self {
            id: joint.id as i32,
            flags: joint.flags,
            bounds_min: joint.bounds_min,
            bounds_max: joint.bounds_max,
            ranges: core::array::from_fn(|i| {
                if i % 2 == 0 {
                    ranges[i / 2].start as i32
                } else {
                    ranges[i / 2].len() as i32
                }
            }),
        }
    }
}

#[repr(C)]
struct OracleContact {
    position: [f32; 3],
    normal: [f32; 3],
    line_id: i32,
    flags: u32,
    callback_count: u32,
    callback_ids: [i32; 128],
}

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_stage_intersection(kind: i32, endpoints: *const f32, out: *mut f32) -> i32;
    fn oracle_stage_endpoints(
        lines: *const OracleLine,
        count: i32,
        id: i32,
        out: *mut f32,
        neighbors: *mut i32,
    );
    fn oracle_stage_project(
        lines: *const OracleLine,
        count: i32,
        kind: i32,
        id: i32,
        point: *const f32,
        delta: *mut f32,
        flags: *mut u32,
        normal: *mut f32,
    ) -> i32;
    fn oracle_stage_query(
        lines: *const OracleLine,
        line_count: i32,
        joints: *const OracleJoint,
        joint_count: i32,
        kind: i32,
        query: *const f32,
        skip: *const i32,
        prechecked: i32,
        accept_mask: u64,
        out: *mut OracleContact,
    ) -> i32;
}

fn index(id: Option<usize>) -> i32 {
    id.map_or(-1, |id| id as i32)
}

fn same(a: f32, b: f32) {
    if b.is_nan() {
        assert!(a.is_nan());
    } else {
        assert_eq!(a.to_bits(), b.to_bits(), "{a:?} != {b:?}");
    }
}

fn intersections(endpoints: [f32; 8]) {
    let [ax, ay, bx, by, cx, cy, dx, dy] = endpoints;
    let actual = [
        line_intersection([ax, ay], [bx, by], [cx, cy], [dx, dy]),
        line_intersection_h([ax, ay], bx, [cx, cy], [dx, dy]),
        line_intersection_v([ax, ay], by, [cx, cy], [dx, dy]),
    ];
    for (kind, actual) in actual.into_iter().enumerate() {
        let mut output = [1234.0, -5678.0];
        // SAFETY: arrays contain all eight inputs/two writable outputs.
        let hit = unsafe {
            oracle_stage_intersection(kind as i32, endpoints.as_ptr(), output.as_mut_ptr())
        } != 0;
        assert_eq!(
            actual.is_some(),
            hit,
            "kind={kind}, endpoints={endpoints:?}"
        );
        if let Some(point) = actual {
            for (a, b) in point.into_iter().zip(output) {
                same(a, b);
            }
        } else {
            assert_eq!(
                output,
                [1234.0, -5678.0],
                "C miss must leave outputs untouched"
            );
        }
    }
}

fn query_reference(
    lines: &[Line],
    joints: &[Joint],
    kind: Surface,
    query: Query,
    mask: u64,
) -> Option<Contact> {
    let stage = Stage::new(lines, joints).unwrap();
    let c_lines: Vec<_> = lines.iter().map(OracleLine::from).collect();
    let c_joints: Vec<_> = joints.iter().map(OracleJoint::from).collect();
    let mut calls = Vec::new();
    let actual = stage
        .sweep_filtered(kind, query, |id| {
            calls.push(id as i32);
            mask & (1 << id) != 0
        })
        .unwrap();
    let mut output = OracleContact {
        position: [1234.0; 3],
        normal: [-5678.0; 3],
        line_id: -100,
        flags: 0xdeadbeef,
        callback_count: 0,
        callback_ids: [0; 128],
    };
    let input = [
        query.from[0],
        query.from[1],
        query.to[0],
        query.to[1],
        query.floor_y_offset,
    ];
    let skip = [
        index(query.skip_line),
        index(query.skip_joint),
        index(query.only_joint),
    ];
    // SAFETY: repr(C) arrays are live for the call, lengths/indices are validated
    // by Stage, generated fixtures fit C's stated 64-line/32-joint limits, and
    // output has the complete writable contact/log structure. C state is TLS.
    let hit = unsafe {
        oracle_stage_query(
            c_lines.as_ptr(),
            c_lines.len() as i32,
            c_joints.as_ptr(),
            c_joints.len() as i32,
            kind.flag() as i32,
            input.as_ptr(),
            skip.as_ptr(),
            i32::from(query.bounding == Bounding::Prechecked),
            mask,
            &mut output,
        )
    } != 0;
    assert_eq!(actual.is_some(), hit, "{kind:?} {query:?}");
    assert_eq!(calls, output.callback_ids[..output.callback_count as usize]);
    if let Some(contact) = actual {
        assert_eq!(contact.line_id as i32, output.line_id);
        assert_eq!(contact.flags, output.flags);
        for (a, b) in contact
            .position
            .into_iter()
            .chain(contact.normal)
            .zip(output.position.into_iter().chain(output.normal))
        {
            same(a, b);
        }
    } else {
        assert_eq!(output.position, [1234.0; 3]);
        assert_eq!(output.normal, [-5678.0; 3]);
        assert_eq!(output.line_id, -100);
        assert_eq!(output.flags, 0xdeadbeef);
    }
    actual
}

fn all_lines(count: usize) -> Joint {
    Joint {
        id: 3,
        flags: ENABLED,
        bounds_min: [-100.0; 2],
        bounds_max: [100.0; 2],
        floor: 0..count,
        ceiling: 0..count,
        left_wall: 0..count,
        right_wall: 0..count,
        dynamic: 0..0,
    }
}

#[test]
fn endpoint_tolerances_zero_segments_and_ieee_values_match_c() {
    let values = [
        0.0,
        -0.0,
        0.0001,
        -0.0001,
        0.1,
        -0.1,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
    ];
    for x in values {
        intersections([-1.0, 0.0, 1.0, 0.0, x, 1.0, x, -1.0]);
        intersections([0.0, -1.0, 0.0, 1.0, -1.0, x, 1.0, x]);
        intersections([0.0, 0.0, 1.0, x, 0.0, x, 1.0, 0.0]);
        intersections([0.0, 0.0, 0.0, 0.0, 0.0, x, 0.0, -x]);
    }
    assert_eq!(
        line_intersection([-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]),
        None
    );
    assert_eq!(
        line_intersection([-1.0, 0.0], [1.0, 0.0], [-0.5, 0.0], [0.5, 0.0]),
        None
    );
}

#[test]
fn each_surface_is_directed_and_first_equal_contact_wins() {
    let cases = [
        (
            Surface::Floor,
            [-2.0, 0.0],
            [2.0, 1.0],
            [0.0, 3.0],
            [0.0, -3.0],
        ),
        (
            Surface::Ceiling,
            [2.0, 1.0],
            [-2.0, 0.0],
            [0.0, -3.0],
            [0.0, 3.0],
        ),
        (
            Surface::LeftWall,
            [0.0, -2.0],
            [1.0, 2.0],
            [-3.0, 0.0],
            [3.0, 0.0],
        ),
        (
            Surface::RightWall,
            [1.0, 2.0],
            [0.0, -2.0],
            [3.0, 0.0],
            [-3.0, 0.0],
        ),
    ];
    for (kind, start, end, from, to) in cases {
        let line = Line {
            start,
            end,
            flags: kind.flag() | ENABLED,
            material_flags: PLATFORM as u16,
            ..Line::default()
        };
        let lines = [line, line];
        let mut joint = all_lines(2);
        joint.id = 5;
        let mut first = joint.clone();
        first.id = 2;
        let joints = [first, joint];
        let query = Query {
            from,
            to,
            ..Query::default()
        };
        let hit = query_reference(&lines, &joints, kind, query, u64::MAX).unwrap();
        assert_eq!((hit.line_id, hit.joint_id), (0, 2));
        assert!(
            query_reference(
                &lines,
                &joints,
                kind,
                Query {
                    from: to,
                    to: from,
                    ..query
                },
                u64::MAX
            )
            .is_none()
        );
    }
}

#[test]
fn floor_callback_flags_offsets_and_dynamic_range_retain_original_order() {
    let base = Line {
        start: [-2.0, 0.0],
        end: [2.0, 0.0],
        flags: FLOOR | ENABLED,
        ..Line::default()
    };
    let lines = [
        Line {
            flags: FLOOR,
            ..base
        },
        Line {
            flags: FLOOR | ENABLED | EMPTY,
            ..base
        },
        Line {
            material_flags: PLATFORM as u16,
            ..base
        },
        Line {
            start: [-2.0, 1.0],
            end: [2.0, 1.0],
            ..base
        },
    ];
    let joint = Joint {
        floor: 0..3,
        dynamic: 3..4,
        ..all_lines(0)
    };
    let query = Query {
        from: [0.0, 3.0],
        to: [0.0, -3.0],
        floor_y_offset: 0.25,
        ..Query::default()
    };
    let hit = query_reference(
        &lines,
        core::slice::from_ref(&joint),
        Surface::Floor,
        query,
        u64::MAX,
    )
    .unwrap();
    assert_eq!((hit.line_id, hit.position[1]), (3, 1.25));
    let hit = query_reference(
        &lines,
        core::slice::from_ref(&joint),
        Surface::Floor,
        query,
        !(1 << 3),
    )
    .unwrap();
    assert_eq!(
        (hit.line_id, hit.flags, hit.position[1]),
        (2, PLATFORM, 0.25)
    );
    assert!(
        query_reference(
            &lines,
            &[joint],
            Surface::Floor,
            Query {
                skip_line: Some(2),
                ..query
            },
            !(1 << 3)
        )
        .is_none()
    );
}

#[test]
fn connected_endpoint_extension_is_asymmetric_and_neighbors_have_strict_distance() {
    let line = Line {
        start: [0.0, 0.0],
        end: [2.0, 0.0],
        flags: FLOOR | ENABLED,
        previous: [Some(1), None],
        next: [Some(1), None],
        ..Line::default()
    };
    let mut lines = [
        line,
        Line {
            start: [4.0, 0.0],
            end: [0.0, 0.0],
            flags: ENABLED,
            ..Line::default()
        },
    ];
    let stage = Stage::new(&lines, &[]).unwrap();
    assert_eq!(
        stage.extended_endpoints(0).unwrap(),
        [[-1.0, 0.0], [3.5, 0.0]]
    );
    for distance in [2.0_f32, 1.9999] {
        lines[0].next = [None, Some(1)];
        lines[1].start = [2.0 + distance, 0.0];
        assert_eq!(
            Stage::new(&lines, &[]).unwrap().neighbor(0, true).unwrap(),
            (distance < 2.0).then_some(1)
        );
        compare_endpoints(&lines);
    }
    lines[1].flags |= HIDDEN;
    compare_endpoints(&lines);
}

fn compare_endpoints(lines: &[Line]) {
    let stage = Stage::new(lines, &[]).unwrap();
    let c_lines: Vec<_> = lines.iter().map(OracleLine::from).collect();
    for id in 0..lines.len() {
        let mut output = [0.0; 4];
        let mut neighbors = [-1; 2];
        // SAFETY: valid repr(C) lines and validated links; output arrays have
        // four writable endpoint floats and two writable neighbor indices.
        unsafe {
            oracle_stage_endpoints(
                c_lines.as_ptr(),
                c_lines.len() as i32,
                id as i32,
                output.as_mut_ptr(),
                neighbors.as_mut_ptr(),
            )
        };
        assert_eq!(index(stage.neighbor(id, false).unwrap()), neighbors[0]);
        assert_eq!(index(stage.neighbor(id, true).unwrap()), neighbors[1]);
        for (a, b) in stage
            .extended_endpoints(id)
            .unwrap()
            .into_iter()
            .flatten()
            .zip(output)
        {
            same(a, b);
        }
    }
}

fn projection_reference(lines: &[Line], id: usize, point: Point) -> Option<Projection> {
    let actual = Stage::new(lines, &[])
        .unwrap()
        .project_floor(id, point)
        .unwrap();
    let generic = surface_projection_reference(lines, Surface::Floor, id, point);
    assert_eq!(
        actual.map(|hit| SurfaceProjection {
            line_id: hit.line_id,
            flags: hit.flags,
            normal: hit.normal,
            delta: hit.vertical_delta
        }),
        generic
    );
    actual
}

fn surface_projection_reference(
    lines: &[Line],
    surface: Surface,
    id: usize,
    point: Point,
) -> Option<SurfaceProjection> {
    let actual = Stage::new(lines, &[])
        .unwrap()
        .project(surface, id, point)
        .unwrap();
    let c_lines: Vec<_> = lines.iter().map(OracleLine::from).collect();
    let (mut delta, mut flags, mut normal) = (1234.0, 0xdeadbeef, [-5678.0; 3]);
    // SAFETY: finite nondegenerate fixtures use acyclic valid links, and all
    // repr(C) arrays and scalar outputs remain live throughout the call.
    let result = unsafe {
        oracle_stage_project(
            c_lines.as_ptr(),
            c_lines.len() as i32,
            surface.flag() as i32,
            id as i32,
            point.as_ptr(),
            &mut delta,
            &mut flags,
            normal.as_mut_ptr(),
        )
    };
    assert_eq!(actual.is_some(), result != -1);
    if let Some(projection) = actual {
        assert_eq!(projection.line_id as i32, result);
        assert_eq!(projection.flags, flags);
        same(projection.delta, delta);
        for (a, b) in projection.normal.into_iter().zip(normal) {
            same(a, b);
        }
    } else {
        assert_eq!((delta, flags, normal), (1234.0, 0xdeadbeef, [-5678.0; 3]));
    }
    actual
}

#[test]
fn floor_projection_walks_seams_and_retains_endpoint_tolerance_and_bias() {
    let lines = [
        Line {
            start: [-2.0, 0.0],
            end: [0.0, 1.0],
            flags: FLOOR | ENABLED,
            next: [Some(1), None],
            ..Line::default()
        },
        Line {
            start: [0.0, 1.0],
            end: [2.0, 2.0],
            flags: FLOOR | ENABLED,
            previous: [Some(0), None],
            material_flags: LEDGE as u16,
            ..Line::default()
        },
    ];
    for id in 0..2 {
        for x in [-2.11, -2.09, -2.0, -1.0, 0.0, 1.0, 2.0, 2.09, 2.11] {
            projection_reference(&lines, id, [x, 0.5]);
        }
    }
    let hit = projection_reference(&lines, 0, [1.0, 1.5]).unwrap();
    assert_eq!((hit.line_id, hit.flags), (1, LEDGE));
    assert_eq!(hit.vertical_delta.to_bits(), 0.0001_f32.to_bits());
}

#[test]
fn wall_and_ceiling_projection_use_final_tangent_coordinate_and_source_bias() {
    for surface in [
        Surface::Floor,
        Surface::Ceiling,
        Surface::LeftWall,
        Surface::RightWall,
    ] {
        let vertical = matches!(surface, Surface::LeftWall | Surface::RightWall);
        let increasing = matches!(surface, Surface::Floor | Surface::LeftWall);
        let mut endpoints = if vertical {
            [[0.0, -2.0], [2.0, 2.0]]
        } else {
            [[-2.0, 0.0], [2.0, 2.0]]
        };
        if !increasing {
            endpoints.swap(0, 1);
        }
        let lines = [Line {
            start: endpoints[0],
            end: endpoints[1],
            flags: surface.flag() | ENABLED,
            ..Line::default()
        }];
        let point = if vertical { [9.0, 1.0] } else { [1.0, 9.0] };
        let hit = surface_projection_reference(&lines, surface, 0, point).unwrap();
        let expected = match surface {
            Surface::Floor => -7.4999_f32,
            Surface::Ceiling => -7.5001,
            _ => -7.5,
        };
        same(hit.delta, expected);
        for along in [-2.11, -2.09, -2.0, 2.0, 2.09, 2.11] {
            let point = if vertical { [9.0, along] } else { [along, 9.0] };
            surface_projection_reference(&lines, surface, 0, point);
        }
    }
}

#[test]
fn projection_direction_reversal_preserves_each_surface_special_case() {
    for surface in [
        Surface::Floor,
        Surface::Ceiling,
        Surface::LeftWall,
        Surface::RightWall,
    ] {
        let vertical = matches!(surface, Surface::LeftWall | Surface::RightWall);
        let increasing = matches!(surface, Surface::Floor | Surface::LeftWall);
        let ranges = if increasing {
            [[0.0, 1.0], [3.0, 4.0]]
        } else {
            [[4.0, 3.0], [1.0, 0.0]]
        };
        let lines: Vec<_> = ranges
            .into_iter()
            .enumerate()
            .map(|(i, [a, b])| {
                let offset = i as f32 * 10.0;
                Line {
                    start: if vertical { [offset, a] } else { [a, offset] },
                    end: if vertical {
                        [offset + 1.0, b]
                    } else {
                        [b, offset + 1.0]
                    },
                    flags: surface.flag() | ENABLED,
                    previous: [i.checked_sub(1), None],
                    next: [(i == 0).then_some(1), None],
                    ..Line::default()
                }
            })
            .collect();
        for id in 0..2 {
            surface_projection_reference(
                &lines,
                surface,
                id,
                if vertical { [0.0, 2.0] } else { [2.0, 0.0] },
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn raw_intersections_match_c_bits(bits in any::<[u32; 8]>()) {
        intersections(bits.map(f32::from_bits));
    }

    #[test]
    fn finite_intersections_match_c(endpoints in prop::array::uniform8(-100.0_f32..100.0)) {
        intersections(endpoints);
    }

    #[test]
    fn sloped_crossings_match_c(x in -50.0_f32..50.0, y in -50.0_f32..50.0, w in 0.1_f32..30.0, h in -15.0_f32..15.0) {
        intersections([x-w,y-h,x+w,y+h,x,y+40.0,x,y-40.0]);
        intersections([x-w,y-h,x+w,y+h,x+40.0,y,x-40.0,y]);
    }

    #[test]
    fn adjacency_and_extended_coordinates_match_c(
        vertices in prop::array::uniform8(-10.0_f32..10.0),
        links in any::<[bool; 4]>(), flags in any::<u32>()
    ) {
        let lines = [
            Line { start: [vertices[0],vertices[1]], end: [vertices[2],vertices[3]],
                flags: ENABLED, previous: [links[0].then_some(1),links[1].then_some(1)],
                next: [links[2].then_some(1),links[3].then_some(1)], ..Line::default() },
            Line { start: [vertices[4],vertices[5]], end: [vertices[6],vertices[7]], flags, ..Line::default() },
        ];
        compare_endpoints(&lines);
    }

    #[test]
    fn floor_projection_matches_c(
        heights in prop::array::uniform4(-20.0_f32..20.0),
        point in prop::array::uniform2(-25.0_f32..25.0), id in 0_usize..3,
        material_flags in any::<u16>(),
    ) {
        let lines: Vec<_> = (0..3).map(|i| Line {
            start: [-15.0 + i as f32*10.0, heights[i]],
            end: [-5.0 + i as f32*10.0, heights[i+1]],
            flags: FLOOR | ENABLED, material_flags,
            previous: [i.checked_sub(1),None], next: [(i < 2).then_some(i+1),None],
        }).collect();
        projection_reference(&lines,id,point);
    }

    #[test]
    fn all_surface_projections_match_c(
        offsets in prop::array::uniform4(-20.0_f32..20.0),
        point in prop::array::uniform2(-25.0_f32..25.0), id in 0_usize..3,
        material_flags in any::<u16>(),
    ) {
        for surface in [Surface::Floor, Surface::Ceiling, Surface::LeftWall, Surface::RightWall] {
            let vertical = matches!(surface, Surface::LeftWall | Surface::RightWall);
            let sign = if matches!(surface, Surface::Floor | Surface::LeftWall) { 1.0 } else { -1.0 };
            let lines: Vec<_> = (0_usize..3).map(|i| {
                let a = sign * (-15.0 + i as f32 * 10.0);
                let b = sign * (-5.0 + i as f32 * 10.0);
                Line { start: if vertical { [offsets[i],a] } else { [a,offsets[i]] },
                    end: if vertical { [offsets[i+1],b] } else { [b,offsets[i+1]] },
                    flags: surface.flag() | ENABLED, material_flags,
                    previous: [i.checked_sub(1),None], next: [(i<2).then_some(i+1),None] }
            }).collect();
            surface_projection_reference(&lines,surface,id,point);
        }
    }

    #[test]
    fn all_surface_queries_match_c(
        raw_lines in prop::collection::vec((prop::array::uniform4(-50.0_f32..50.0), any::<u32>(), any::<u16>()), 1..9),
        path in prop::array::uniform4(-60.0_f32..60.0),
        offset in -2.0_f32..2.0,
        joint_flags in any::<[u32; 2]>(),
        mask in any::<u64>(), skips in any::<[bool; 3]>(), prechecked in any::<bool>(),
    ) {
        let lines: Vec<_> = raw_lines.into_iter().map(|(p, flags, material_flags)| Line {
            start: [p[0],p[1]], end: [p[2],p[3]], flags, material_flags, ..Line::default()
        }).collect();
        let split = lines.len()/2;
        let joints = [
            Joint { id: 5, flags: joint_flags[0], bounds_min: [-40.0;2], bounds_max: [40.0;2],
                floor: 0..split, ceiling: 0..split, left_wall: 0..split, right_wall: 0..split,
                dynamic: split..lines.len() },
            Joint { id: 2, flags: joint_flags[1], ..all_lines(lines.len()) },
        ];
        let query = Query {
            from: [path[0],path[1]], to: [path[2],path[3]], floor_y_offset: offset,
            skip_line: skips[0].then_some(0), skip_joint: skips[1].then_some(5),
            only_joint: skips[2].then_some(2),
            bounding: if prechecked { Bounding::Prechecked } else { Bounding::Compute },
        };
        for kind in [Surface::Floor, Surface::Ceiling, Surface::LeftWall, Surface::RightWall] {
            query_reference(&lines,&joints,kind,query,mask);
        }
    }
}
