use super::lifecycle::NativeContext;
use super::lifecycle_host::LifecycleHost;
use super::starlark::value::{NativeHost, NativeKind, NativeObject, NativeValue};
use crate::fighter::specials;
use crate::game::script::events::{AnimationEventId, Dispatcher, Event as ScriptEvent, EventKind};
use crate::game::script::resources::{ArticleId, ArticleResource};
use crate::game::{BUTTON_B, Event, Match, MatchData};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::sync::Arc;

const FIXTURE: &str = include_str!("../../../tests/fixtures/game/integration-match.json");

fn match_with_projectile_resource() -> Match {
    let mut data: MatchData = serde_json::from_str(FIXTURE).expect("synthetic fixture");
    data.fighters[0].specials = Some(crate::game::script::resources::Specials {
        character: "projectile-test".into(),
        special_attributes: None,
        animations: None,
        articles: None,
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

fn typed_article_match() -> Match {
    let mut data: MatchData = serde_json::from_str(FIXTURE).expect("synthetic fixture");
    data.fighters[0].specials = Some(crate::game::script::resources::Specials {
        character: "fox".into(),
        special_attributes: None,
        animations: None,
        articles: Some(BTreeMap::from([(
            ArticleId::FOX_LASER,
            ArticleResource::Ray {
                lifetime: 18.0,
                move_id: 37,
                hitboxes: vec![
                    serde_json::from_value(json!({
                        "group": 0,
                        "bone": 0,
                        "center": [0.25, 0.0, 0.0],
                        "radius": 0.5,
                        "damage": 3,
                        "angle_degrees": 45.0,
                        "growth": 20,
                        "fixed": 0,
                        "base": 10
                    }))
                    .unwrap(),
                ],
            },
        )])),
        resources: crate::game::script::resources::Resources::default(),
    });
    Match::new(data, 7).expect("typed article fixture")
}

fn typed_falco_article_match() -> Match {
    let mut data: MatchData = serde_json::from_str(FIXTURE).expect("synthetic fixture");
    data.fighters[0].specials = Some(crate::game::script::resources::Specials {
        character: "falco".into(),
        special_attributes: None,
        animations: None,
        articles: Some(BTreeMap::from([(
            ArticleId::FALCO_LASER,
            ArticleResource::Ray {
                lifetime: 100.0,
                move_id: 37,
                hitboxes: vec![
                    serde_json::from_value(json!({
                        "group": 0,
                        "bone": 0,
                        "center": [0.25, 0.0, 0.0],
                        "radius": 0.5,
                        "damage": 3,
                        "angle_degrees": 0.0,
                        "growth": 100,
                        "fixed": 5,
                        "base": 0
                    }))
                    .unwrap(),
                ],
            },
        )])),
        resources: crate::game::script::resources::Resources::default(),
    });
    Match::new(data, 7).expect("typed Falco article fixture")
}

fn typed_gravity_article_match() -> Match {
    let mut data: MatchData = serde_json::from_str(FIXTURE).expect("synthetic fixture");
    data.fighters[0].specials = Some(
        serde_json::from_value(json!({
            "character": "mario",
            "articles": {
                "48": {
                    "kind": "mario_fireball",
                    "speed": 1.5,
                    "angle": 0.4,
                    "lifetime": 60.0,
                    "half_life": 30.0,
                    "gravity": 0.08,
                    "terminal_velocity": 2.4,
                    "surface_multiplier": 0.5,
                    "terrain_stop_speed": 0.2,
                    "move_id": 20,
                    "hitboxes": [{
                        "group": 0,
                        "bone": 0,
                        "center": [0.0, 0.0, 0.0],
                        "radius": 0.5,
                        "damage": 3,
                        "angle_degrees": 45.0,
                        "growth": 20,
                        "fixed": 0,
                        "base": 10
                    }],
                    "contact": {
                        "reflection": "none",
                        "shield": "bounce",
                        "persistence": "despawn"
                    }
                }
            }
        }))
        .expect("gravity article fixture"),
    );
    Match::new(data, 7).expect("typed gravity article fixture")
}

fn legacy_fox_match() -> Match {
    let mut data = match_with_projectile_resource().data().clone();
    data.fighters[0]
        .specials
        .as_mut()
        .expect("legacy specials")
        .character = "fox".into();
    Match::new(data, 7).expect("legacy Fox article fixture")
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

#[test]
fn typed_article_spawn_stages_only_numeric_compact_command() {
    let m = typed_article_match();
    let mut host = host_for(&m);
    host.call(
        "fighter.spawn_article",
        &[
            NativeValue::Int(i64::from(ArticleId::FOX_LASER.0)),
            NativeValue::List(vec![
                NativeValue::F32(1.0),
                NativeValue::F32(2.0),
                NativeValue::F32(0.0),
            ]),
            NativeValue::F32(0.25),
            NativeValue::F32(4.0),
        ],
    )
    .expect("typed article call");
    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("article commit");
    assert_eq!(fighter.pending_article_spawns.len(), 1);
    assert!(fighter.pending_projectiles.is_empty());
    let mut state = m.state().clone();
    state.fighters[0] = fighter;
    specials::emit_projectiles(m.data(), &mut state).expect("typed article drain");
    let projectile = state.projectiles.first().expect("ray spawned");
    assert_eq!(
        projectile.kind,
        crate::game::projectile::ProjectileKind::FoxLaser
    );
    assert_eq!(projectile.lifetime, 18.0);
    assert_eq!(projectile.hitboxes[0].damage, 3);
}

#[test]
fn typed_falco_laser_spawn_preserves_native_kind_and_exported_lifetime() {
    let m = typed_falco_article_match();
    let mut host = host_for(&m);
    host.call(
        "fighter.spawn_article",
        &[
            NativeValue::Int(i64::from(ArticleId::FALCO_LASER.0)),
            NativeValue::List(vec![
                NativeValue::F32(1.0),
                NativeValue::F32(2.0),
                NativeValue::F32(0.0),
            ]),
            NativeValue::F32(0.0),
            NativeValue::F32(5.0),
        ],
    )
    .expect("Falco laser article call");
    let mut state = m.state().clone();
    let mut fighter = state.fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("Falco laser article commit");
    state.fighters[0] = fighter;

    specials::emit_projectiles(m.data(), &mut state).expect("Falco laser drain");
    let projectile = state.projectiles.first().expect("Falco laser spawned");
    assert_eq!(
        projectile.kind,
        crate::game::projectile::ProjectileKind::FalcoLaser
    );
    assert_eq!(projectile.lifetime, 100.0);
    assert_eq!(projectile.speed, 5.0);
    assert_eq!(projectile.hitboxes[0].growth, 100);
    assert_eq!(projectile.hitboxes[0].fixed, 5);
}

#[test]
fn typed_article_spawn_rejects_string_ids_without_staging() {
    let m = typed_article_match();
    let mut host = host_for(&m);
    let error = host
        .call(
            "fighter.spawn_article",
            &[
                NativeValue::String("fox_laser".into()),
                NativeValue::List(vec![NativeValue::F32(0.0); 3]),
                NativeValue::F32(0.0),
                NativeValue::F32(1.0),
            ],
        )
        .expect_err("string article id must be rejected");
    assert!(error.to_string().contains("numeric"));
    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("rollback commit");
    assert!(fighter.pending_article_spawns.is_empty());
}

#[test]
fn mario_fireball_spawn_uses_article_owned_launch() {
    let m = typed_gravity_article_match();
    let mut host = host_for(&m);
    host.call(
        "fighter.spawn_article",
        &[
            NativeValue::Int(i64::from(ArticleId::MARIO_FIRE.0)),
            NativeValue::List(vec![
                NativeValue::F32(1.0),
                NativeValue::F32(2.0),
                NativeValue::F32(0.0),
            ]),
            NativeValue::F32(-1.0),
        ],
    )
    .expect("compact gravity article call");
    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("gravity article commit");
    assert_eq!(fighter.pending_article_spawns.len(), 1);
    let mut state = m.state().clone();
    state.fighters[0] = fighter;
    specials::emit_projectiles(m.data(), &mut state).expect("gravity article drain");
    let projectile = state.projectiles.first().expect("gravity article spawned");
    assert_eq!(
        projectile.kind,
        crate::game::projectile::ProjectileKind::Gravity(ArticleId::MARIO_FIRE)
    );
    assert!((projectile.velocity[0] + 1.5 * libm::cosf(0.4)).abs() < 1e-6);
    assert!((projectile.velocity[1] - 1.5 * libm::sinf(0.4)).abs() < 1e-6);
    assert_eq!(projectile.half_life, Some(30.0));
    assert!(matches!(
        projectile.behavior,
        crate::game::projectile::ProjectileBehavior::MarioFireball(_)
    ));
}

#[test]
fn legacy_fox_projectile_call_adapts_to_numeric_article_command() {
    let m = legacy_fox_match();
    let mut host = host_for(&m);
    let hitboxes = host
        .get("context.neutral.laser.hitboxes")
        .expect("legacy laser hitboxes");
    let move_id = host
        .get("context.neutral.laser.move_id")
        .expect("legacy laser move id");
    host.call_named(
        "fighter.emit_projectile",
        &[],
        &named_projectile(hitboxes, move_id),
    )
    .expect("legacy adapter call");
    let mut fighter = m.state().fighters[0].clone();
    host.commit_into(&mut fighter, None).expect("legacy commit");
    assert_eq!(fighter.pending_article_spawns.len(), 1);
    assert!(fighter.pending_projectiles.is_empty());
    assert_eq!(
        fighter.pending_article_spawns[0].article_id,
        ArticleId::FOX_LASER
    );
}

#[test]
fn typed_article_drain_commits_projectile_event_and_consumes_queue() {
    let m = typed_article_match();
    let mut host = host_for(&m);
    host.call(
        "fighter.spawn_article",
        &[
            NativeValue::Int(i64::from(ArticleId::FOX_LASER.0)),
            NativeValue::List(vec![
                NativeValue::F32(1.0),
                NativeValue::F32(2.0),
                NativeValue::F32(0.0),
            ]),
            NativeValue::F32(0.25),
            NativeValue::F32(4.0),
        ],
    )
    .expect("article launch command");
    let mut state = m.state().clone();
    let mut fighter = state.fighters[0].clone();
    host.commit_into(&mut fighter, None)
        .expect("article commit");
    assert_eq!(fighter.pending_article_spawns.len(), 1);
    state.fighters[0] = fighter;

    specials::emit_projectiles(m.data(), &mut state).expect("deferred article drain");
    assert!(state.fighters[0].pending_article_spawns.is_empty());
    assert_eq!(state.projectiles.len(), 1);
    assert_eq!(
        state.events,
        vec![Event::ProjectileSpawned {
            owner: 0,
            projectile_kind: crate::game::projectile::ProjectileKind::FoxLaser,
        }]
    );
}

#[test]
fn script_event_boundary_preserves_button_and_animation_ids() {
    let owner = crate::game::script::scheduler::OwnerId::new(23);
    let mut dispatcher = Dispatcher::default();
    let input_subscription = dispatcher
        .subscribe(owner, EventKind::InputPressed)
        .expect("input subscription");
    let animation_subscription = dispatcher
        .subscribe(owner, EventKind::AnimationEvent)
        .expect("animation subscription");

    let input = ScriptEvent::InputPressed {
        frame: 12,
        player: 0,
        buttons: BUTTON_B,
    };
    assert_eq!(input.kind(), EventKind::InputPressed);
    assert_eq!(input.frame(), 12);
    assert_eq!(dispatcher.dispatch(&input), vec![input_subscription]);

    let animation = ScriptEvent::AnimationEvent {
        frame: 13,
        player: 0,
        action: crate::game::Action::SpecialNStart,
        event_id: AnimationEventId(2),
    };
    assert_eq!(animation.kind(), EventKind::AnimationEvent);
    assert_eq!(animation.frame(), 13);
    assert!(matches!(
        animation,
        ScriptEvent::AnimationEvent {
            event_id: AnimationEventId(2),
            ..
        }
    ));
    assert_eq!(AnimationEventId(2).bit(), 0b100);
    assert_eq!(
        dispatcher.dispatch(&animation),
        vec![animation_subscription]
    );
}
