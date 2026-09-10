use serde::Deserialize;
use skirmish::animation::{Channel, ChannelTarget, DataOffset, HsdDataSection};

const FIXTURE: &str = include_str!("fixtures/melee-ui/back-rotation-spline.json");
const MNMAALL_SHA256: &str = "895e1895e004f84b2cec5583402dedc25ff6173b179f294d50e76945bae32ff0";

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
