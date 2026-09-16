//! Native Pon loading and callback conformance for Captain Falcon.
//!
//! The host below is deliberately a small resource-shaped fixture rather than
//! a gameplay pack.  It supplies only the native object paths exercised by the
//! translated Falcon Punch, Dive, and aerial Kick callbacks.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use skirmish_script_runtime::host_object;
use skirmish_script_runtime::{
    CompiledProgram, Error, HostRef, NativeHost, NativeKind, NativeObject, NativeValue,
    SourceBundle, shared_host,
};

fn captain_bundle() -> SourceBundle {
    SourceBundle::new("fighter-api-captain-conformance-v1")
        .with_file(
            "captain.py",
            include_str!("../../../scripts/fighters/captain.py"),
        )
        .and_then(|bundle| {
            bundle.with_file(
                "shared/common.py",
                include_str!("../../../scripts/fighters/common.py"),
            )
        })
        .expect("Captain Falcon module path")
}

fn captain_program() -> CompiledProgram {
    CompiledProgram::new_with_bundle(
        include_str!("../../../scripts/fighters/captain.py"),
        "captain.py",
        [],
        Some(captain_bundle()),
    )
    .expect("construct Captain Falcon Pon program")
}

fn callback(
    program: &CompiledProgram,
    move_id: &str,
    event: &str,
) -> skirmish_script_runtime::CallbackHandle {
    let prefix = format!("{move_id}.");
    let name = program
        .callback_keys()
        .expect("Captain callback export")
        .into_iter()
        .find(|name| name.starts_with(&prefix) && name.ends_with(event))
        .unwrap_or_else(|| panic!("Captain callback {move_id}.{event} was not exported"));
    program.bind_callback(&name)
}

fn action_record<'a>(
    root: &'a BTreeMap<String, NativeValue>,
    name: &str,
) -> &'a BTreeMap<String, NativeValue> {
    let NativeValue::Dict(actions) = &root["actions"] else {
        panic!("Captain actions must be a dictionary");
    };
    let NativeValue::Dict(action) = &actions[name] else {
        panic!("Captain action {name} must be a dictionary");
    };
    action
}

fn object(kind: NativeKind, path: &str) -> NativeValue {
    NativeValue::Object(NativeObject {
        kind,
        path: path.into(),
    })
}

#[derive(Default)]
struct CaptainState {
    calls: Vec<(String, Vec<NativeValue>)>,
    sets: Vec<(String, NativeValue)>,
    action: String,
    stick: [f32; 2],
    facing: f32,
    velocity: [f32; 2],
    ground_velocity: f32,
}

struct CaptainHost {
    state: Arc<Mutex<CaptainState>>,
}

impl NativeHost for CaptainHost {
    fn get(&mut self, path: &str) -> Result<NativeValue, Error> {
        let state = self.state.lock().unwrap();
        let value = match path {
            "fighter.action" => NativeValue::String(state.action.clone()),
            "fighter.facing" => NativeValue::F32(state.facing),
            "fighter.action_state" => object(NativeKind::State, "fighter.action_state"),
            "fighter.velocity" => NativeValue::Vec2(state.velocity),
            "fighter.ground_velocity" => NativeValue::F32(state.ground_velocity),
            "context.input" => object(NativeKind::Input, "context.input"),
            "context.input.stick" => NativeValue::Vec2(state.stick),
            "context.rules" => object(NativeKind::Context, "context.rules"),
            "context.rules.specials" => object(NativeKind::Context, "context.rules.specials"),
            "context.rules.specials.vertical_threshold" => NativeValue::F32(0.5),
            "context.rules.specials.horizontal_threshold" => NativeValue::F32(0.5),
            "context.ground_open" => NativeValue::Bool(true),
            "context.air_open" => NativeValue::Bool(true),
            "context.event" => object(NativeKind::Context, "context.event"),
            "context.event.value" => NativeValue::Int(1),
            "neutral.attributes" => object(NativeKind::Value, "neutral.attributes"),
            "neutral.attributes.specialn_stick_range_y_neg" => NativeValue::F32(0.125),
            "neutral.attributes.specialn_stick_range_y_pos" => NativeValue::F32(0.625),
            "neutral.attributes.specialn_angle_diff" => NativeValue::F32(30.0),
            "neutral.attributes.specialn_vel_x" => NativeValue::F32(1.95),
            "neutral.attributes.specialn_vel_mul" => NativeValue::F32(0.92),
            "up.attributes" => object(NativeKind::Value, "up.attributes"),
            "up.attributes.specialhi_input_var" => NativeValue::F32(0.225),
            "up.attributes.specialhi_freefall_air_spd_mul" => NativeValue::F32(0.72),
            "up.attributes.specialhi_landing_lag" => NativeValue::Int(30),
            "down.attributes" => object(NativeKind::Value, "down.attributes"),
            "side.attributes" => object(NativeKind::Value, "side.attributes"),
            "side.attributes.specials_gr_vel_x" => NativeValue::F32(0.75),
            "hit" => object(NativeKind::Hit, "hit"),
            _ => return Err(Error::Host(format!("unexpected Captain get path {path}"))),
        };
        Ok(value)
    }

    fn set(&mut self, path: &str, value: NativeValue) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();
        if path == "fighter.facing" {
            if let NativeValue::F32(facing) = &value {
                state.facing = *facing;
            }
        }
        if path == "fighter.velocity" {
            if let NativeValue::List(values) = &value {
                if let [NativeValue::F32(x), NativeValue::F32(y)] = values.as_slice() {
                    state.velocity = [*x, *y];
                }
            }
        }
        if path == "fighter.ground_velocity" {
            if let NativeValue::F32(value) = &value {
                state.ground_velocity = *value;
            }
        }
        state.sets.push((path.to_owned(), value));
        Ok(())
    }

    fn call(&mut self, path: &str, args: &[NativeValue]) -> Result<NativeValue, Error> {
        self.state
            .lock()
            .unwrap()
            .calls
            .push((path.to_owned(), args.to_vec()));
        match path {
            "context.input.just_pressed" => Ok(NativeValue::Bool(true)),
            "context.resource" => {
                let Some(NativeValue::String(resource)) = args.first() else {
                    return Err(Error::Host("Captain resource path is not a string".into()));
                };
                Ok(object(NativeKind::Value, resource))
            }
            "hit.resource" => {
                let Some(NativeValue::String(resource)) = args.first() else {
                    return Err(Error::Host(
                        "Captain hit resource path is not a string".into(),
                    ));
                };
                Ok(object(NativeKind::Value, resource))
            }
            "fighter.change_action" | "fighter.enter_fall_special" => Ok(NativeValue::None),
            other => Err(Error::Host(format!("unexpected Captain call path {other}"))),
        }
    }

    fn call_named(
        &mut self,
        path: &str,
        args: &[NativeValue],
        named: &BTreeMap<String, NativeValue>,
    ) -> Result<NativeValue, Error> {
        self.state
            .lock()
            .unwrap()
            .calls
            .push((path.to_owned(), args.to_vec()));
        match path {
            "fighter.enter_fall_special" => {
                assert_eq!(named["mobility"], NativeValue::F32(0.72));
                assert_eq!(named["landing_lag"], NativeValue::Int(30));
                Ok(NativeValue::None)
            }
            other => Err(Error::Host(format!(
                "unexpected Captain named call path {other}"
            ))),
        }
    }
}

fn context() -> NativeValue {
    object(NativeKind::Context, "context")
}

#[test]
fn captain_definition_exports_resource_backed_callbacks_and_slippi_states() {
    let program = captain_program();
    let metadata = program
        .export_metadata()
        .expect("export Captain definition");
    let NativeValue::Dict(root) = metadata else {
        panic!("Captain definition must be a dictionary");
    };
    assert_eq!(root["name"], NativeValue::String("captain-falcon".into()));
    assert_eq!(
        root["external_ids"],
        NativeValue::List(vec![NativeValue::Int(0)])
    );

    let NativeValue::Dict(movesets) = &root["movesets"] else {
        panic!("Captain movesets must be a dictionary");
    };
    let NativeValue::Dict(specials) = &movesets["specials"] else {
        panic!("Captain specials must be a dictionary");
    };
    let expected = [
        ("neutral", "special.neutral.ground", 347),
        ("up", "special.up.ground", 353),
        ("down", "special.down.air", 359),
    ];
    for (slot, action_name, state) in expected {
        let NativeValue::String(move_id) = &specials[slot] else {
            panic!("Captain {slot} move id must be a string");
        };
        let action = action_record(&root, action_name);
        assert_eq!(
            action["source_behavior"],
            NativeValue::String(move_id.clone())
        );
        assert_eq!(action["slippi_state"], NativeValue::Int(state));
    }

    for move_id in ["move_0", "move_2", "move_3"] {
        for event in ["input_pressed", "animation_end"] {
            if program
                .callback_keys()
                .expect("Captain callbacks")
                .iter()
                .any(|name| name.starts_with(&format!("{move_id}.")) && name.ends_with(event))
            {
                let _ = callback(&program, move_id, event);
            }
        }
    }
}

#[test]
fn captain_callbacks_dispatch_against_resource_shaped_host() {
    let program = captain_program();
    let state = Arc::new(Mutex::new(CaptainState {
        action: "Action.WAIT".into(),
        stick: [0.0, 0.0],
        facing: 1.0,
        ..CaptainState::default()
    }));
    let host = shared_host(CaptainHost {
        state: Arc::clone(&state),
    });
    let fighter = HostRef::new(host.clone(), NativeKind::Fighter, "fighter");
    let context_value = context();

    // The three callbacks below are selected from exported logical slots,
    // ensuring the loader retained the actual bound Captain move methods.
    let punch = callback(&program, "move_0", "input_pressed");
    let raptor_boost = callback(&program, "move_1", "input_pressed");
    let dive = callback(&program, "move_2", "input_pressed");
    let kick = callback(&program, "move_3", "input_pressed");
    let punch_command = callback(&program, "move_0", "command_changed");
    let dive_command = callback(&program, "move_2", "command_changed");
    let dive_animation_end = callback(&program, "move_2", "animation_end");

    program.prepare_for_current_thread().unwrap();

    // Neutral B is intentionally dispatched with a neutral stick.  The host
    // action starts as WAIT and the callback must choose ground Falcon Punch.
    state.lock().unwrap().stick = [0.0, 0.0];
    assert_eq!(
        program
            .dispatch(&punch, fighter.clone(), &[context_value.clone()])
            .unwrap(),
        NativeValue::Bool(true)
    );

    // Side-B from WAIT selects the ground Raptor Boost start and mirrors the
    // native entry callback's atomic velocity and ground-velocity reset.
    state.lock().unwrap().stick = [0.6, 0.0];
    assert_eq!(
        program
            .dispatch(&raptor_boost, fighter.clone(), &[context_value.clone()])
            .unwrap(),
        NativeValue::Bool(true)
    );
    state.lock().unwrap().stick = [-0.6625, 0.7375];
    assert_eq!(
        program
            .dispatch(&dive, fighter.clone(), &[context_value.clone()])
            .unwrap(),
        NativeValue::Bool(true)
    );
    assert_eq!(
        program
            .dispatch(&kick, fighter.clone(), &[context_value])
            .unwrap(),
        NativeValue::Bool(false),
        "upward replay stick must not enter aerial Falcon Kick"
    );

    // The captured PlCa neutral attributes drive the aerial launch angle and
    // speed for this exact y=.5 command sample.
    {
        let mut state = state.lock().unwrap();
        state.action = "Action.SPECIAL_AIR_N_START".into();
        state.stick = [0.0, 0.5];
    }
    program
        .dispatch(&punch_command, fighter.clone(), &[context()])
        .unwrap();

    // The captured up-special input variable (.225) turns Falcon toward the
    // replay's stick x=-.6625 command before native model rotation/physics.
    {
        let mut state = state.lock().unwrap();
        state.action = "Action.SPECIAL_AIR_HI".into();
        state.stick = [-0.6625, 0.7375];
        state.facing = 1.0;
    }
    program
        .dispatch(&dive_command, fighter.clone(), &[context()])
        .unwrap();
    assert_eq!(state.lock().unwrap().facing, -1.0);

    // The captured up-special attributes pass unchanged through the terminal
    // animation boundary: .72 mobility and 30 frames of landing lag.
    state.lock().unwrap().action = "Action.SPECIAL_HI".into();
    program
        .dispatch(&dive_animation_end, fighter, &[context()])
        .unwrap();

    let host = state.lock().unwrap();
    assert!(
        host.calls
            .iter()
            .any(|(path, _)| path == "context.resource")
    );
    assert!(
        host.sets
            .iter()
            .any(|(path, _)| path == "fighter.action_frame")
    );
    let velocity = host
        .sets
        .iter()
        .find(|(path, value)| {
            path == "fighter.velocity"
                && matches!(value, NativeValue::List(values)
                    if values.len() == 2
                        && matches!((&values[0], &values[1]),
                            (NativeValue::F32(x), NativeValue::F32(y))
                                if (*x - 1.801565).abs() < 0.0001
                                    && (*y - 0.746232).abs() < 0.0001))
        })
        .map(|(_, value)| value);
    assert!(matches!(velocity, Some(NativeValue::List(values))
        if values.len() == 2
            && matches!((&values[0], &values[1]),
                (NativeValue::F32(x), NativeValue::F32(y))
                    if (*x - 1.801565).abs() < 0.0001
                        && (*y - 0.746232).abs() < 0.0001)));

    assert!(host.calls.iter().any(|(path, args)| {
        path == "fighter.change_action"
            && args.first() == Some(&NativeValue::String("special_s_start".into()))
    }));
    assert!(host.sets.iter().any(|(path, value)| {
        path == "fighter.velocity"
            && matches!(value, NativeValue::List(values)
                if values == &vec![NativeValue::F32(0.0), NativeValue::F32(0.0)])
    }));
    assert!(
        host.sets
            .iter()
            .any(|(path, value)| path == "fighter.ground_velocity"
                && *value == NativeValue::F32(0.0))
    );
}

#[test]
fn captain_grounded_down_special_dispatches_falcon_kick() {
    let program = captain_program();
    let state = Arc::new(Mutex::new(CaptainState {
        action: "Action.WAIT".into(),
        stick: [0.0, -0.8],
        facing: 1.0,
        ..CaptainState::default()
    }));
    let host = shared_host(CaptainHost {
        state: Arc::clone(&state),
    });
    let fighter = HostRef::new(host, NativeKind::Fighter, "fighter");
    let kick = callback(&program, "move_3", "input_pressed");

    program.prepare_for_current_thread().unwrap();
    assert_eq!(
        program.dispatch(&kick, fighter, &[context()]).unwrap(),
        NativeValue::Bool(true)
    );

    let host = state.lock().unwrap();
    assert!(host.calls.iter().any(|(path, args)| {
        path == "fighter.change_action"
            && args.first() == Some(&NativeValue::String("special_lw".into()))
    }));
    assert!(
        host.sets
            .iter()
            .any(|(path, value)| path == "fighter.action_frame" && *value == NativeValue::Int(1))
    );
}

#[test]
fn captain_raptor_boost_contact_enters_follow_through_with_ground_multiplier() {
    let program = captain_program();
    let state = Arc::new(Mutex::new(CaptainState {
        action: "Action.SPECIAL_S_START".into(),
        velocity: [2.0, 3.0],
        ground_velocity: 4.0,
        facing: 1.0,
        ..CaptainState::default()
    }));
    let host = shared_host(CaptainHost {
        state: Arc::clone(&state),
    });
    let fighter = HostRef::new(host.clone(), NativeKind::Fighter, "fighter");
    let hit = HostRef::new(host, NativeKind::Hit, "hit");
    let contact = callback(&program, "move_1", "before_hit");

    program.prepare_for_current_thread().unwrap();
    program
        .dispatch(&contact, fighter, &[host_object(&hit)])
        .unwrap();

    let host = state.lock().unwrap();
    assert_eq!(host.action, "Action.SPECIAL_S_START");
    assert!(host.calls.iter().any(|(path, args)| {
        path == "fighter.change_action"
            && args.first() == Some(&NativeValue::String("special_s".into()))
    }));
    assert!(host.sets.iter().any(|(path, value)| {
        path == "fighter.velocity"
            && *value == NativeValue::List(vec![NativeValue::F32(2.0), NativeValue::F32(0.0)])
    }));
    assert!(host.sets.iter().any(|(path, value)| {
        path == "fighter.ground_velocity" && *value == NativeValue::F32(3.0)
    }));
}
