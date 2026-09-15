use super::lifecycle::NativeContext;
use super::lifecycle_host::LifecycleHost;
use super::starlark::value::{NativeHost, NativeKind, NativeObject, NativeValue};
use crate::fighter::specials;
use crate::game::{Event, Match, MatchData};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

const FIXTURE: &str = include_str!("../../../tests/fixtures/game/integration-match.json");

fn match_with_projectile_resource() -> Match {
    let mut data: MatchData = serde_json::from_str(FIXTURE).expect("synthetic fixture");
    data.fighters[0].specials = Some(crate::game::script::resources::Specials {
        character: "projectile-test".into(),
        resources: crate::game::script::resources::Resources::new(BTreeMap::from([(
            "neutral".into(),
            json!({
                "laser": {
                    "lifetime": 18.0,
                    "move_id": 37,
                    "hitboxes": [{
                        "group": 0,
                        "bone": 0,
                        "center": [0.25, 0.0, 0.0],
                        "radius": 0.5,
                        "damage": 3,
                        "angle_degrees": 45.0,
                        "growth": 20,
                        "fixed": 0,
                        "base": 10
                    }]
                }
            }),
        )]))
        .expect("projectile resources"),
    });
    Match::new(data, 7).expect("synthetic match")
}

// Keep construction of a named call in one place so this test exercises the
// public native ABI shape used by Pon rather than a private PendingProjectile.
fn named_projectile(hitboxes: NativeValue, move_id: NativeValue) -> BTreeMap<String, NativeValue> {
    BTreeMap::from([
        ("kind".into(), NativeValue::String("laser".into())),
        (
            "position".into(),
            NativeValue::List(vec![
                NativeValue::F32(1.0),
                NativeValue::F32(2.0),
                NativeValue::F32(0.0),
            ]),
        ),
        ("angle".into(), NativeValue::F32(0.25)),
        ("speed".into(), NativeValue::F32(4.0)),
        ("lifetime".into(), NativeValue::F32(18.0)),
        ("hitboxes".into(), hitboxes),
        ("move_id".into(), move_id),
    ])
}

fn host_for(m: &Match) -> LifecycleHost {
    let data = &m.data().fighters[0];
    let cache = data.script_resources.get().expect("match resource cache");
    LifecycleHost::new_with_cache(
        &m.state().fighters[0],
        None,
        Value::Object(Map::new()),
        NativeContext::empty(),
        Some(data),
        Arc::clone(&cache),
    )
}

#[test]
fn named_projectile_commits_then_spawns_with_event_and_staling_identity() {
    let m = match_with_projectile_resource();
    let mut host = host_for(&m);
    let hitboxes = host
        .get("context.neutral.laser.hitboxes")
        .expect("linked projectile hitboxes handle");
    assert!(matches!(hitboxes, NativeValue::Object(_)));
    let move_id = host
        .get("context.neutral.laser.move_id")
        .expect("linked projectile move id");
    assert_eq!(move_id, NativeValue::Int(37));
    host.call_named(
        "fighter.emit_projectile",
        &[],
        &named_projectile(hitboxes, move_id),
    )
    .expect("named projectile call");

    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("lifecycle commit");
    assert_eq!(fighter.pending_projectiles.len(), 1);
    assert_eq!(m.state().fighters[0].pending_projectiles.len(), 0);

    let mut state = m.state().clone();
    state.fighters[0] = fighter;
    specials::emit_projectiles(m.data(), &mut state).expect("deferred spawn");
    let projectile = state.projectiles.first().expect("spawned projectile");
    assert_eq!(
        projectile.kind,
        crate::game::projectile::ProjectileKind::FoxLaser
    );
    assert_eq!(projectile.owner, 0);
    assert_eq!(projectile.position, [1.0, 2.0, 0.0]);
    assert_eq!(projectile.angle, 0.25);
    assert_eq!(projectile.speed, 4.0);
    assert_eq!(projectile.lifetime, 18.0);
    assert_eq!(projectile.hitboxes.len(), 1);
    assert_eq!(projectile.hitboxes[0].damage, 3);
    assert_eq!(projectile.staling_identity.move_id, 37);
    assert_eq!(
        state.events,
        vec![Event::ProjectileSpawned {
            owner: 0,
            projectile_kind: crate::game::projectile::ProjectileKind::FoxLaser,
        }]
    );
}

#[test]
fn invalid_followup_call_does_not_commit_staged_projectile() {
    let m = match_with_projectile_resource();
    let mut host = host_for(&m);
    let hitboxes = host
        .get("context.neutral.laser.hitboxes")
        .expect("linked projectile hitboxes handle");
    let move_id = host
        .get("context.neutral.laser.move_id")
        .expect("linked projectile move id");
    host.call_named(
        "fighter.emit_projectile",
        &[],
        &named_projectile(hitboxes, move_id),
    )
    .expect("valid staged call");
    let mut invalid = named_projectile(
        NativeValue::Object(NativeObject {
            kind: NativeKind::Value,
            path: "context.neutral.laser.hitboxes".into(),
        }),
        NativeValue::Int(37),
    );
    invalid.insert("move_id".into(), NativeValue::Int(-1));
    assert!(
        host.call_named("fighter.emit_projectile", &[], &invalid)
            .is_err()
    );

    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("valid staged call remains committable");
    assert_eq!(fighter.pending_projectiles.len(), 1);
    assert_eq!(fighter.pending_projectiles[0].move_id, 37);
    assert!(m.state().fighters[0].pending_projectiles.is_empty());
}

#[test]
fn projectile_queue_enforces_bounded_eight_command_limit() {
    let m = match_with_projectile_resource();
    let mut host = host_for(&m);
    let hitboxes = host
        .get("context.neutral.laser.hitboxes")
        .expect("linked projectile hitboxes handle");
    let move_id = host
        .get("context.neutral.laser.move_id")
        .expect("linked projectile move id");
    for _ in 0..crate::game::projectile::MAX_PENDING_PROJECTILES {
        host.call_named(
            "fighter.emit_projectile",
            &[],
            &named_projectile(hitboxes.clone(), move_id.clone()),
        )
        .expect("queue capacity");
    }
    let error = host
        .call_named(
            "fighter.emit_projectile",
            &[],
            &named_projectile(hitboxes, move_id),
        )
        .expect_err("ninth projectile must be rejected");
    assert!(
        error
            .to_string()
            .contains("projectile emission queue is full")
    );
}
