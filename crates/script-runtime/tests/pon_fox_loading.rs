//! Native Pon loading conformance for the complete class based Fox resource.
//!
//! This intentionally loads the checked in authoring package through the
//! SourceBundle importer.  The test only observes
//! the plain exported definition and callback names returned by Pon.

use std::collections::{BTreeMap, BTreeSet};

use skirmish_script_runtime::{CompiledProgram, NativeValue, SourceBundle};

fn fox_bundle() -> SourceBundle {
    SourceBundle::new("fighter-api-fox-conformance-v1")
        .with_file("fox.py", include_str!("../../../scripts/fighters/fox.py"))
        .expect("Fox module path")
}

fn dict<'a>(value: &'a NativeValue, label: &str) -> &'a BTreeMap<String, NativeValue> {
    match value {
        NativeValue::Dict(value) => value,
        other => panic!("{label} must be a dictionary, got {other:?}"),
    }
}

fn string<'a>(value: &'a NativeValue, label: &str) -> &'a str {
    match value {
        NativeValue::String(value) => value,
        other => panic!("{label} must be a string, got {other:?}"),
    }
}

fn ints(value: &NativeValue, label: &str) -> Vec<i64> {
    match value {
        NativeValue::List(values) => values
            .iter()
            .map(|value| match value {
                NativeValue::Int(value) => *value,
                other => panic!("{label} must contain integers, got {other:?}"),
            })
            .collect(),
        other => panic!("{label} must be a list, got {other:?}"),
    }
}

#[test]
fn dispatch_on_unprepared_thread_fails_without_compiling() {
    let source = include_str!("../../../scripts/api/tests/fixtures/native_fighter.py");
    let program =
        CompiledProgram::new(source, "probe.py", []).expect("construct valid Pon program");
    let callback = program.bind_callback("probe");
    let error = std::thread::spawn(move || program.invoke_values(&callback, &[]).unwrap_err())
        .join()
        .expect("unprepared worker thread should return an error");
    let message = error.to_string();
    assert!(
        message.contains("not prepared for current thread"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("prepare_for_current_thread"),
        "error must explain the preparation boundary: {message}"
    );
}

#[test]
fn fox_definition_loads_with_all_groups_typed_state_and_special_callbacks() {
    // Execute the real authoring module as the Pon program so its class keeps
    // the program module identity expected by the registered loader.
    let source = include_str!("../../../scripts/fighters/fox.py");
    let program = CompiledProgram::new_with_bundle(source, "fox.py", [], Some(fox_bundle()))
        .expect("construct Fox Pon program");
    program
        .prepare_for_current_thread()
        .expect("prepare Fox Pon program");

    let metadata = program.export_metadata().expect("export Fox definition");
    let root = dict(&metadata, "Fox definition");
    assert_eq!(string(root.get("name").expect("name"), "name"), "fox");
    assert_eq!(
        root.get("external_ids")
            .map(|value| ints(value, "external_ids")),
        Some(vec![2])
    );

    let parameters = dict(root.get("parameters").expect("parameters"), "parameters");
    assert_eq!(
        string(
            parameters.get("projectile_kind").expect("projectile_kind"),
            "projectile_kind"
        ),
        "fox_laser"
    );

    let action_state = dict(
        root.get("action_state").expect("action_state"),
        "action_state",
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
            action_state.contains_key(field),
            "Fox action state lacks {field}"
        );
    }
    // Metadata uses the stable JSON-compatible sequence wire form. The
    // lifecycle host reifies this declared fixed tuple as immutable state.
    assert!(
        matches!(action_state["command"], NativeValue::List(ref values) if *values == vec![NativeValue::Int(0); 4])
    );
    assert!(matches!(action_state["repeat_armed"], NativeValue::Bool(_)));
    assert!(matches!(action_state["gravity_delay"], NativeValue::F32(_)));

    let movesets = dict(root.get("movesets").expect("movesets"), "movesets");
    let expected_groups = [
        ("specials", ["neutral", "side", "up", "down"].as_slice()),
        (
            "aerials",
            ["neutral", "forward", "back", "up", "down"].as_slice(),
        ),
        ("grounded", ["jab", "rapid_jab", "dash"].as_slice()),
        ("tilts", ["forward", "up", "down"].as_slice()),
        ("smashes", ["forward", "up", "down"].as_slice()),
        ("grabs", ["standing", "dash", "pummel"].as_slice()),
        ("throws", ["forward", "back", "up", "down"].as_slice()),
        (
            "defense",
            [
                "shield",
                "spot_dodge",
                "roll_forward",
                "roll_back",
                "air_dodge",
            ]
            .as_slice(),
        ),
        (
            "ledge",
            ["wait", "getup", "roll", "attack", "jump"].as_slice(),
        ),
        (
            "getup",
            ["neutral", "roll_forward", "roll_back", "attack"].as_slice(),
        ),
        ("taunt", ["taunt"].as_slice()),
    ];
    let mut move_ids = BTreeSet::new();
    for (group_name, slots) in expected_groups {
        let group = dict(movesets.get(group_name).expect(group_name), group_name);
        assert_eq!(group.len(), slots.len(), "unexpected {group_name} slots");
        for slot in slots {
            let id = string(group.get(*slot).expect(slot), slot);
            assert!(
                id.starts_with("move_"),
                "{group_name}.{slot} has invalid move id {id}"
            );
            move_ids.insert(id.to_owned());
        }
    }

    let behaviors = match root.get("behaviors").expect("behaviors") {
        NativeValue::List(values) => values,
        other => panic!("behaviors must be a list, got {other:?}"),
    };
    let behavior_ids = behaviors
        .iter()
        .map(|behavior| {
            string(
                dict(behavior, "behavior").get("id").expect("behavior id"),
                "behavior id",
            )
            .to_owned()
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        behavior_ids, move_ids,
        "every declared move has one exported identity"
    );

    let callback_keys = program.callback_keys().expect("Fox callbacks export");
    let callback_set = callback_keys.iter().collect::<BTreeSet<_>>();
    let specials = dict(movesets.get("specials").expect("specials"), "specials");
    let special_ids = ["neutral", "side", "up", "down"]
        .iter()
        .map(|slot| string(specials.get(*slot).expect(slot), slot).to_owned())
        .collect::<Vec<_>>();
    for id in special_ids {
        assert!(
            callback_set
                .iter()
                .any(|key| key.starts_with(&format!("{id}."))),
            "special move {id} exported no callbacks"
        );
        let behavior = behaviors
            .iter()
            .find(|behavior| {
                string(
                    dict(behavior, "behavior").get("id").expect("behavior id"),
                    "behavior id",
                ) == id
            })
            .expect("special behavior");
        let validate = dict(behavior, "behavior")
            .get("validate")
            .expect("validate key");
        assert!(
            matches!(validate, NativeValue::String(_)),
            "special {id} lacks validator"
        );
    }
}
