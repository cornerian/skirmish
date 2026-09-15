use std::collections::BTreeMap;

use skirmish::game::script::resources::{Resources, Specials};

fn fixture() -> Specials {
    Specials {
        character: "Fox".into(),
        resources: Resources::new(BTreeMap::from([
            ("neutral".into(), serde_json::json!({"frames": []})),
            (
                "down".into(),
                serde_json::json!({"nested": {"values": [1, 2, 3]}}),
            ),
            ("metadata".into(), serde_json::json!({"phase": "loop"})),
        ]))
        .unwrap(),
    }
}

#[test]
fn specials_json_round_trip_preserves_flat_resource_keys_and_indexing() {
    let original = fixture();
    let encoded = serde_json::to_value(&original).unwrap();
    assert_eq!(encoded["character"], "Fox");
    assert!(encoded.get("neutral").is_some());
    assert!(encoded.get("down").is_some());

    let decoded: Specials = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, original);
    assert!(decoded.lookup("down.nested.values").is_some());
    assert!(decoded.attack("neutral").is_some());
}

#[test]
fn specials_cbor_round_trip_preserves_flat_resource_keys_and_indexing() {
    let original = fixture();
    let mut encoded = Vec::new();
    ciborium::into_writer(&original, &mut encoded).unwrap();
    let decoded: Specials = ciborium::from_reader(encoded.as_slice()).unwrap();
    assert_eq!(decoded, original);
    assert!(decoded.lookup("down.nested.values").is_some());
    assert!(decoded.attack("neutral").is_some());
}
