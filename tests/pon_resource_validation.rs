//! Registration-time execution of the native Fox resource validators.
//!
//! These tests intentionally construct a real `Match`: checking exported
//! metadata alone would not prove that the validator callback was dispatched
//! against the immutable native resource cache.

#[path = "support/conformance.rs"]
mod conformance;
#[path = "support/fox_down_special.rs"]
mod down_special_resources;
#[path = "support/fox_neutral_special.rs"]
mod neutral_special_resources;
#[path = "support/fox_side_special.rs"]
mod side_special_resources;
#[path = "support/fox_up_special.rs"]
mod up_special_resources;

use skirmish::game::{
    Match,
    data::MotionStateProfile,
    script::resources::{Resources, Specials},
};

fn valid_data() -> skirmish::game::data::MatchData {
    let base = conformance::data();
    let profiles = [
        neutral_special_resources::profile(base.clone()),
        side_special_resources::profile(base.clone()),
        up_special_resources::profile(base.clone()),
        down_special_resources::profile(base),
    ];
    let special_rules = profiles[2].rules.specials;
    let mut values = std::collections::BTreeMap::new();
    for profile in profiles {
        let specials = profile.fighters[0]
            .specials
            .clone()
            .expect("special fixture");
        values.extend(specials.resources.values);
    }
    let specials = Specials {
        character: "Fox".into(),
        special_attributes: None,
        animations: None,
        articles: None,
        resources: Resources::new(values).expect("combined specials resources"),
    };
    let mut data = conformance::data();
    data.rules.specials = special_rules;
    for fighter in &mut data.fighters {
        fighter.specials = Some(specials.clone());
    }
    data
}

fn neutral_only_data() -> skirmish::game::data::MatchData {
    let base = conformance::data();
    let neutral = neutral_special_resources::profile(base.clone());
    let special_rules = up_special_resources::profile(base).rules.specials;
    let specials = neutral.fighters[0]
        .specials
        .clone()
        .expect("neutral fixture");
    let mut data = conformance::data();
    data.rules.specials = special_rules;
    for fighter in &mut data.fighters {
        fighter.specials = Some(specials.clone());
    }
    data
}

#[test]
fn invalid_complete_animation_attack_is_rejected_before_match_creation() {
    let mut data = conformance::data();
    let mut attack = serde_json::to_value(&data.fighters[0].jab).unwrap();
    attack["frames"][0]["hitboxes"] = serde_json::json!([{
        "group": 0,
        "bone": 0,
        "center": [0.0, 0.0, 0.0],
        "radius": -1.0,
        "damage": 1,
        "angle_degrees": 45.0,
        "growth": 1,
        "fixed": 1,
        "base": 1
    }]);
    let specials: Specials = serde_json::from_value(serde_json::json!({
        "character": "donkey-kong",
        "animations": {
            "331": {
                "animation_id": 331,
                "state_ids": [381],
                "status": "complete",
                "resource": attack
            }
        }
    }))
    .unwrap();
    for fighter in &mut data.fighters {
        fighter.motion_states = Some(vec![MotionStateProfile {
            state_id: 381,
            animation_id: 331,
            move_id: 20,
            flags: 0,
        }]);
        fighter.specials = Some(specials.clone());
    }

    let error = Match::new(data, 0).expect_err("invalid animation attack must fail registration");
    let message = error.to_string();
    assert!(
        message.contains("invalid animation 331"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("invalid or unsupported hitbox"),
        "unexpected error: {message}"
    );
}

#[test]
fn valid_neutral_resource_runs_its_validator_during_match_registration() {
    Match::new(valid_data(), 0).expect("valid neutral resource must pass native validator");
}

#[test]
fn neutral_only_resources_skip_disabled_optional_behaviors() {
    Match::new(neutral_only_data(), 0)
        .expect("missing optional side/up/down roots must not link their actions");
}

#[test]
fn missing_active_neutral_resource_is_rejected() {
    let mut data = neutral_only_data();
    for fighter in &mut data.fighters {
        fighter
            .specials
            .as_mut()
            .expect("neutral specials")
            .resources
            .values
            .get_mut("neutral")
            .expect("neutral root")
            .as_object_mut()
            .expect("neutral object")
            .remove("neutral_thresholds");
    }
    let error = Match::new(data, 0).expect_err("active neutral resource must be validated");
    assert!(error.to_string().contains("resource validator"));
}

#[test]
fn invalid_neutral_resource_is_rejected_by_its_validator_before_match_creation() {
    let mut data = valid_data();
    for fighter in &mut data.fighters {
        let thresholds = fighter
            .specials
            .as_mut()
            .expect("neutral fixture has specials")
            .resources
            .values
            .get_mut("neutral")
            .expect("neutral resource")
            .get_mut("neutral_thresholds")
            .expect("neutral thresholds")
            .as_array_mut()
            .expect("threshold array");
        thresholds[0] = serde_json::json!(0.0);
    }

    let error = Match::new(data, 0).expect_err("invalid resource must fail registration");
    let message = error.to_string();
    assert!(
        message.contains("resource validator"),
        "unexpected error: {message}"
    );
    assert!(
        message.contains("returned false"),
        "unexpected error: {message}"
    );
}
