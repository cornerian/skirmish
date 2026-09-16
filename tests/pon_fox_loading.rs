//! Game registration boundary for the complete native Fox definition.

use skirmish::game::script::definition::{AssetStore, Definition};

#[test]
fn registered_fox_load_decodes_schema_and_registers_special_callbacks() {
    let source = include_str!("../scripts/fighters/fox.py");
    let assets = AssetStore::builtins();
    assert!(assets.shared("common.py").is_some());
    assert!(
        assets
            .dependencies(source)
            .unwrap()
            .contains_key("shared/common.py")
    );
    let definition = Definition::load_registered(source, &assets).expect("load Fox definition");

    assert_eq!(definition.manifest.name, "fox");
    assert_eq!(definition.manifest.external_ids, vec![2]);
    assert_eq!(
        definition
            .manifest
            .parameters
            .get("projectile_kind")
            .and_then(serde_json::Value::as_str),
        Some("fox_laser")
    );
    for field in [
        "command",
        "repeat_armed",
        "fire_pending",
        "gravity_delay",
        "release_lag",
        "is_release",
        "turn_frames",
        "turned",
        "looping",
        "travel_remaining",
        "ground_travel_frames",
        "travel_angle",
    ] {
        assert!(
            definition.manifest.action_state.fields.contains_key(field),
            "Fox action state lacks {field}"
        );
    }

    let special_ids = ["neutral", "side", "up", "down"]
        .iter()
        .map(|slot| {
            definition
                .manifest
                .movesets
                .get("specials")
                .and_then(serde_json::Value::as_object)
                .and_then(|group| group.get(*slot))
                .and_then(serde_json::Value::as_str)
                .expect("special move identity")
        })
        .collect::<Vec<_>>();
    for id in special_ids {
        let behavior = definition
            .manifest
            .behaviors
            .iter()
            .find(|behavior| behavior.id.as_deref() == Some(id))
            .expect("special behavior");
        assert!(
            behavior.validate.is_some(),
            "special {:?} lacks validator",
            behavior.id
        );
        assert!(
            !behavior.callbacks.is_empty(),
            "special {:?} lacks callbacks",
            behavior.id
        );
    }
    assert!(
        definition
            .program
            .has_hook(skirmish::game::script::Hook::InputPressed)
            .unwrap()
    );
}
