//! Integrity and cross-source checks for incomplete native resource evidence.
//! These checks do not certify a fighter, a stage, or whole-game behavior.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};

fn directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/native-data")
}

fn json(file: &str) -> Value {
    serde_json::from_slice(&fs::read(directory().join(file)).unwrap()).unwrap()
}

fn hex_bytes(value: &Value) -> Vec<u8> {
    value
        .as_str()
        .unwrap()
        .split_ascii_whitespace()
        .map(|byte| {
            assert_eq!(byte.len(), 2);
            u8::from_str_radix(byte, 16).unwrap()
        })
        .collect()
}

fn sha256_text(value: &Value) -> bool {
    value.as_str().is_some_and(|text| {
        text.len() == 64
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

#[test]
fn manifest_covers_every_fixture_once_and_pins_its_content() {
    let manifest = json("sources.json");
    let mut source_ids = BTreeSet::new();
    for source in manifest["sources"].as_array().unwrap() {
        assert!(source_ids.insert(source["id"].as_str().unwrap()));
        assert!(source["url"].as_str().unwrap().starts_with("https://"));
        assert!(sha256_text(&source["sha256"]));
        assert!(source["bytes"].as_u64().unwrap() > 0);
        assert!(!source["license"].as_str().unwrap().is_empty());
    }
    let mut listed = BTreeSet::new();
    for fixture in manifest["fixtures"].as_array().unwrap() {
        let file = fixture["file"].as_str().unwrap();
        assert_eq!(PathBuf::from(file).components().count(), 1);
        assert!(!file.starts_with('.'));
        assert!(listed.insert(file.to_owned()), "duplicate fixture {file}");
        assert!(source_ids.contains(fixture["source_id"].as_str().unwrap()));
        assert!(!fixture["selection"].as_str().unwrap().is_empty());
        let contents = fs::read(directory().join(file)).unwrap();
        assert_eq!(contents.len() as u64, fixture["bytes"].as_u64().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(contents)),
            fixture["sha256"].as_str().unwrap(),
            "fixture changed without updating provenance: {file}"
        );
    }
    let actual: BTreeSet<_> = fs::read_dir(directory())
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            entry.file_name().into_string().unwrap()
        })
        .filter(|name| name != "sources.json")
        .collect();
    assert!(!listed.is_empty());
    assert_eq!(listed, actual);
}

#[test]
fn attributes_preserve_binary32_values_and_the_integer_jump_count() {
    let fox = json("fox-native-subset.json");
    let attributes = fox["attributes"].as_array().unwrap();
    let expected_indices: BTreeSet<_> = (0..38).chain(57..63).collect();
    let mut indices = BTreeSet::new();
    let mut fields = BTreeSet::new();
    for attribute in attributes {
        let index = attribute["index"].as_u64().unwrap();
        assert!(indices.insert(index));
        assert!(fields.insert(attribute["upstream_field"].as_str().unwrap()));
        assert_eq!(
            attribute["offset"].as_str().unwrap(),
            format!("0x{:03x}", index * 4)
        );
        if attribute["type"] == "float" {
            let value = attribute["value"].as_f64().unwrap() as f32;
            assert!(value.is_finite());
            let bits = attribute["bits"]
                .as_str()
                .unwrap()
                .strip_prefix("f32:")
                .unwrap();
            assert_eq!(bits.len(), 8);
            assert_eq!(value.to_bits(), u32::from_str_radix(bits, 16).unwrap());
        } else {
            assert_eq!(attribute["type"], "int");
            assert_eq!(attribute["upstream_field"], "max_jumps");
            assert_eq!(attribute["value"].as_i64(), Some(2));
            assert!(attribute.get("bits").is_none());
        }
    }
    assert_eq!(indices, expected_indices);
}

#[test]
fn raw_hitbox_radius_matches_captured_frames_using_upstream_scaling() {
    let fox = json("fox-native-subset.json");
    let events = fox["jab1"]["publisher_subaction"]["events"]
        .as_array()
        .unwrap();
    let mut joined = Vec::new();
    let mut radii = Vec::new();
    for event in events {
        let bytes = hex_bytes(&event["bytes"]);
        assert_eq!(bytes.len() as u64, event["length"].as_u64().unwrap());
        joined.extend_from_slice(&bytes);
        if bytes[0] & 0xfc == 0x2c {
            assert_eq!(bytes.len(), 20);
            // Pinned lb/types.h: spawn_hitbox_0/1; ftAction_8007121C.
            let word = u32::from_be_bytes(bytes[..4].try_into().unwrap());
            assert_eq!((word >> 23) & 7, radii.len() as u32); // hitbox ID
            assert_eq!((word >> 11) & 0xff, 25); // bone index
            assert_eq!(word & 0x3ff, 4); // damage
            let size = u16::from_be_bytes(bytes[4..6].try_into().unwrap());
            radii.push(f32::from(size) * 0.003906_f32);
        }
    }
    assert_eq!(radii.len(), 2);
    assert_eq!(joined, hex_bytes(&fox["jab1"]["runtime_event_bytes_hex"]));
    assert_eq!(
        events.len() as u64,
        fox["jab1"]["event_count"].as_u64().unwrap()
    );

    // This fixed source excerpt has no embedded commas or escaped quotes.
    let csv = fs::read_to_string(directory().join("fox-jab1-captured.csv")).unwrap();
    let mut lines = csv.lines();
    let headers: Vec<_> = lines.next().unwrap().split(',').collect();
    let mut active_frames = Vec::new();
    let mut first_iasa = None;
    let mut count = 0;
    for (index, line) in lines.enumerate() {
        let cells: Vec<_> = line.split(',').map(|cell| cell.trim_matches('"')).collect();
        assert_eq!(cells.len(), headers.len());
        let value = |column: &str| {
            cells[headers
                .iter()
                .position(|header| header.trim_matches('"') == column)
                .unwrap()]
        };
        assert_eq!(value("character"), "1");
        assert_eq!(value("action"), "44");
        let frame = value("frame").parse::<usize>().unwrap();
        assert_eq!(frame, index + 1);
        if value("hitbox_1_status") == "True" {
            active_frames.push(frame);
            assert_eq!(value("hitbox_2_status"), "True");
            for (box_index, radius) in radii.iter().enumerate() {
                let captured = value(&format!("hitbox_{}_size", box_index + 1))
                    .parse::<f32>()
                    .unwrap();
                assert_eq!(radius.to_bits(), captured.to_bits());
            }
        }
        if value("iasa") == "True" {
            first_iasa.get_or_insert(frame);
        }
        count += 1;
    }
    assert_eq!(count, 17);
    assert_eq!(active_frames, [2, 3]);
    assert_eq!(first_iasa, Some(8));

    let processed = json("fox-jab1-processed.json");
    let jab = &processed["publisher_jab1"];
    assert_eq!(jab["iasa"], 16);
    let processed_size = jab["hitFrames"][0]["hitboxes"][0]["size"].as_f64().unwrap() as f32;
    assert_ne!(processed_size.to_bits(), radii[0].to_bits());
}

#[test]
fn incomplete_evidence_cannot_silently_become_a_complete_match_resource() {
    let fox = json("fox-native-subset.json");
    let stage = json("final-destination-bounds.json");
    for resource in [&fox, &stage] {
        assert_eq!(resource["complete_simulation_resource"], false);
        assert_eq!(resource["game_revision_verified"], false);
    }
    assert_eq!(fox["jab1"]["parsed_fields_are_authoritative"], false);
    assert!(fox["jab1"]["missing"].as_array().unwrap().len() >= 5);
    for field in ["floor_y", "collision_vertices", "collision_segments"] {
        assert!(stage.get(field).unwrap().is_null(), "unsupported {field}");
    }
    assert_eq!(
        json("fox-jab1-processed.json")["authoritative_runtime_values"],
        false
    );
    let manifest = json("sources.json");
    assert!(manifest["conflicts"].as_array().unwrap().len() >= 3);
    let gaps: BTreeSet<_> = manifest["coverage_gaps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|gap| gap.as_str().unwrap())
        .collect();
    for required in [
        "complete_bone_hierarchy_and_bind_pose",
        "bone_animation_tracks",
        "hurtbox_capsules_and_bone_attachments",
        "full_stage_collision_mesh",
        "verified_same_game_revision_across_sources",
        "complete_native_simulator_checkpoint",
    ] {
        assert!(gaps.contains(required), "missing coverage gap: {required}");
    }
}
