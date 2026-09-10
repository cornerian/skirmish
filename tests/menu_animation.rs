use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use skirmish::animation::{
    Channel, ChannelTarget, DataOffset, FObjTrack, HsdDataSection, Interpolation, ScalarEncoding,
};

const FIXTURE: &str = include_str!("fixtures/melee-ui/back-rotation-spline.json");
const COVERAGE_FIXTURE: &str = include_str!("fixtures/melee-ui/back-panel-animation.json");
const MNMAALL_SHA256: &str = "895e1895e004f84b2cec5583402dedc25ff6173b179f294d50e76945bae32ff0";
const ORACLE_REVISION: &str = "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9";
const FOBJ_C_SHA256: &str = "fa24a983f583ce31e0f8c809fe9084d4df7c5c4eb57be2d8a044537f790f32e9";
const SPLINE_C_SHA256: &str = "736ab29868437a263bcdbdaefdf973c75e9fa4ff3d86a3baf548f3396d415c50";

#[derive(Deserialize)]
struct Fixture {
    schema: String,
    source: String,
    source_sha256: String,
    source_revision: String,
    data_section_file_offset: u32,
    binding: Binding,
    aobj: ByteRange,
    fobj: ByteRange,
    program: ByteRange,
    expected_samples: Vec<ExpectedSample>,
    provenance: String,
}

#[derive(Deserialize)]
struct Binding {
    model: String,
    model_offset: u32,
    animation: String,
    animation_offset: u32,
    preorder_index: u32,
    anim_joint_offset: u32,
    joint_offset: u32,
    joint_name: String,
    parent_chain: Vec<u32>,
    dobj_offset: u32,
    mesh_name: String,
    mesh_vertex_count: u32,
    channel: String,
}

#[derive(Deserialize)]
struct ByteRange {
    offset: u32,
    bytes_hex: String,
}

#[derive(Deserialize)]
struct ExpectedSample {
    frame: f32,
    value_bits: String,
}

#[derive(Deserialize)]
struct CoverageFixture {
    schema: String,
    source: SourceProvenance,
    oracle: OracleProvenance,
    roots: Vec<RootCoverage>,
    coverage: Coverage,
    data_ranges: Vec<ByteRange>,
    animations: Vec<AnimationBinding>,
    track_samples: Vec<TrackSamples>,
}

#[derive(Deserialize)]
struct SourceProvenance {
    file: String,
    sha256: String,
    game: String,
    version: String,
    data_section_file_offset: u32,
    data_section_size: u32,
}

#[derive(Deserialize)]
struct OracleProvenance {
    repository: String,
    revision: String,
    sources: Vec<OracleSource>,
    compiler_flags: Vec<String>,
    method: String,
}

#[derive(Deserialize)]
struct OracleSource {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RootCoverage {
    id: String,
    animation: String,
    animation_offset: u32,
    model: String,
    model_offset: u32,
    topology_nodes: usize,
    aobj_count: usize,
    fobj_count: usize,
}

#[derive(Deserialize)]
struct Coverage {
    aobj_count: usize,
    fobj_count: usize,
    sample_count: usize,
    distinct_combination_count: usize,
    combinations: Vec<ExpectedCombination>,
}

#[derive(Deserialize)]
struct ExpectedCombination {
    target: String,
    channel: u8,
    value_encoding: u8,
    slope_encoding: u8,
    opcodes: Vec<u8>,
    occurrences: usize,
}

#[derive(Deserialize)]
struct AnimationBinding {
    root_id: String,
    target: String,
    preorder_index: usize,
    tree_node_offset: u32,
    model_node_offset: u32,
    owner_kind: String,
    owner_offset: u32,
    aobj_offset: u32,
}

#[derive(Deserialize)]
struct TrackSamples {
    fobj_offset: u32,
    samples: Vec<ExpectedBitSample>,
}

#[derive(Deserialize)]
struct ExpectedBitSample {
    frame_bits: String,
    value_bits: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CombinationKey {
    target: String,
    channel: u8,
    value_encoding: u8,
    slope_encoding: u8,
    opcodes: Vec<u8>,
}

struct BoundTrack {
    root_id: String,
    target: String,
    track: FObjTrack,
}

fn hex_bytes(value: &str) -> Vec<u8> {
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| {
            let pair = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(pair, 16).unwrap()
        })
        .collect()
}

fn insert(bytes: &mut [u8], range: &ByteRange) {
    let start = range.offset as usize;
    let source = hex_bytes(&range.bytes_hex);
    bytes[start..start + source.len()].copy_from_slice(&source);
}

fn hex_u32(value: &str) -> u32 {
    assert_eq!(value.len(), 8);
    u32::from_str_radix(value, 16).unwrap()
}

fn channel_target(value: &str) -> ChannelTarget {
    match value {
        "joint" => ChannelTarget::Joint,
        "material" => ChannelTarget::Material,
        "texture" => ChannelTarget::Texture,
        _ => panic!("unknown fixture channel target {value}"),
    }
}

fn raw_channel(value: Channel) -> u8 {
    match value {
        Channel::JointRotationX => 1,
        Channel::JointRotationY => 2,
        Channel::JointRotationZ => 3,
        Channel::JointTranslationX => 5,
        Channel::JointTranslationY => 6,
        Channel::JointTranslationZ => 7,
        Channel::JointScaleX => 8,
        Channel::JointScaleY => 9,
        Channel::JointScaleZ => 10,
        Channel::JointBranchVisibility => 12,
        Channel::MaterialDiffuseR => 4,
        Channel::MaterialDiffuseG => 5,
        Channel::MaterialDiffuseB => 6,
        Channel::MaterialAlpha => 10,
        Channel::TextureImage => 1,
        Channel::TextureTranslationU => 2,
        Channel::TextureTranslationV => 3,
        Channel::TextureBlend => 9,
        Channel::TextureKonstAlpha => 15,
        Channel::TextureTev0Alpha => 19,
    }
}

fn raw_encoding(value: ScalarEncoding) -> u8 {
    match value {
        ScalarEncoding::Float32 => 0,
        ScalarEncoding::Signed16 { fractional_bits } => 0x20 | fractional_bits,
        ScalarEncoding::Unsigned16 { fractional_bits } => 0x40 | fractional_bits,
        ScalarEncoding::Signed8 { fractional_bits } => 0x60 | fractional_bits,
        ScalarEncoding::Unsigned8 { fractional_bits } => 0x80 | fractional_bits,
    }
}

fn raw_opcode(value: Interpolation) -> u8 {
    match value {
        Interpolation::Constant => 1,
        Interpolation::Linear => 2,
        Interpolation::SplineZero => 3,
        Interpolation::Spline => 4,
    }
}

#[test]
fn pinned_back_joint_curve_decodes_and_samples_bit_exactly() {
    let fixture: Fixture = serde_json::from_str(FIXTURE).unwrap();
    assert_eq!(fixture.schema, "skirmish-hsd-animation-fixture-v1");
    assert_eq!(fixture.source, "MnMaAll.dat");
    assert_eq!(fixture.source_sha256, MNMAALL_SHA256);
    assert_eq!(fixture.source_revision, "Melee USA 1.02");
    assert_eq!(fixture.data_section_file_offset, 32);
    assert_eq!(fixture.binding.model, "MenMainBack_Top_joint");
    assert_eq!(fixture.binding.model_offset, 26_664);
    assert_eq!(fixture.binding.animation, "MenMainBack_Top_animjoint");
    assert_eq!(fixture.binding.animation_offset, 76_892);
    assert_eq!(fixture.binding.preorder_index, 93);
    assert_eq!(fixture.binding.anim_joint_offset, 78_752);
    assert_eq!(fixture.binding.joint_offset, 32_616);
    assert_eq!(fixture.binding.joint_name, "joint_7f68");
    assert_eq!(
        fixture.binding.parent_chain,
        [32_616, 32_552, 32_488, 26_728, 26_664]
    );
    assert_eq!(fixture.binding.dobj_offset, 25_560);
    assert_eq!(fixture.binding.mesh_name, "joint_7f68_p63c0");
    assert_eq!(fixture.binding.mesh_vertex_count, 3);
    assert_eq!(fixture.binding.channel, "rotation_x");
    assert!(
        fixture
            .provenance
            .contains("0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9")
    );

    let length = [&fixture.aobj, &fixture.fobj, &fixture.program]
        .into_iter()
        .map(|range| range.offset as usize + range.bytes_hex.len() / 2)
        .max()
        .unwrap();
    let mut bytes = vec![0; length];
    insert(&mut bytes, &fixture.aobj);
    insert(&mut bytes, &fixture.fobj);
    insert(&mut bytes, &fixture.program);

    let animation = HsdDataSection::new(&bytes)
        .unwrap()
        .decode_aobj(DataOffset::new(fixture.aobj.offset), ChannelTarget::Joint)
        .unwrap();
    assert_eq!(animation.tracks.len(), 1);
    assert_eq!(animation.tracks[0].channel, Channel::JointRotationX);
    for expected in fixture.expected_samples {
        let values = animation.sample_requested_frame(expected.frame).unwrap();
        assert_eq!(values.len(), 1);
        let expected_bits = u32::from_str_radix(&expected.value_bits, 16).unwrap();
        assert_eq!(
            values[0].value.to_bits(),
            expected_bits,
            "frame {}",
            expected.frame
        );
    }
}

#[test]
fn every_back_and_panel_track_matches_the_pinned_c_oracle() {
    let fixture: CoverageFixture = serde_json::from_str(COVERAGE_FIXTURE).unwrap();
    assert_eq!(fixture.schema, "skirmish-hsd-animation-coverage-v2");
    assert_eq!(fixture.source.file, "MnMaAll.dat");
    assert_eq!(fixture.source.sha256, MNMAALL_SHA256);
    assert_eq!(fixture.source.game, "GALE01");
    assert_eq!(fixture.source.version, "1.02");
    assert_eq!(fixture.source.data_section_file_offset, 32);
    assert_eq!(fixture.source.data_section_size, 2_095_960);

    assert_eq!(
        fixture.oracle.repository,
        "https://github.com/doldecomp/melee.git"
    );
    assert_eq!(fixture.oracle.revision, ORACLE_REVISION);
    let oracle_sources = fixture
        .oracle
        .sources
        .iter()
        .map(|source| (source.path.as_str(), source.sha256.as_str()))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        oracle_sources.get("src/sysdolphin/baselib/fobj.c"),
        Some(&FOBJ_C_SHA256)
    );
    assert_eq!(
        oracle_sources.get("src/sysdolphin/baselib/spline.c"),
        Some(&SPLINE_C_SHA256)
    );
    assert_eq!(
        fixture.oracle.compiler_flags,
        [
            "-std=gnu11",
            "-O0",
            "-fwrapv",
            "-ffp-contract=off",
            "-fno-fast-math"
        ]
    );
    assert!(fixture.oracle.method.contains("HSD_FObjReqAnimAll"));
    assert!(fixture.oracle.method.contains("HSD_FObjInterpretAnim"));
    assert!(
        fixture
            .oracle
            .method
            .contains("exactly one update callback")
    );
    assert!(fixture.oracle.method.contains("does not establish PowerPC"));

    assert_eq!(fixture.roots.len(), 4);
    let roots = fixture
        .roots
        .iter()
        .map(|root| (root.id.as_str(), root))
        .collect::<BTreeMap<_, _>>();
    for root in &fixture.roots {
        let expected = match root.id.as_str() {
            "back_joint" => (
                "MenMainBack_Top_animjoint",
                76_892,
                "MenMainBack_Top_joint",
                26_664,
                102,
                40,
                87,
            ),
            "back_material" => (
                "MenMainBack_Top_matanim_joint",
                83_380,
                "MenMainBack_Top_joint",
                26_664,
                102,
                15,
                15,
            ),
            "panel_joint" => (
                "MenMainPanel_Top_animjoint",
                350_292,
                "MenMainPanel_Top_joint",
                140_584,
                106,
                74,
                170,
            ),
            "panel_material" => (
                "MenMainPanel_Top_matanim_joint",
                354_560,
                "MenMainPanel_Top_joint",
                140_584,
                106,
                4,
                4,
            ),
            id => panic!("unexpected fixture root {id}"),
        };
        assert_eq!(
            (
                root.animation.as_str(),
                root.animation_offset,
                root.model.as_str(),
                root.model_offset,
                root.topology_nodes,
                root.aobj_count,
                root.fobj_count,
            ),
            expected
        );
    }

    let expected_ranges = [
        (72_000_u32, 76_892_u32),
        (78_932, 81_140),
        (333_472, 350_292),
        (352_412, 352_748),
    ];
    assert_eq!(fixture.data_ranges.len(), expected_ranges.len());
    let mut data = vec![0_u8; expected_ranges.last().unwrap().1 as usize];
    let mut fixture_bytes = 0_usize;
    for (range, &(expected_start, expected_end)) in
        fixture.data_ranges.iter().zip(expected_ranges.iter())
    {
        let source = hex_bytes(&range.bytes_hex);
        assert_eq!(range.offset, expected_start);
        assert_eq!(range.offset as usize + source.len(), expected_end as usize);
        fixture_bytes += source.len();
        insert(&mut data, range);
    }
    assert_eq!(fixture_bytes, 24_256);

    assert_eq!(fixture.animations.len(), fixture.coverage.aobj_count);
    let section = HsdDataSection::new(&data).unwrap();
    let mut tracks = BTreeMap::<u32, BoundTrack>::new();
    let mut aobj_offsets = BTreeSet::new();
    let mut aobjs_by_root = BTreeMap::<String, usize>::new();
    let mut fobjs_by_root = BTreeMap::<String, usize>::new();
    for binding in &fixture.animations {
        let root = roots
            .get(binding.root_id.as_str())
            .unwrap_or_else(|| panic!("unknown binding root {}", binding.root_id));
        assert!(binding.preorder_index < root.topology_nodes);
        for offset in [
            binding.tree_node_offset,
            binding.model_node_offset,
            binding.owner_offset,
            binding.aobj_offset,
        ] {
            assert_ne!(offset, 0);
            assert_eq!(offset % 4, 0);
            assert!(offset < fixture.source.data_section_size);
        }
        match (binding.target.as_str(), binding.owner_kind.as_str()) {
            ("joint", "animjoint") | ("material", "matanim") | ("texture", "texanim") => {}
            pair => panic!("invalid target/owner pairing {pair:?}"),
        }
        if binding.target == "joint" {
            assert!(binding.root_id.ends_with("_joint"));
            assert_eq!(binding.owner_offset, binding.tree_node_offset);
        } else {
            assert!(binding.root_id.ends_with("_material"));
        }
        assert!(aobj_offsets.insert(binding.aobj_offset));

        let animation = section
            .decode_aobj(
                DataOffset::new(binding.aobj_offset),
                channel_target(&binding.target),
            )
            .unwrap_or_else(|error| {
                panic!(
                    "failed to decode {} AObj at {:#x}: {error}",
                    binding.root_id, binding.aobj_offset
                )
            });
        *aobjs_by_root.entry(binding.root_id.clone()).or_default() += 1;
        *fobjs_by_root.entry(binding.root_id.clone()).or_default() += animation.tracks.len();
        for track in animation.tracks {
            let offset = track.descriptor_offset.get();
            let previous = tracks.insert(
                offset,
                BoundTrack {
                    root_id: binding.root_id.clone(),
                    target: binding.target.clone(),
                    track,
                },
            );
            assert!(
                previous.is_none(),
                "duplicate FObj descriptor at {offset:#x}"
            );
        }
    }
    assert_eq!(aobj_offsets.len(), fixture.coverage.aobj_count);
    assert_eq!(tracks.len(), fixture.coverage.fobj_count);
    assert_eq!(fixture.coverage.aobj_count, 133);
    assert_eq!(fixture.coverage.fobj_count, 276);
    for root in &fixture.roots {
        assert_eq!(aobjs_by_root.get(&root.id), Some(&root.aobj_count));
        assert_eq!(fobjs_by_root.get(&root.id), Some(&root.fobj_count));
    }

    let mut actual_combinations = BTreeMap::<CombinationKey, usize>::new();
    for bound in tracks.values() {
        let opcodes = bound
            .track
            .keys
            .iter()
            .map(|key| raw_opcode(key.interpolation))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let key = CombinationKey {
            target: bound.target.clone(),
            channel: raw_channel(bound.track.channel),
            value_encoding: raw_encoding(bound.track.value_encoding),
            slope_encoding: raw_encoding(bound.track.slope_encoding),
            opcodes,
        };
        *actual_combinations.entry(key).or_default() += 1;
    }
    let expected_combinations = fixture
        .coverage
        .combinations
        .iter()
        .map(|combination| {
            (
                CombinationKey {
                    target: combination.target.clone(),
                    channel: combination.channel,
                    value_encoding: combination.value_encoding,
                    slope_encoding: combination.slope_encoding,
                    opcodes: combination.opcodes.clone(),
                },
                combination.occurrences,
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        expected_combinations.len(),
        fixture.coverage.combinations.len()
    );
    assert_eq!(
        expected_combinations.len(),
        fixture.coverage.distinct_combination_count
    );
    assert_eq!(fixture.coverage.distinct_combination_count, 119);
    assert_eq!(actual_combinations, expected_combinations);

    let target_channels = actual_combinations
        .keys()
        .map(|key| (key.target.as_str(), key.channel))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        target_channels,
        [
            ("joint", 1),
            ("joint", 2),
            ("joint", 3),
            ("joint", 5),
            ("joint", 6),
            ("joint", 7),
            ("joint", 8),
            ("joint", 9),
            ("joint", 10),
            ("joint", 12),
            ("material", 10),
            ("texture", 1),
            ("texture", 2),
            ("texture", 3),
            ("texture", 9),
        ]
        .into_iter()
        .collect()
    );
    let value_encodings = actual_combinations
        .keys()
        .map(|key| key.value_encoding)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        value_encodings,
        [
            0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
            0x61, 0x62, 0x63, 0x65, 0x66, 0x83, 0x84, 0x85, 0x87, 0x88,
        ]
        .into_iter()
        .collect()
    );
    let slope_encodings = actual_combinations
        .keys()
        .map(|key| key.slope_encoding)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        slope_encodings,
        [0x00, 0x2e, 0x50, 0x87, 0x88].into_iter().collect()
    );
    let opcodes = actual_combinations
        .keys()
        .flat_map(|key| key.opcodes.iter().copied())
        .collect::<BTreeSet<_>>();
    assert_eq!(opcodes, [1, 2, 3, 4].into_iter().collect());

    assert_eq!(fixture.track_samples.len(), tracks.len());
    let mut sampled_tracks = BTreeSet::new();
    let mut sample_count = 0_usize;
    for expected_track in &fixture.track_samples {
        assert!(sampled_tracks.insert(expected_track.fobj_offset));
        let bound = tracks
            .get(&expected_track.fobj_offset)
            .unwrap_or_else(|| panic!("sampled unknown FObj at {:#x}", expected_track.fobj_offset));
        let mut boundary_count = 0_usize;
        let mut interior_opcodes = BTreeSet::new();
        for expected in &expected_track.samples {
            sample_count += 1;
            let frame = f32::from_bits(hex_u32(&expected.frame_bits));
            let time = frame + f32::from(bound.track.start_frame as i16);
            let mut segment_start = 0.0_f32;
            let mut is_boundary = time == 0.0;
            for key in &bound.track.keys[..bound.track.keys.len() - 1] {
                let segment_end = segment_start + f32::from(key.wait);
                if time == segment_end {
                    is_boundary = true;
                } else if segment_start < time && time < segment_end {
                    interior_opcodes.insert(raw_opcode(key.interpolation));
                }
                segment_start = segment_end;
            }
            boundary_count += usize::from(is_boundary);

            let actual = bound.track.sample(frame).unwrap().unwrap_or_else(|| {
                panic!(
                    "{} {} FObj {:#x} emitted no value at frame {frame}",
                    bound.root_id, bound.target, expected_track.fobj_offset
                )
            });
            assert_eq!(
                actual.to_bits(),
                hex_u32(&expected.value_bits),
                "{} {} FObj {:#x} at frame {frame}",
                bound.root_id,
                bound.target,
                expected_track.fobj_offset
            );
        }
        assert!(boundary_count >= 2, "missing boundary samples");
        let outgoing_opcodes = bound.track.keys[..bound.track.keys.len() - 1]
            .iter()
            .filter(|key| key.wait != 0)
            .map(|key| raw_opcode(key.interpolation))
            .collect::<BTreeSet<_>>();
        assert_eq!(interior_opcodes, outgoing_opcodes);
    }
    assert_eq!(sampled_tracks, tracks.keys().copied().collect());
    assert_eq!(sample_count, fixture.coverage.sample_count);
    assert_eq!(sample_count, 1_349);
}
