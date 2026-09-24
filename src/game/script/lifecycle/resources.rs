//! Immutable, pre-indexed lifecycle resources.
//!
//! Resource indexing belongs to match construction. A callback receives a
//! cheap path handle into this cache; it never serializes `FighterData`, pose
//! trees, or attack frame geometry while the simulation is running. The
//! Starlark backend can project a handle lazily into its safe value type.

use super::Error;
use super::action_events::{
    ActionEventTable, ClockSpec, CountdownSpec, FrameSpec, MarkerId, MarkerSpec,
};
use super::lifecycle_host::json_to_native;
use super::motion::{MotionProfile, MotionProfileId};
use super::motion_resources::{
    MotionDescriptor, MotionParameterSource, SpecialAttributeRef, link_profile,
};
use super::starlark::value as native;
use crate::game::data::{FighterData, Hitbox, Rules};
use crate::game::projectile::ProjectileKind;
use crate::game::script::definition::ActionDefinition;
use crate::game::script::resources::AttackId;
use crate::game::script::resources::{
    ArticleId, ArticleResource, LuigiFireballContactPolicy, ProjectileContactPolicy,
};
use serde_json::{Map, Value as JsonValue};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const MAX_PATH_BYTES: usize = 256;
type Action = crate::game::Action;
type ActionMap<T> = BTreeMap<Action, T>;
type OwnedActionMap<T> = BTreeMap<(usize, Action), T>;

#[derive(Clone, Debug)]
struct AttackMeta {
    samples: Arc<[JsonValue]>,
    validation: Result<(), String>,
}

type CommandTraceRows = Arc<[[Option<u32>; 4]]>;
type AnimationEventMaskRows = Arc<[u8]>;

#[derive(Clone, Debug)]
struct ProjectileMeta {
    lifetime: f32,
    hitboxes: Arc<[Hitbox]>,
    move_id: u16,
}

#[derive(Clone, Debug)]
pub(crate) struct ArticleMeta {
    pub(crate) behavior: ArticleBehavior,
}

#[derive(Clone, Debug)]
pub(crate) enum ArticleBehavior {
    Ray {
        kind: ProjectileKind,
        lifetime: f32,
        hitboxes: Arc<[Hitbox]>,
        move_id: u16,
    },
    Gravity {
        speed: f32,
        angle: f32,
        lifetime: f32,
        half_life: f32,
        gravity: f32,
        terminal_velocity: f32,
        surface_multiplier: f32,
        terrain_stop_speed: f32,
        hitboxes: Arc<[Hitbox]>,
        move_id: u16,
        contact: ProjectileContactPolicy,
    },
    MarioFireball {
        speed: f32,
        angle: f32,
        lifetime: f32,
        half_life: f32,
        gravity: f32,
        terminal_velocity: f32,
        surface_multiplier: f32,
        terrain_stop_speed: f32,
        hitboxes: Arc<[Hitbox]>,
        move_id: u16,
        contact: ProjectileContactPolicy,
    },
    LuigiFireball {
        speed: f32,
        angle: f32,
        lifetime: f32,
        half_life: f32,
        gravity: f32,
        terminal_velocity: f32,
        surface_multiplier: f32,
        terrain_stop_speed: f32,
        hitboxes: Arc<[Hitbox]>,
        move_id: u16,
        contact: ProjectileContactPolicy,
    },
}

/// Read-only resource data shared by all dispatches for one fighter.
#[derive(Clone, Debug)]
pub(crate) struct ResourceCache {
    special_attributes: Option<super::resources::SpecialAttributes>,
    values: Arc<BTreeMap<String, JsonValue>>,
    attacks: Arc<BTreeMap<String, AttackMeta>>,
    array_counts: Arc<BTreeMap<String, usize>>,
    /// The immutable program selected while the resource projection is built.
    /// Keeping this beside the cache makes the normal dispatch path a cheap
    /// Arc clone; it never reparses or clones the source on each hook.
    program: Option<Arc<super::Program>>,
    profiles: Arc<Vec<MotionProfile>>,
    action_profiles: Arc<Vec<Option<MotionProfileId>>>,
    action_profiles_custom: Arc<ActionMap<MotionProfileId>>,
    action_profiles_by_owner: Arc<OwnedActionMap<MotionProfileId>>,
    action_delay_fields: Arc<Vec<Option<String>>>,
    action_delay_fields_custom: Arc<ActionMap<Option<String>>>,
    action_delay_fields_by_owner: Arc<OwnedActionMap<Option<String>>>,
    action_animation_loops: Arc<Vec<bool>>,
    action_animation_loops_custom: Arc<ActionMap<bool>>,
    action_animation_loops_by_owner: Arc<OwnedActionMap<bool>>,
    action_attacks_by_owner: Arc<OwnedActionMap<AttackId>>,
    command_traces: Arc<ActionMap<CommandTraceRows>>,
    command_traces_by_owner: Arc<OwnedActionMap<CommandTraceRows>>,
    animation_event_masks: Arc<ActionMap<AnimationEventMaskRows>>,
    animation_event_masks_by_owner: Arc<OwnedActionMap<AnimationEventMaskRows>>,
    projectile_resources: Arc<BTreeMap<String, ProjectileMeta>>,
    articles: Arc<BTreeMap<ArticleId, ArticleMeta>>,
    action_events: Arc<ActionEventTable>,
    /// Native states whose motion table entry links to a complete executable
    /// `specials.animations` resource.  This is built once at registration;
    /// callback queries are binary searches over the sorted ids.
    complete_animation_states: Arc<[u32]>,
}

impl MotionParameterSource for ResourceCache {
    fn number_path(&self, path: &str) -> Option<f32> {
        self.value_path(path)
            .and_then(|value| value.as_f64())
            .filter(|value| value.is_finite())
            .filter(|value| *value >= f64::from(f32::MIN) && *value <= f64::from(f32::MAX))
            .map(|value| value as f32)
    }

    fn special_attribute(&self, reference: SpecialAttributeRef) -> Option<f32> {
        self.special_attribute(reference.layout, reference.field_id)
    }
}

struct LinkedMotionParameters<'a> {
    values: &'a JsonValue,
    resources: &'a ResourceCache,
}

impl MotionParameterSource for LinkedMotionParameters<'_> {
    fn number_path(&self, path: &str) -> Option<f32> {
        <JsonValue as MotionParameterSource>::number_path(self.values, path)
    }

    fn special_attribute(&self, reference: SpecialAttributeRef) -> Option<f32> {
        self.resources
            .special_attribute(reference.layout, reference.field_id)
    }
}

/// Match-owned handle for the immutable resource projection. The handle is
/// intentionally ignored by resource equality and serde: it is derived from
/// the serialized fighter/rules data at match construction and must never
/// affect replay identity or checkpoints.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResourceCacheHandle(Option<Arc<ResourceCache>>);

impl PartialEq for ResourceCacheHandle {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for ResourceCacheHandle {}

impl ResourceCacheHandle {
    pub(crate) fn get(&self) -> Option<Arc<ResourceCache>> {
        self.0.clone()
    }

    pub(crate) fn replace(&mut self, cache: Arc<ResourceCache>) {
        self.0 = Some(cache);
    }
}

impl ResourceCache {
    /// Build once during match/resource construction. `check_attacks` should
    /// be true at the validation boundary and false only for a trusted cache
    /// assembled after validation.
    pub(crate) fn build(
        data: Option<&FighterData>,
        rules: Option<&Rules>,
        check_attacks: bool,
    ) -> Result<Self, Error> {
        let (mut values, attacks) = resource_views(data, rules, check_attacks)?;
        let projectile_resources = link_projectile_resources(data)?;
        let articles = link_article_resources(data, &projectile_resources)?;
        if let Some(data) = data {
            if let Some(escape_air) = &data.escape_air {
                let value = serde_json::to_value(escape_air).map_err(|error| {
                    Error::Invalid(format!("invalid escape-air resource: {error}"))
                })?;
                values.insert("escape_air".into(), sanitized_view(&value));
            }
            // These two compact structures are safe to project eagerly. They
            // are shared parameters, never pose or attack-frame trees.
            let locomotion = serde_json::to_value(data.locomotion)
                .map_err(|error| Error::Invalid(format!("invalid locomotion resource: {error}")))?;
            values.insert("locomotion".into(), sanitized_view(&locomotion));
            // Motion descriptors use `fighter.*` for compact native movement
            // parameters. Project only the typed movement record needed by
            // the linker; this is immutable registration data, not a runtime
            // fighter object or a per-frame VM view.
            let movement = serde_json::to_value(&data.movement)
                .map_err(|error| Error::Invalid(format!("invalid movement resource: {error}")))?;
            values.insert("fighter".into(), sanitized_view(&movement));
            // Motion constructors authored against the shared parameter
            // namespace use `movement.*`; retain the same immutable record
            // under that canonical path as the fighter compatibility alias.
            values.insert("movement".into(), sanitized_view(&movement));
        }
        if let Some(rules) = rules {
            let rules = serde_json::to_value(rules)
                .map_err(|error| Error::Invalid(format!("invalid rules resource: {error}")))?;
            values.insert("rules".into(), sanitized_view(&rules));
        }
        let array_counts = values
            .iter()
            .filter_map(|(path, value)| value.as_array().map(|items| (path.clone(), items.len())))
            .collect();
        let program = data
            .map(crate::game::script::definition::program_for_registration)
            .transpose()?
            .flatten();
        let mut cache = Self {
            special_attributes: data
                .and_then(|fighter| fighter.specials.as_ref())
                .and_then(|specials| specials.special_attributes.clone()),
            values: Arc::new(values),
            attacks: Arc::new(attacks),
            array_counts: Arc::new(array_counts),
            program,
            profiles: Arc::default(),
            action_profiles: Arc::default(),
            action_profiles_custom: Arc::default(),
            action_profiles_by_owner: Arc::default(),
            action_delay_fields: Arc::default(),
            action_delay_fields_custom: Arc::default(),
            action_delay_fields_by_owner: Arc::default(),
            action_animation_loops: Arc::default(),
            action_animation_loops_custom: Arc::default(),
            action_animation_loops_by_owner: Arc::default(),
            action_attacks_by_owner: Arc::default(),
            command_traces: Arc::default(),
            command_traces_by_owner: Arc::default(),
            animation_event_masks: Arc::default(),
            animation_event_masks_by_owner: Arc::default(),
            projectile_resources: Arc::new(projectile_resources),
            articles: Arc::new(articles),
            action_events: Arc::new(
                ActionEventTable::compile(Vec::new(), Vec::new(), Vec::new())
                    .map_err(|error| Error::Invalid(error.to_string()))?,
            ),
            complete_animation_states: link_complete_animation_states(data),
        };
        cache.action_events = Arc::new(cache.compile_action_events()?);
        let environment = cache.environment()?;
        cache.link_program(&environment)?;
        cache.link_motion_profiles(data, rules)?;
        cache.link_attack_resources(data)?;
        cache.link_command_traces()?;
        Ok(cache)
    }

    pub(crate) fn projectile_hitboxes(&self, path: &str) -> Option<&[Hitbox]> {
        self.projectile_resources
            .get(path)
            .map(|meta| meta.hitboxes.as_ref())
    }

    pub(crate) fn article(&self, id: ArticleId) -> Option<&ArticleMeta> {
        self.articles.get(&id)
    }

    /// Map the old stringly projectile adapter onto the numeric catalog when
    /// its descriptor is the same native article.  This runs only at the
    /// compatibility boundary; typed callbacks use `article` directly.
    pub(crate) fn legacy_article(
        &self,
        kind: ProjectileKind,
        hitboxes_path: &str,
        lifetime: f32,
        move_id: u16,
    ) -> Option<ArticleId> {
        let id = match kind {
            ProjectileKind::FoxLaser => ArticleId::FOX_LASER,
            ProjectileKind::FalcoLaser => ArticleId::FALCO_LASER,
            ProjectileKind::LuigiFire => return None,
            ProjectileKind::Gravity(_) => return None,
        };
        let meta = self.article(id)?;
        let ArticleBehavior::Ray {
            lifetime: article_lifetime,
            move_id: article_move_id,
            hitboxes: article_hitboxes,
            ..
        } = &meta.behavior
        else {
            return None;
        };
        (*article_lifetime == lifetime
            && *article_move_id == move_id
            && self
                .projectile_resources
                .get(hitboxes_path)
                .is_some_and(|legacy| {
                    legacy.move_id == *article_move_id
                        && legacy.lifetime == *article_lifetime
                        && legacy.hitboxes.as_ref() == article_hitboxes.as_ref()
                }))
        .then_some(id)
    }

    pub(crate) fn projectile_move_id(&self, path: &str) -> Option<u16> {
        path.strip_suffix(".move_id")
            .and_then(|descriptor| {
                self.projectile_resources
                    .get(&format!("{descriptor}.hitboxes"))
            })
            .map(|meta| meta.move_id)
    }

    pub(crate) fn program(&self) -> Option<Arc<super::Program>> {
        self.program.clone()
    }

    pub(crate) fn motion_profile(
        &self,
        action: Action,
    ) -> Option<(MotionProfileId, &MotionProfile)> {
        let id = match action.builtin_index() {
            Some(index) => self.action_profiles.get(index)?.as_ref()?,
            None => self.action_profiles_custom.get(&action)?,
        };
        Some((*id, self.profiles.get(usize::from(id.0))?))
    }

    pub(crate) fn motion_profile_by_id(&self, id: MotionProfileId) -> Option<&MotionProfile> {
        self.profiles.get(usize::from(id.0))
    }

    pub(crate) fn profile_delay_field(&self, action: Action) -> Option<&str> {
        match action.builtin_index() {
            Some(index) => self
                .action_delay_fields
                .get(index)
                .and_then(Option::as_deref),
            None => self
                .action_delay_fields_custom
                .get(&action)
                .and_then(Option::as_deref),
        }
    }

    pub(crate) fn motion_profile_for_owner(
        &self,
        owner: Option<usize>,
        action: Action,
    ) -> Option<(MotionProfileId, &MotionProfile)> {
        match owner {
            Some(owner) => self
                .action_profiles_by_owner
                .get(&(owner, action))
                .copied()
                .and_then(|id| {
                    self.profiles
                        .get(usize::from(id.0))
                        .map(|profile| (id, profile))
                }),
            None => self.motion_profile(action),
        }
    }

    pub(crate) fn profile_delay_field_for_owner(
        &self,
        owner: Option<usize>,
        action: Action,
    ) -> Option<&str> {
        match owner {
            Some(owner) => self
                .action_delay_fields_by_owner
                .get(&(owner, action))
                .and_then(Option::as_deref),
            None => self.profile_delay_field(action),
        }
    }

    /// Whether the registered action's finite animation samples cycle while
    /// its native action frame continues advancing. This is immutable action
    /// metadata compiled at resource registration.
    pub(crate) fn animation_loop(&self, action: Action) -> bool {
        match action.builtin_index() {
            Some(index) => self
                .action_animation_loops
                .get(index)
                .copied()
                .unwrap_or(false),
            None => self
                .action_animation_loops_custom
                .get(&action)
                .copied()
                .unwrap_or(false),
        }
    }

    pub(crate) fn animation_loop_for_owner(&self, owner: Option<usize>, action: Action) -> bool {
        match owner {
            Some(owner) => self
                .action_animation_loops_by_owner
                .get(&(owner, action))
                .copied()
                .unwrap_or(false),
            None => self.animation_loop(action),
        }
    }

    pub(crate) fn action_events(&self) -> &ActionEventTable {
        &self.action_events
    }

    /// Return whether a numeric native motion state has a complete,
    /// executable animation resource.  The immutable index makes this a
    /// logarithmic hot-path lookup and naturally fails closed for absent or
    /// unsupported animation data.
    pub(crate) fn has_complete_animation(&self, state_id: u32) -> bool {
        self.complete_animation_states
            .binary_search(&state_id)
            .is_ok()
    }

    pub(crate) fn special_attribute(&self, layout: u8, field_id: u16) -> Option<f32> {
        let attributes = self.special_attributes.as_ref()?;
        (attributes.layout == layout)
            .then(|| attributes.get(field_id))
            .flatten()
    }

    pub(crate) fn command_trace(
        &self,
        owner: Option<usize>,
        action: Action,
    ) -> Option<&[[Option<u32>; 4]]> {
        match owner {
            Some(owner) => self
                .command_traces_by_owner
                .get(&(owner, action))
                .map(AsRef::as_ref),
            None => self.command_traces.get(&action).map(AsRef::as_ref),
        }
    }

    pub(crate) fn animation_event_masks(
        &self,
        owner: Option<usize>,
        action: Action,
    ) -> Option<&[u8]> {
        match owner {
            Some(owner) => self
                .animation_event_masks_by_owner
                .get(&(owner, action))
                .map(AsRef::as_ref),
            None => self.animation_event_masks.get(&action).map(AsRef::as_ref),
        }
    }

    fn link_command_traces(&mut self) -> Result<(), Error> {
        let Some(program) = self.program.as_ref() else {
            return Ok(());
        };
        let mut root = BTreeMap::new();
        let mut event_masks_root = BTreeMap::new();
        for (canonical, action) in selected_action_definitions(&program.metadata().actions) {
            if let Some(source) = action.source_behavior.as_deref() {
                let behavior = program
                    .metadata()
                    .behaviors
                    .iter()
                    .find(|behavior| behavior.id.as_deref() == Some(source))
                    .ok_or_else(|| {
                        Error::Invalid(format!("root action source behavior {source:?} is unknown"))
                    })?;
                if behavior
                    .resource
                    .as_deref()
                    .is_some_and(|path| self.value(path).is_none())
                {
                    continue;
                }
            }
            self.link_action_sidecars(canonical, action, &mut root, &mut event_masks_root)?;
        }
        let mut owned = BTreeMap::new();
        let mut event_masks_owned = BTreeMap::new();
        for (owner, behavior) in program.metadata().behaviors.iter().enumerate() {
            // A behavior resource is an optional enablement gate.  If its
            // declared root is absent, the definition omits that behavior
            // entirely, including its optional action resources.  Once the
            // root exists, however, every referenced action resource remains
            // strict and must link successfully below.
            if behavior
                .resource
                .as_deref()
                .is_some_and(|path| self.value(path).is_none())
            {
                continue;
            }
            for (canonical, action) in selected_action_definitions(&behavior.actions) {
                self.link_action_sidecars(
                    (owner, canonical),
                    action,
                    &mut owned,
                    &mut event_masks_owned,
                )?;
            }
        }
        self.command_traces = Arc::new(root);
        self.command_traces_by_owner = Arc::new(owned);
        self.animation_event_masks = Arc::new(event_masks_root);
        self.animation_event_masks_by_owner = Arc::new(event_masks_owned);
        Ok(())
    }

    fn link_action_sidecars<K>(
        &self,
        key: K,
        action: &ActionDefinition,
        traces: &mut BTreeMap<K, CommandTraceRows>,
        event_masks: &mut BTreeMap<K, AnimationEventMaskRows>,
    ) -> Result<(), Error>
    where
        K: Clone + Ord,
    {
        if let Some(path) = action.command_trace.as_deref() {
            self.validate_command_trace_lengths(path, action.attack.as_deref())?;
            traces.insert(key.clone(), self.compile_command_trace(path)?);
            if let Some(masks) =
                self.compile_animation_event_masks(path, action.attack.as_deref())?
            {
                event_masks.insert(key.clone(), masks);
            }
            return Ok(());
        }

        // Some native actions have no command variables at all but still
        // expose animation events.  Their compact sidecar lives beside the
        // action's animation resource, so it must be linked independently of
        // the command-trace cache.
        let Some(path) = action.attack.as_deref() else {
            return Ok(());
        };
        if self
            .value_path(&format!("{path}.animation_event_masks"))
            .is_some()
            && let Some(masks) = self.compile_animation_event_masks(path, Some(path))?
        {
            event_masks.insert(key, masks);
        }
        Ok(())
    }

    fn validate_command_trace_lengths(
        &self,
        command_path: &str,
        attack_path: Option<&str>,
    ) -> Result<(), Error> {
        // Standalone traces are a supported cache API (used by generic
        // command-trace consumers); parity is meaningful only when the
        // action descriptor supplies its pose resource.
        let Some(attack_path) = attack_path else {
            return Ok(());
        };
        let expected = self.frame_count(attack_path);
        if expected == 0 {
            return Err(Error::Invalid(format!(
                "command trace {command_path:?} references missing pose resource {attack_path:?}"
            )));
        }
        let command_rows = self
            .array_length_path(&format!("{command_path}.cmd_vars"))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "command trace {command_path:?} is missing cmd_vars"
                ))
            })?;
        let interrupt_rows = self
            .array_length_path(&format!("{command_path}.allow_interrupt"))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "command trace {command_path:?} is missing allow_interrupt"
                ))
            })?;
        if command_rows != expected || interrupt_rows != expected {
            return Err(Error::Invalid(format!(
                "command trace {command_path:?} has {command_rows}/{interrupt_rows} rows, expected {expected}"
            )));
        }
        Ok(())
    }

    fn compile_command_trace(&self, path: &str) -> Result<CommandTraceRows, Error> {
        let Some(JsonValue::Array(rows)) = self.value_path(&format!("{path}.cmd_vars")) else {
            return Err(Error::Invalid(format!(
                "unknown command trace resource {path:?}"
            )));
        };
        if rows.is_empty() || rows.len() > 4_096 {
            return Err(Error::Invalid(format!(
                "command trace {path:?} must have 1..=4096 frames"
            )));
        }
        let mut linked = Vec::with_capacity(rows.len());
        for (frame, row) in rows.iter().enumerate() {
            let Some(columns) = row.as_array() else {
                return Err(Error::Invalid(format!(
                    "command trace {path:?} frame {frame} must be an array"
                )));
            };
            if columns.len() != 4 {
                return Err(Error::Invalid(format!(
                    "command trace {path:?} frame {frame} must have exactly four columns"
                )));
            }
            let mut typed = [None; 4];
            for (column, value) in columns.iter().enumerate() {
                typed[column] = command_trace_value(value).ok_or_else(|| {
                    Error::Invalid(format!(
                        "command trace {path:?} frame {frame} column {column} is invalid"
                    ))
                })?;
            }
            linked.push(typed);
        }
        Ok(Arc::from(linked.into_boxed_slice()))
    }

    /// Compile the compact native animation-event sidecar once at resource
    /// registration. Missing entries are an all-zero suffix; the exporter
    /// intentionally trims only trailing zeros, so indices remain frame
    /// aligned and the hot path never parses JSON.
    fn compile_animation_event_masks(
        &self,
        path: &str,
        attack_path: Option<&str>,
    ) -> Result<Option<AnimationEventMaskRows>, Error> {
        let Some(value) = self.value_path(&format!("{path}.animation_event_masks")) else {
            return Ok(None);
        };
        let Some(rows) = value.as_array() else {
            return Err(Error::Invalid(format!(
                "animation event masks {path:?} must be an array"
            )));
        };
        let Some(attack_path) = attack_path else {
            return Err(Error::Invalid(format!(
                "animation event masks {path:?} require an animation resource"
            )));
        };
        let expected = self.frame_count(attack_path);
        if expected == 0 {
            return Err(Error::Invalid(format!(
                "animation event masks {path:?} reference missing pose resource {attack_path:?}"
            )));
        }
        if rows.len() > expected || rows.len() > 4_096 {
            return Err(Error::Invalid(format!(
                "animation event masks {path:?} has {} rows, expected at most {expected}",
                rows.len()
            )));
        }
        let mut linked = Vec::with_capacity(rows.len());
        for (frame, value) in rows.iter().enumerate() {
            let mask = value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| {
                    Error::Invalid(format!(
                        "animation event masks {path:?} frame {frame} must be a u8"
                    ))
                })?;
            linked.push(mask);
        }
        Ok(Some(Arc::from(linked.into_boxed_slice())))
    }

    pub(crate) fn attack_for_owner(
        &self,
        owner: Option<usize>,
        action: Action,
    ) -> Option<AttackId> {
        let key = (owner.unwrap_or(usize::MAX), action);
        self.action_attacks_by_owner.get(&key).copied()
    }

    fn link_attack_resources(&mut self, data: Option<&FighterData>) -> Result<(), Error> {
        let Some(data) = data else {
            return Ok(());
        };
        let Some(program) = self.program.as_ref() else {
            return Ok(());
        };
        let Some(specials) = data.specials.as_ref() else {
            return Ok(());
        };
        self.action_attacks_by_owner =
            Arc::new(Self::attack_links(program.metadata(), &specials.resources));
        Ok(())
    }

    fn attack_links(
        definition: &super::definition::FighterDefinition,
        resources: &crate::game::script::resources::Resources,
    ) -> OwnedActionMap<AttackId> {
        let mut links = BTreeMap::new();
        let canonical = |definition: &super::definition::ActionDefinition| {
            definition.action.as_deref().and_then(|reference| {
                super::parse_action(reference.strip_prefix("Action.").unwrap_or(reference))
            })
        };
        for action_definition in definition.actions.values() {
            if let (Some(action), Some(path)) = (
                canonical(action_definition),
                action_definition.attack.as_deref(),
            ) && let Some(id) = resources.attack_id(path)
            {
                links.insert((usize::MAX, action), id);
            }
        }
        for (owner, behavior) in definition.behaviors.iter().enumerate() {
            for action_definition in behavior.actions.values() {
                if let (Some(action), Some(path)) = (
                    canonical(action_definition),
                    action_definition.attack.as_deref(),
                ) && let Some(id) = resources.attack_id(path)
                {
                    links.insert((owner, action), id);
                }
            }
        }
        links
    }

    fn link_motion_profiles(
        &mut self,
        data: Option<&FighterData>,
        rules: Option<&Rules>,
    ) -> Result<(), Error> {
        let Some(data) = data else {
            return Ok(());
        };
        let Some(program) = self.program.as_ref() else {
            return Ok(());
        };
        let movement = serde_json::to_value(&data.movement)
            .map_err(|error| Error::Invalid(format!("invalid motion parameters: {error}")))?;
        let rules = rules
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| Error::Invalid(format!("invalid motion rules: {error}")))?
            .unwrap_or(serde_json::Value::Null);
        let parameter_values = serde_json::json!({
            "movement": movement.clone(),
            "fighter": movement,
            "rules": rules,
        });
        let parameters = LinkedMotionParameters {
            values: &parameter_values,
            resources: self,
        };
        let mut profiles = Vec::new();
        let mut profile_ids = BTreeMap::new();
        let mut owner_profile_ids: OwnedActionMap<MotionProfileId> = BTreeMap::new();
        let mut delay_fields = BTreeMap::new();
        let mut owner_delay_fields: OwnedActionMap<Option<String>> = BTreeMap::new();
        let mut animation_loops = BTreeMap::new();
        let mut owner_animation_loops: OwnedActionMap<bool> = BTreeMap::new();
        for (name, action) in &program.metadata().actions {
            let key = action
                .action
                .as_deref()
                .map(|reference| reference.strip_prefix("Action.").unwrap_or(reference))
                .unwrap_or(name)
                .to_owned();
            animation_loops.insert(key, action.animation_loop);
        }
        for (behavior_index, behavior) in program.metadata().behaviors.iter().enumerate() {
            for (name, action) in &behavior.actions {
                let key = action
                    .action
                    .as_deref()
                    .map(|reference| reference.strip_prefix("Action.").unwrap_or(reference))
                    .unwrap_or(name)
                    .to_owned();
                animation_loops.insert(key, action.animation_loop);
                if let Some(canonical) = action.action.as_deref().and_then(|reference| {
                    super::parse_action(reference.strip_prefix("Action.").unwrap_or(reference))
                }) {
                    owner_animation_loops
                        .insert((behavior_index, canonical), action.animation_loop);
                }
            }
        }
        let disabled_actions: std::collections::BTreeSet<String> = program
            .metadata()
            .behaviors
            .iter()
            .filter(|behavior| {
                behavior
                    .resource
                    .as_deref()
                    .is_some_and(|path| self.value_path(path).is_none())
            })
            .flat_map(|behavior| behavior.actions.values())
            .filter_map(|action| action.action.as_deref())
            .map(|action| {
                action
                    .strip_prefix("Action.")
                    .unwrap_or(action)
                    .to_ascii_lowercase()
            })
            .collect();
        let behavior_action_enabled = |name: &str| {
            program
                .metadata()
                .behaviors
                .iter()
                .find(|behavior| {
                    behavior.actions.contains_key(name)
                        || name.split_once('.').is_some_and(|(owner, local)| {
                            behavior.actions.contains_key(local)
                                || behavior.id.as_deref() == Some(owner)
                        })
                })
                .is_none_or(|behavior| {
                    behavior
                        .resource
                        .as_deref()
                        .is_none_or(|path| self.value_path(path).is_some())
                })
        };
        // Root action metadata is an owning action table too. Keep it in the
        // same immutable index as behavior-owned actions so a root callback
        // cannot accidentally fall back to a VM movement hook.
        for (name, action) in &program.metadata().actions {
            if !behavior_action_enabled(name)
                || action.action.as_deref().is_some_and(|action| {
                    disabled_actions.contains(
                        action
                            .strip_prefix("Action.")
                            .unwrap_or(action)
                            .to_ascii_lowercase()
                            .as_str(),
                    )
                })
            {
                continue;
            }
            let Some(compiled) = action.motion.as_ref() else {
                continue;
            };
            let descriptor = MotionDescriptor::from_compiled_constructor(compiled)
                .map_err(|error| Error::Invalid(format!("action {name:?} motion: {error}")))?;
            let profile = link_profile(&descriptor, self, &parameters)
                .map_err(|error| Error::Invalid(format!("action {name:?} motion: {error}")))?;
            let id = MotionProfileId::new(
                u16::try_from(profiles.len())
                    .map_err(|_| Error::Invalid("too many motion profiles".into()))?,
            );
            let action_key = action
                .action
                .as_deref()
                .map(|reference| reference.strip_prefix("Action.").unwrap_or(reference))
                .unwrap_or(name)
                .to_owned();
            profile_ids.insert(action_key.clone(), id);
            let delay = action.profile_delay_field.clone();
            if let Some(field) = &delay {
                let declared = program.metadata().action_state.fields.get(field);
                if !declared.is_some_and(|field| field.value_type == super::StateType::Number) {
                    return Err(Error::Invalid(format!(
                        "action {name:?} profile_delay_field {field:?} must name a declared f32 state field"
                    )));
                }
            }
            delay_fields.insert(action_key.clone(), delay);
            profiles.push(profile);
        }
        for (behavior_index, behavior) in program.metadata().behaviors.iter().enumerate() {
            if behavior
                .resource
                .as_deref()
                .is_some_and(|path| self.value_path(path).is_none())
            {
                continue;
            }
            for (name, action) in &behavior.actions {
                let Some(compiled) = action.motion.as_ref() else {
                    continue;
                };
                let descriptor = MotionDescriptor::from_compiled_constructor(compiled)
                    .map_err(|error| Error::Invalid(format!("action {name:?} motion: {error}")))?;
                let profile = link_profile(&descriptor, self, &parameters)
                    .map_err(|error| Error::Invalid(format!("action {name:?} motion: {error}")))?;
                let id = MotionProfileId::new(
                    u16::try_from(profiles.len())
                        .map_err(|_| Error::Invalid("too many motion profiles".into()))?,
                );
                let action_key = action
                    .action
                    .as_deref()
                    .map(|reference| reference.strip_prefix("Action.").unwrap_or(reference))
                    .unwrap_or(name)
                    .to_owned();
                profile_ids.insert(action_key.clone(), id);
                if let Some(canonical) = super::parse_action(&action_key) {
                    owner_profile_ids.insert((behavior_index, canonical), id);
                }
                let delay = action.profile_delay_field.clone();
                if let Some(field) = &delay {
                    let declared = program.metadata().action_state.fields.get(field);
                    if !declared.is_some_and(|field| field.value_type == super::StateType::Number) {
                        return Err(Error::Invalid(format!(
                            "action {name:?} profile_delay_field {field:?} must name a declared f32 state field"
                        )));
                    }
                }
                delay_fields.insert(action_key.clone(), delay);
                if let Some(canonical) = super::parse_action(&action_key) {
                    owner_delay_fields.insert(
                        (behavior_index, canonical),
                        action.profile_delay_field.clone(),
                    );
                }
                profiles.push(profile);
            }
        }
        self.profiles = Arc::new(profiles);
        let mut action_profiles = vec![None; Action::BUILTIN_COUNT];
        let mut action_profiles_custom = BTreeMap::new();
        for (name, id) in &profile_ids {
            if let Some(action) = super::parse_action(name) {
                if let Some(index) = action.builtin_index() {
                    action_profiles[index] = Some(*id);
                } else {
                    action_profiles_custom.insert(action, *id);
                }
            }
        }
        self.action_profiles = Arc::new(action_profiles);
        self.action_profiles_custom = Arc::new(action_profiles_custom);
        self.action_profiles_by_owner = Arc::new(owner_profile_ids);
        let mut action_delay_fields = vec![None; Action::BUILTIN_COUNT];
        let mut action_delay_fields_custom = BTreeMap::new();
        for (name, delay) in delay_fields.iter() {
            if let Some(action) = super::parse_action(name) {
                if let Some(index) = action.builtin_index() {
                    action_delay_fields[index] = delay.clone();
                } else {
                    action_delay_fields_custom.insert(action, delay.clone());
                }
            }
        }
        self.action_delay_fields = Arc::new(action_delay_fields);
        self.action_delay_fields_custom = Arc::new(action_delay_fields_custom);
        self.action_delay_fields_by_owner = Arc::new(owner_delay_fields);
        let mut action_animation_loops = vec![false; Action::BUILTIN_COUNT];
        let mut action_animation_loops_custom = BTreeMap::new();
        for (name, looping) in animation_loops {
            if let Some(action) = super::parse_action(&name) {
                if let Some(index) = action.builtin_index() {
                    action_animation_loops[index] = looping;
                } else {
                    action_animation_loops_custom.insert(action, looping);
                }
            }
        }
        self.action_animation_loops = Arc::new(action_animation_loops);
        self.action_animation_loops_custom = Arc::new(action_animation_loops_custom);
        self.action_animation_loops_by_owner = Arc::new(owner_animation_loops);
        Ok(())
    }

    fn compile_action_events(&self) -> Result<ActionEventTable, Error> {
        let Some(program) = self.program.as_ref() else {
            return ActionEventTable::compile(vec![], vec![], vec![])
                .map_err(|error| Error::Invalid(error.to_string()));
        };
        let mut markers = Vec::new();
        let mut countdowns = Vec::new();
        let mut clocks = Vec::new();
        let mut frames = Vec::new();
        let mut token = 0_u64;
        let mut append = |binding: &super::definition::EventBinding| -> Result<(), Error> {
            token = token
                .checked_add(1)
                .ok_or_else(|| Error::Invalid("too many action-event bindings".into()))?;
            let action_names = if binding.actions.is_empty() {
                binding
                    .action
                    .as_deref()
                    .map(|action| vec![action.to_owned()])
                    .unwrap_or_default()
            } else {
                binding.actions.clone()
            };
            let actions = action_names
                .iter()
                .map(|name| {
                    resolve_action_name(program, name).ok_or_else(|| {
                        Error::Invalid(format!("unknown action event owner {name:?}"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            if let Some(name) = &binding.marker {
                let track = binding.track.as_deref().ok_or_else(|| {
                    Error::Invalid(format!("marker {name:?} is missing its track"))
                })?;
                let flags = self
                    .value_path(track)
                    .and_then(JsonValue::as_array)
                    .ok_or_else(|| {
                        Error::Invalid(format!("marker track {track:?} must be an array"))
                    })?
                    .iter()
                    .map(|value| {
                        value.as_bool().ok_or_else(|| {
                            Error::Invalid(format!("marker track {track:?} must contain booleans"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                markers.push(MarkerSpec {
                    id: MarkerId::new(
                        u16::try_from(markers.len())
                            .map_err(|_| Error::Invalid("too many marker bindings".into()))?,
                    ),
                    name: name.clone(),
                    track: track.to_owned(),
                    actions: actions.clone(),
                    flags,
                    token,
                });
            }
            if let Some(field) = &binding.countdown {
                let declared = program.metadata().action_state.fields.get(field);
                if !declared.is_some_and(|field| field.value_type == super::StateType::Number) {
                    return Err(Error::Invalid(format!(
                        "countdown field {field:?} must name a declared f32 action-state field"
                    )));
                }
                countdowns.push(CountdownSpec {
                    field: field.clone(),
                    actions,
                    token,
                    phase: binding.countdown_phase,
                });
            }
            if let Some(frame) = binding.deadline {
                let action = action_names.first().ok_or_else(|| {
                    Error::Invalid("action-frame deadline is missing an action owner".into())
                })?;
                let action = resolve_action_name(program, action).ok_or_else(|| {
                    Error::Invalid(format!("unknown action-frame owner {action:?}"))
                })?;
                frames.push(FrameSpec {
                    action,
                    frame,
                    token,
                });
            }
            Ok(())
        };
        for binding in &program.metadata().callbacks {
            append(binding)?;
        }
        for behavior in &program.metadata().behaviors {
            if behavior
                .resource
                .as_deref()
                .is_some_and(|path| self.value_path(path).is_none())
            {
                continue;
            }
            for binding in &behavior.callbacks {
                append(binding)?;
            }
            for clock in &behavior.clocks {
                let declared = program.metadata().action_state.fields.get(&clock.field);
                if !declared.is_some_and(|field| field.value_type == super::StateType::Number) {
                    return Err(Error::Invalid(format!(
                        "clock field {:?} must name a declared f32 action-state field",
                        clock.field
                    )));
                }
                clocks.push(ClockSpec {
                    field: clock.field.clone(),
                    actions: clock
                        .actions
                        .iter()
                        .map(|name| {
                            resolve_action_name(program, name).ok_or_else(|| {
                                Error::Invalid(format!("unknown clock action owner {name:?}"))
                            })
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                });
            }
        }
        ActionEventTable::compile_with_frames(markers, countdowns, clocks, frames)
            .map_err(|error| Error::Invalid(error.to_string()))
    }

    /// Build the registration-time compiler environment from the immutable
    /// JSON projection. Every object/array is retained as a value handle and
    /// every scalar leaf receives its exact scalar type; no source callback
    /// can add fields or perform a dynamic schema lookup later.
    pub(crate) fn environment(&self) -> Result<super::starlark::Environment, Error> {
        let mut environment = super::starlark::Environment::default();
        for (name, value) in self.values.iter() {
            register_json_schema(&mut environment, &format!("context.{name}"), value)?;
        }
        Ok(environment)
    }

    /// Replace the prepared program with the verified link for this
    /// registration. Linking is intentionally performed before the cache is
    /// published, so all later lifecycle calls share this immutable Arc.
    pub(crate) fn link_program(
        &mut self,
        environment: &super::starlark::Environment,
    ) -> Result<(), Error> {
        let Some(program) = self.program.take() else {
            return Ok(());
        };
        let linked = program.linked_with_environment(environment)?;
        self.program = Some(Arc::new(linked));
        Ok(())
    }

    pub(crate) fn value(&self, path: &str) -> Option<&JsonValue> {
        checked_path(path)
            .ok()
            .and_then(|path| self.values.get(path))
    }

    /// Resolve a projected resource path without materializing its container.
    /// The VM appends sequence indices and fields to a host object path (for
    /// example, `attacks[0].hitboxes[1].damage`), so this parser must preserve
    /// the lazy cache contract across both operations.
    pub(crate) fn value_path(&self, path: &str) -> Option<&JsonValue> {
        if path.is_empty() || path.len() > MAX_PATH_BYTES {
            return None;
        }
        if !path.contains('[') {
            checked_path(path).ok()?;
            if let Some(value) = self.value(path) {
                return Some(value);
            }
            let parts: Vec<_> = path.split('.').collect();
            for split in (1..parts.len()).rev() {
                let root = parts[..split].join(".");
                let suffix = parts[split..].join(".");
                if let Some(root_value) = self.value(&root) {
                    return nested_value(root_value, &suffix);
                }
            }
            return None;
        }
        let bracket = path.find('[')?;
        let root = &path[..bracket];
        // The array root may itself be a nested object path (for example
        // `neutral.thresholds[1]`). Resolve that path through the same lazy
        // traversal used for dotted fields before consuming indices.
        let mut value = self.value_path(root)?;
        let mut rest = &path[bracket..];
        while !rest.is_empty() {
            if let Some(after_open) = rest.strip_prefix('[') {
                let close = after_open.find(']')?;
                let index = after_open[..close].parse::<usize>().ok()?;
                value = value.as_array()?.get(index)?;
                rest = &after_open[close + 1..];
                continue;
            }
            let fields = rest.strip_prefix('.')?;
            let end = fields.find(['.', '[']).unwrap_or(fields.len());
            let field = &fields[..end];
            if field.is_empty()
                || !field
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
            {
                return None;
            }
            value = value.as_object()?.get(field)?;
            rest = &fields[end..];
        }
        Some(value)
    }

    pub(crate) fn array_length_path(&self, path: &str) -> Option<usize> {
        self.value_path(path)?.as_array().map(Vec::len)
    }

    pub(crate) fn frame_count(&self, path: &str) -> usize {
        let Ok(path) = checked_path(path) else {
            return 0;
        };
        self.attacks.get(path).map_or_else(
            || self.array_counts.get(path).copied().unwrap_or(0),
            |meta| meta.samples.len(),
        )
    }

    pub(crate) fn array_length(&self, path: &str, index: Option<usize>) -> Option<usize> {
        let value = match index {
            Some(index) => self.value_path(&format!("{path}[{index}]"))?,
            None => self.value_path(path)?,
        };
        value.as_array().map(Vec::len)
    }

    pub(crate) fn sample(&self, path: &str, frame: usize) -> Option<&JsonValue> {
        let path = checked_path(path).ok()?;
        if let Some(meta) = self.attacks.get(path) {
            return meta.samples.get(frame);
        }
        self.values.get(path)?.as_array()?.get(frame)
    }

    pub(crate) fn validate_attack(&self, path: &str) -> Result<bool, Error> {
        let path = checked_path(path).map_err(|error| Error::Invalid(error.to_owned()))?;
        match self.attacks.get(path) {
            Some(AttackMeta {
                validation: Ok(()), ..
            }) => Ok(true),
            Some(AttackMeta {
                validation: Err(error),
                ..
            }) => Err(Error::Invalid(error.clone())),
            None => Ok(false),
        }
    }
}

/// Resolve an action owner at registration time. Decorators refer to the
/// local action declaration (`start`, `down`, ...), while the native table
/// must retain the canonical engine action. This is link-time metadata work;
/// dispatch never repeats this lookup or parses an action name.
fn resolve_action_name(program: &super::Program, name: &str) -> Option<crate::game::Action> {
    let canonical = name.strip_prefix("Action.").unwrap_or(name);
    if let Some(action) = super::parse_action(canonical) {
        return Some(action);
    }
    let definition_action = |definition: &super::definition::ActionDefinition| {
        definition.action.as_deref().and_then(|reference| {
            super::parse_action(reference.strip_prefix("Action.").unwrap_or(reference))
        })
    };
    let metadata = program.metadata();
    metadata
        .actions
        .get(name)
        .and_then(definition_action)
        .or_else(|| {
            metadata
                .behaviors
                .iter()
                .find_map(|behavior| behavior.actions.get(name).and_then(definition_action))
        })
}

fn canonical_action(
    name: &str,
    action: &super::definition::ActionDefinition,
) -> Option<crate::game::Action> {
    action
        .action
        .as_deref()
        .and_then(|reference| {
            super::parse_action(reference.strip_prefix("Action.").unwrap_or(reference))
        })
        .or_else(|| super::parse_action(name.strip_prefix("Action.").unwrap_or(name)))
}

/// Select the same descriptor the definition index uses for each canonical
/// action before inspecting optional fields. In particular, a canonical map
/// key with `command_trace: None` must suppress an alias carrying a trace.
fn selected_action_definitions(
    actions: &BTreeMap<String, super::definition::ActionDefinition>,
) -> ActionMap<&super::definition::ActionDefinition> {
    let mut selected = BTreeMap::new();
    for (name, action) in actions {
        let Some(canonical) = canonical_action(name, action) else {
            continue;
        };
        if !selected.contains_key(&canonical) || is_canonical_name(name, canonical) {
            selected.insert(canonical, action);
        }
    }
    selected
}

fn is_canonical_name(name: &str, action: crate::game::Action) -> bool {
    name.strip_prefix("Action.").unwrap_or(name) == format!("{action:?}")
}

fn command_trace_value(value: &JsonValue) -> Option<Option<u32>> {
    if value.is_null() {
        return Some(None);
    }
    if let Some(integer) = value.as_u64() {
        return Some(Some(u32::try_from(integer).ok()?));
    }
    let number = value.as_f64()?;
    (number.is_finite() && number >= 0.0 && number <= f64::from(u32::MAX) && number.fract() == 0.0)
        .then_some(Some(number as u32))
}

fn nested_value<'a>(value: &'a JsonValue, path: &str) -> Option<&'a JsonValue> {
    path.split('.')
        .filter(|part| !part.is_empty())
        .try_fold(value, |value, part| value.as_object()?.get(part))
}

fn register_json_schema(
    environment: &mut super::starlark::Environment,
    path: &str,
    value: &JsonValue,
) -> Result<(), Error> {
    use super::starlark::{
        ValueType,
        environment::{HostField, HostType},
    };

    // Command traces are linked into the native fixed-width representation;
    // they do not need a closed VM sequence schema (nullable numeric rows
    // may mix JSON integer and float spellings). Keep the field addressable as
    // an opaque resource handle and skip its children.
    if path.rsplit('.').next() == Some("cmd_vars") {
        if environment.host_schema().field(path).is_none() {
            environment
                .register_host_field(
                    path.to_owned(),
                    HostField::read_only(HostType::Value(ValueType::Handle)),
                )
                .map_err(|error| Error::Invalid(format!("resource schema {path:?}: {error}")))?;
        }
        return Ok(());
    }

    let ty = json_host_type(value)
        .map_err(|error| Error::Invalid(format!("resource schema {path:?}: {error}")))?;
    // Generic host fields (for example context.input) are authoritative if a
    // resource happens to use the same top-level name.
    if environment.host_schema().field(path).is_none() {
        environment
            .register_host_field(path.to_owned(), HostField::read_only(ty))
            .map_err(|error| Error::Invalid(format!("resource schema {path:?}: {error}")))?;
    }
    if let JsonValue::Object(object) = value {
        for (name, child) in object {
            register_json_schema(environment, &format!("{path}.{name}"), child)?;
        }
    }
    Ok(())
}

/// Infer the closed VM type for a JSON resource value. Arrays are represented
/// as typed VM sequences, so indexing a resource cannot quietly turn into an
/// untyped `Value` read. Empty and all-null arrays retain an explicit handle
/// element type because their contents provide no narrower native hint.
fn json_host_type(value: &JsonValue) -> Result<super::starlark::environment::HostType, Error> {
    use super::starlark::ValueType;
    use super::starlark::environment::HostType;
    use super::starlark::value::NativeKind;

    match value {
        JsonValue::Bool(_) => Ok(HostType::bool()),
        JsonValue::Number(number) if number.is_i64() => Ok(HostType::i64()),
        JsonValue::Number(_) => Ok(HostType::f32()),
        JsonValue::String(_) => Ok(HostType::string()),
        JsonValue::Object(_) => Ok(HostType::Object(NativeKind::Value)),
        JsonValue::Null => Ok(HostType::Value(ValueType::Option(Box::new(
            ValueType::Handle,
        )))),
        JsonValue::Array(values) => {
            let mut element: Option<ValueType> = None;
            let mut nullable = false;
            for value in values {
                if value.is_null() {
                    nullable = true;
                    continue;
                }
                let candidate = json_host_type(value)?.as_value_type().ok_or_else(|| {
                    Error::Invalid("resource array element has no VM type".into())
                })?;
                if let Some(expected) = &element {
                    element =
                        Some(merge_resource_types(expected, &candidate).ok_or_else(|| {
                            Error::Invalid(
                                "resource arrays must contain one homogeneous element type".into(),
                            )
                        })?);
                } else {
                    element = Some(candidate);
                }
            }
            let element = element.unwrap_or(ValueType::Handle);
            let element = if nullable {
                ValueType::Option(Box::new(element))
            } else {
                element
            };
            Ok(HostType::Value(ValueType::Sequence(Box::new(element))))
        }
    }
}

/// Merge resource array element types while treating `Handle` as the open
/// type used for empty and all-null arrays. This lets command-trace rows with
/// no command values coexist with rows containing integer values.
fn merge_resource_types(
    left: &super::starlark::ValueType,
    right: &super::starlark::ValueType,
) -> Option<super::starlark::ValueType> {
    use super::starlark::ValueType;
    match (left, right) {
        (ValueType::Handle, other) | (other, ValueType::Handle) => Some(other.clone()),
        (ValueType::Option(left), ValueType::Option(right)) => Some(ValueType::Option(Box::new(
            merge_resource_types(left, right)?,
        ))),
        (ValueType::Sequence(left), ValueType::Sequence(right)) => Some(ValueType::Sequence(
            Box::new(merge_resource_types(left, right)?),
        )),
        (left, right) if left == right => Some(left.clone()),
        _ => None,
    }
}

fn checked_path(path: &str) -> Result<&str, &'static str> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.split('.').any(|part| {
            part.is_empty()
                || !part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
        })
    {
        return Err("invalid resource path");
    }
    Ok(path)
}

/// Validate every native attack cached from generic special resources. This
/// remains a public boundary used by match validation.
pub fn validate(data: &FighterData, rules: &Rules) -> Result<(), Error> {
    let cache = if let Some(cache) = data.script_resources.get() {
        cache
    } else {
        Arc::new(ResourceCache::build(Some(data), Some(rules), true)?)
    };
    for (path, attack) in cache.attacks.iter() {
        if let Err(error) = &attack.validation {
            return Err(Error::Invalid(format!("{path}: {error}")));
        }
    }
    validate_callbacks(data, Arc::clone(&cache), rules)?;
    Ok(())
}

/// Run each declared behavior validator at the resource boundary. These
/// callbacks receive context only and are invoked before match construction,
/// so malformed or unsupported resources cannot reach gameplay dispatch.
fn validate_callbacks(
    data: &FighterData,
    resources: Arc<ResourceCache>,
    rules: &Rules,
) -> Result<(), Error> {
    let program = resources
        .program()
        .or_else(|| crate::game::script::definition::cached_program(data));
    let Some(program) = program else {
        return Ok(());
    };
    let metadata = program.metadata();
    for (index, behavior) in metadata.behaviors.iter().enumerate() {
        let owned_resource = behavior.resource.clone();
        if owned_resource
            .as_deref()
            .is_some_and(|path| resources.value_path(path).is_none())
        {
            continue;
        }
        let Some(name) = behavior.validate.as_deref() else {
            continue;
        };
        let compiled = program.compiled().ok_or_else(|| {
            Error::Invalid(format!(
                "resource validator `{name}` for behavior {index} has no linked program"
            ))
        })?;
        // Authoring callbacks are published by the loader's callback table,
        // while `CompiledProgram::callback` only knows statically registered
        // native/bootstrap handles. Verify the logical name against the
        // exported table, then bind it to this program identity.
        let exported = compiled
            .callback_keys()
            .map_err(|error| Error::Invalid(format!("validator export table: {error}")))?;
        if !exported.iter().any(|key| key == name) {
            return Err(Error::Invalid(format!(
                "resource validator `{name}` for behavior {index} is not exported"
            )));
        }
        let callback = compiled.bind_callback(name);
        let host = Arc::new(Mutex::new(ValidationHost::new(
            Arc::clone(&resources),
            rules,
            owned_resource,
        )));
        let shared: native::SharedNativeHost = host.clone();
        let context = native::HostRef::context(shared);
        let result = compiled
            .dispatch(&callback, context, &[])
            .map_err(|error| Error::Invalid(format!("resource validator `{name}`: {error}")))?;
        if !matches!(result, native::NativeValue::Bool(true)) {
            return Err(Error::Invalid(format!(
                "resource validator `{name}` returned false"
            )));
        }
    }
    Ok(())
}

#[allow(dead_code)]
struct ValidationHost {
    resources: Arc<ResourceCache>,
    context: JsonValue,
    owned_resource: Option<String>,
}

#[allow(dead_code)]
impl ValidationHost {
    fn new(resources: Arc<ResourceCache>, rules: &Rules, owned_resource: Option<String>) -> Self {
        let mut context = Map::new();
        context.insert(
            "rules".into(),
            serde_json::to_value(rules).unwrap_or(JsonValue::Null),
        );
        Self {
            resources,
            context: JsonValue::Object(context),
            owned_resource,
        }
    }

    fn resource_value(&self, path: &str) -> Option<native::NativeValue> {
        let value = self.resources.value_path(path)?;
        match value {
            JsonValue::Object(_) | JsonValue::Array(_) => {
                Some(native::NativeValue::Object(native::NativeObject {
                    kind: native::NativeKind::Value,
                    path: format!("context.{path}"),
                }))
            }
            _ => json_to_native(value, &format!("context.{path}")).ok(),
        }
    }
}

impl native::NativeHost for ValidationHost {
    fn get(&mut self, path: &str) -> Result<native::NativeValue, native::Error> {
        if path == "context" {
            return Ok(native::NativeValue::Object(native::NativeObject {
                kind: native::NativeKind::Context,
                path: path.into(),
            }));
        }
        if let Some(resource) = path.strip_prefix("context.") {
            if let Some(move_id) = self.resources.projectile_move_id(resource) {
                return Ok(native::NativeValue::Int(i64::from(move_id)));
            }
            if let Some(resource) = resource.strip_suffix(".length")
                && let Some(length) = self.resources.array_length_path(resource)
            {
                return Ok(native::NativeValue::Int(length as i64));
            }
            if let Some(value) = self.resources.value_path(resource) {
                return match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        Ok(native::NativeValue::Object(native::NativeObject {
                            kind: native::NativeKind::Value,
                            path: path.into(),
                        }))
                    }
                    _ => json_to_native(value, path),
                };
            }
            let parts: Vec<_> = resource.split('.').collect();
            for split in (1..parts.len()).rev() {
                let root = parts[..split].join(".");
                let suffix = parts[split..].join(".");
                if let Some(root_value) = self.resources.value(&root)
                    && let Some(value) = super::lifecycle_host::json_path(root_value, &suffix)
                {
                    return json_to_native(value, path);
                }
            }
            let relative = resource.strip_prefix("rules.").unwrap_or(resource);
            if let Some(value) = super::lifecycle_host::json_path(&self.context, resource)
                .or_else(|| super::lifecycle_host::json_path(&self.context, relative))
            {
                return json_to_native(value, path);
            }
        }
        Err(native::Error::Host(format!(
            "unknown validation context path `{path}`"
        )))
    }

    fn set(&mut self, _path: &str, _value: native::NativeValue) -> Result<(), native::Error> {
        Err(native::Error::Host(
            "resource validation context is read-only".into(),
        ))
    }

    fn call_named(
        &mut self,
        path: &str,
        args: &[native::NativeValue],
        named: &BTreeMap<String, native::NativeValue>,
    ) -> Result<native::NativeValue, native::Error> {
        if let Some(value) = super::builtins::call_named(self, path, args, named)? {
            return Ok(value);
        }
        if named.is_empty() {
            self.call(path, args)
        } else {
            Err(native::Error::Host(format!(
                "native call `{path}` does not accept named arguments"
            )))
        }
    }

    fn call(
        &mut self,
        path: &str,
        args: &[native::NativeValue],
    ) -> Result<native::NativeValue, native::Error> {
        if let Some(value) = super::builtins::call(self, path, args)? {
            return Ok(value);
        }
        let string_arg = |index: usize, name: &str| {
            args.get(index)
                .and_then(|value| match value {
                    native::NativeValue::String(value) => Some(value.as_str()),
                    _ => None,
                })
                .ok_or_else(|| native::Error::Host(format!("{name} must be a string")))
        };
        if path == "context.resource" || path == "resource" {
            if args.is_empty() {
                let resource = self.owned_resource.as_deref().ok_or_else(|| {
                    native::Error::Host("no owned resource is available for this callback".into())
                })?;
                let value = self.resources.value_path(resource).ok_or_else(|| {
                    native::Error::Host(format!("owned resource `{resource}` is unavailable"))
                })?;
                return Ok(match value {
                    JsonValue::Object(_) | JsonValue::Array(_) => {
                        native::NativeValue::Object(native::NativeObject {
                            kind: native::NativeKind::Value,
                            path: format!("context.{resource}"),
                        })
                    }
                    _ => json_to_native(value, &format!("context.{resource}"))?,
                });
            }
            let path = string_arg(0, "resource path")?;
            return Ok(self
                .resource_value(path)
                .unwrap_or(native::NativeValue::None));
        }
        if path == "context.frames" || path == "frames" {
            return Ok(native::NativeValue::Int(
                self.resources.frame_count(string_arg(0, "resource path")?) as i64,
            ));
        }
        if path == "context.validate_attack" || path == "validate_attack" {
            return self
                .resources
                .validate_attack(string_arg(0, "attack path")?)
                .map(native::NativeValue::Bool)
                .map_err(|error| native::Error::Host(error.to_string()));
        }
        if path == "context.array_length" || path == "array_length" {
            let index = args.get(1).and_then(|value| match value {
                native::NativeValue::Int(value) => usize::try_from(*value).ok(),
                _ => None,
            });
            return Ok(self
                .resources
                .array_length(string_arg(0, "resource path")?, index)
                .map_or(native::NativeValue::None, |length| {
                    native::NativeValue::Int(length as i64)
                }));
        }
        Err(native::Error::Host(format!(
            "unknown validation method `{path}`"
        )))
    }
}

type ResourceViews = (BTreeMap<String, JsonValue>, BTreeMap<String, AttackMeta>);

fn link_projectile_resources(
    data: Option<&FighterData>,
) -> Result<BTreeMap<String, ProjectileMeta>, Error> {
    let Some(data) = data else {
        return Ok(BTreeMap::new());
    };
    let Some(specials) = data.specials.as_ref() else {
        return Ok(BTreeMap::new());
    };
    let mut linked = BTreeMap::new();
    for (key, value) in &specials.resources.values {
        link_projectile_value(key, value, &mut linked)?;
    }
    Ok(linked)
}

fn link_projectile_value(
    path: &str,
    value: &JsonValue,
    linked: &mut BTreeMap<String, ProjectileMeta>,
) -> Result<(), Error> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    // Only this complete, compact descriptor shape is a projectile resource.
    // Ordinary resources containing a field named `hitboxes` remain untouched.
    if object.contains_key("lifetime")
        && object.contains_key("hitboxes")
        && object.contains_key("move_id")
    {
        let lifetime = object["lifetime"].as_f64().ok_or_else(|| {
            Error::Invalid(format!(
                "projectile resource {path:?} lifetime must be a number"
            ))
        })?;
        if !lifetime.is_finite() || lifetime < 0.0 || lifetime > f64::from(f32::MAX) {
            return Err(Error::Invalid(format!(
                "projectile resource {path:?} lifetime is invalid"
            )));
        }
        let move_id = object["move_id"]
            .as_u64()
            .and_then(|id| u16::try_from(id).ok())
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "projectile resource {path:?} move_id must be a u16 integer"
                ))
            })?;
        let hitboxes: Vec<Hitbox> =
            serde_json::from_value(object["hitboxes"].clone()).map_err(|error| {
                Error::Invalid(format!(
                    "projectile resource {path:?} hitboxes are invalid: {error}"
                ))
            })?;
        if !(1..=4).contains(&hitboxes.len()) {
            return Err(Error::Invalid(format!(
                "projectile resource {path:?} must contain 1..=4 hitboxes"
            )));
        }
        for hitbox in &hitboxes {
            let finite = |value: f32| value.is_finite() && value.abs() <= 1_000_000.0;
            if !hitbox.center.iter().copied().all(finite)
                || !hitbox.radius.is_finite()
                || !(0.0..=1_000_000.0).contains(&hitbox.radius)
                || !hitbox.angle_degrees.is_finite()
                || !(0.0..=362.0).contains(&hitbox.angle_degrees)
                || hitbox.angle_degrees.fract() != 0.0
                || hitbox.group >= 16
                || hitbox.damage > 999
                || !(-1000..=1000).contains(&hitbox.shield_damage)
                || hitbox.growth > 1000
                || hitbox.fixed > 1000
                || hitbox.base > 1000
            {
                return Err(Error::Invalid(format!(
                    "projectile resource {path:?} contains an invalid hitbox"
                )));
            }
        }
        linked.insert(
            format!("{path}.hitboxes"),
            ProjectileMeta {
                lifetime: lifetime as f32,
                hitboxes: Arc::from(hitboxes.into_boxed_slice()),
                move_id,
            },
        );
        return Ok(());
    }
    for (key, child) in object {
        link_projectile_value(&format!("{path}.{key}"), child, linked)?;
    }
    Ok(())
}

fn link_article_resources(
    data: Option<&FighterData>,
    legacy: &BTreeMap<String, ProjectileMeta>,
) -> Result<BTreeMap<ArticleId, ArticleMeta>, Error> {
    let Some(data) = data else {
        return Ok(BTreeMap::new());
    };
    let Some(specials) = data.specials.as_ref() else {
        return Ok(BTreeMap::new());
    };
    if let Some(articles) = specials.articles.as_ref() {
        let mut linked = BTreeMap::new();
        for (id, resource) in articles {
            let path = format!("article {}", id.0);
            let mario_fireball = matches!(resource, ArticleResource::MarioFireball { .. });
            if mario_fireball && *id != ArticleId::MARIO_FIRE {
                return Err(Error::Invalid(format!(
                    "Mario fireball resource must use article id {}, got {}",
                    ArticleId::MARIO_FIRE.0,
                    id.0
                )));
            }
            let behavior = match resource {
                ArticleResource::Ray {
                    lifetime,
                    hitboxes,
                    move_id,
                } => {
                    let kind = match *id {
                        ArticleId::FOX_LASER => ProjectileKind::FoxLaser,
                        ArticleId::FALCO_LASER => ProjectileKind::FalcoLaser,
                        ArticleId(value) => {
                            return Err(Error::Invalid(format!(
                                "unsupported ray article id {value}"
                            )));
                        }
                    };
                    validate_projectile_fields(&path, *lifetime, hitboxes, *move_id)?;
                    ArticleBehavior::Ray {
                        kind,
                        lifetime: *lifetime,
                        hitboxes: Arc::from(hitboxes.clone().into_boxed_slice()),
                        move_id: *move_id,
                    }
                }
                ArticleResource::GravityProjectile {
                    speed,
                    angle,
                    lifetime,
                    half_life,
                    gravity,
                    terminal_velocity,
                    surface_multiplier,
                    terrain_stop_speed,
                    hitboxes,
                    move_id,
                    contact,
                }
                | ArticleResource::MarioFireball {
                    speed,
                    angle,
                    lifetime,
                    half_life,
                    gravity,
                    terminal_velocity,
                    surface_multiplier,
                    terrain_stop_speed,
                    hitboxes,
                    move_id,
                    contact,
                } => {
                    if let Some(name) = link_native_article(*id) {
                        return Err(Error::Invalid(format!(
                            "unsupported Link article {name} id {}",
                            id.0
                        )));
                    }
                    if let Some(name) = peach_native_article(*id) {
                        return Err(Error::Invalid(format!(
                            "unsupported Peach article {name} id {}",
                            id.0
                        )));
                    }
                    validate_gravity_projectile_fields(GravityProjectileFields {
                        path: &path,
                        speed: *speed,
                        angle: *angle,
                        lifetime: *lifetime,
                        half_life: *half_life,
                        gravity: *gravity,
                        terminal_velocity: *terminal_velocity,
                        surface_multiplier: *surface_multiplier,
                        terrain_stop_speed: *terrain_stop_speed,
                        hitboxes,
                        move_id: *move_id,
                    })?;
                    let behavior = if mario_fireball {
                        ArticleBehavior::MarioFireball {
                            speed: *speed,
                            angle: *angle,
                            lifetime: *lifetime,
                            half_life: *half_life,
                            gravity: *gravity,
                            terminal_velocity: *terminal_velocity,
                            surface_multiplier: *surface_multiplier,
                            terrain_stop_speed: *terrain_stop_speed,
                            hitboxes: Arc::from(hitboxes.clone().into_boxed_slice()),
                            move_id: *move_id,
                            contact: *contact,
                        }
                    } else {
                        ArticleBehavior::Gravity {
                            speed: *speed,
                            angle: *angle,
                            lifetime: *lifetime,
                            half_life: *half_life,
                            gravity: *gravity,
                            terminal_velocity: *terminal_velocity,
                            surface_multiplier: *surface_multiplier,
                            terrain_stop_speed: *terrain_stop_speed,
                            hitboxes: Arc::from(hitboxes.clone().into_boxed_slice()),
                            move_id: *move_id,
                            contact: *contact,
                        }
                    };
                    behavior
                }
                ArticleResource::LuigiFireball {
                    speed,
                    lifetime,
                    gravity,
                    terminal_velocity,
                    terrain_stop_speed,
                    effect_id,
                    hitboxes,
                    move_id,
                    contact,
                } => {
                    if *id != ArticleId::LUIGI_FIRE || *effect_id != 1288 {
                        return Err(Error::Invalid(
                            "Luigi fireball requires article 105 and native effect 1288".into(),
                        ));
                    }
                    if *contact != LuigiFireballContactPolicy::SourceLogic89 {
                        return Err(Error::Invalid(
                            "unsupported Luigi fireball contact policy".into(),
                        ));
                    }
                    validate_gravity_projectile_fields(GravityProjectileFields {
                        path: &path,
                        speed: *speed,
                        angle: 0.0,
                        lifetime: *lifetime,
                        half_life: *lifetime,
                        gravity: *gravity,
                        terminal_velocity: *terminal_velocity,
                        surface_multiplier: 1.0,
                        terrain_stop_speed: *terrain_stop_speed,
                        hitboxes,
                        move_id: *move_id,
                    })?;
                    ArticleBehavior::LuigiFireball {
                        speed: *speed,
                        angle: 0.0,
                        lifetime: *lifetime,
                        half_life: *lifetime,
                        gravity: *gravity,
                        terminal_velocity: *terminal_velocity,
                        surface_multiplier: 1.0,
                        terrain_stop_speed: *terrain_stop_speed,
                        hitboxes: Arc::from(hitboxes.clone().into_boxed_slice()),
                        move_id: *move_id,
                        contact: ProjectileContactPolicy {
                            reflection:
                                crate::game::script::resources::ProjectileReflection::ReverseOwner,
                            shield: crate::game::script::resources::ProjectileShield::Bounce,
                            persistence:
                                crate::game::script::resources::ProjectilePersistence::Despawn,
                        },
                    }
                }
            };
            linked.insert(*id, ArticleMeta { behavior });
        }
        return Ok(linked);
    }

    // v16 packs predate the numeric article catalog.  Convert only the two
    // native ray articles that already have a complete legacy descriptor;
    // this compatibility path is centralized here and never runs per frame.
    let id = if specials.character_key_is("fox") {
        ArticleId::FOX_LASER
    } else if specials.character_key_is("falco") {
        ArticleId::FALCO_LASER
    } else {
        return Ok(BTreeMap::new());
    };
    let Some(meta) = legacy.get("neutral.laser.hitboxes") else {
        return Ok(BTreeMap::new());
    };
    let kind = if id == ArticleId::FOX_LASER {
        ProjectileKind::FoxLaser
    } else {
        ProjectileKind::FalcoLaser
    };
    Ok(BTreeMap::from([(
        id,
        ArticleMeta {
            behavior: ArticleBehavior::Ray {
                kind,
                lifetime: meta.lifetime,
                hitboxes: Arc::clone(&meta.hitboxes),
                move_id: meta.move_id,
            },
        },
    )]))
}

/// Peach's native item callbacks own these articles' state machines.  They
/// are listed explicitly from `melee/it/forward.h` so a generic gravity
/// descriptor cannot silently turn a turnip, parasol, Toad, or Bomber effect
/// into a projectile with incorrect behavior.
fn peach_native_article(id: ArticleId) -> Option<&'static str> {
    match id.0 {
        98 => Some("Bomber explosion"),
        99 => Some("turnip"),
        103 => Some("parasol"),
        104 => Some("Toad"),
        111 => Some("Toad spore"),
        _ => None,
    }
}

/// Link and Young Link articles have stateful native item logic (attachment,
/// return/stick states, pickup, and bomb fuse transitions).  They must not be
/// accepted as generic gravity projectiles until that item substrate exists.
fn link_native_article(id: ArticleId) -> Option<&'static str> {
    match id.0 {
        58 => Some("bomb"),
        59 => Some("Young Link bomb"),
        60 => Some("boomerang"),
        61 => Some("Young Link boomerang"),
        62 => Some("hookshot"),
        63 => Some("Young Link hookshot"),
        64 => Some("arrow"),
        65 => Some("Young Link fire arrow"),
        _ => None,
    }
}

struct GravityProjectileFields<'a> {
    path: &'a str,
    speed: f32,
    angle: f32,
    lifetime: f32,
    half_life: f32,
    gravity: f32,
    terminal_velocity: f32,
    surface_multiplier: f32,
    terrain_stop_speed: f32,
    hitboxes: &'a [Hitbox],
    move_id: u16,
}

fn validate_gravity_projectile_fields(fields: GravityProjectileFields<'_>) -> Result<(), Error> {
    let GravityProjectileFields {
        path,
        speed,
        angle,
        lifetime,
        half_life,
        gravity,
        terminal_velocity,
        surface_multiplier,
        terrain_stop_speed,
        hitboxes,
        move_id,
    } = fields;
    let finite = |value: f32| value.is_finite() && value.abs() <= 1_000_000.0;
    if !finite(speed) || speed < 0.0 {
        return Err(Error::Invalid(format!("{path} speed is invalid")));
    }
    if !finite(angle) {
        return Err(Error::Invalid(format!("{path} angle is invalid")));
    }
    if !finite(half_life) || half_life < 0.0 {
        return Err(Error::Invalid(format!("{path} half-life is invalid")));
    }
    if !finite(gravity) || !finite(terminal_velocity) || terminal_velocity < 0.0 {
        return Err(Error::Invalid(format!("{path} gravity is invalid")));
    }
    if !finite(surface_multiplier) || surface_multiplier < 0.0 {
        return Err(Error::Invalid(format!(
            "{path} surface multiplier is invalid"
        )));
    }
    if !finite(terrain_stop_speed) || terrain_stop_speed < 0.0 {
        return Err(Error::Invalid(format!(
            "{path} terrain stop speed is invalid"
        )));
    }
    validate_projectile_fields(path, lifetime, hitboxes, move_id)
}

fn validate_projectile_fields(
    path: &str,
    lifetime: f32,
    hitboxes: &[Hitbox],
    move_id: u16,
) -> Result<(), Error> {
    if !lifetime.is_finite() || lifetime < 0.0 {
        return Err(Error::Invalid(format!("{path} lifetime is invalid")));
    }
    if !(1..=4).contains(&hitboxes.len()) {
        return Err(Error::Invalid(format!(
            "{path} must contain 1..=4 hitboxes"
        )));
    }
    for hitbox in hitboxes {
        let finite = |value: f32| value.is_finite() && value.abs() <= 1_000_000.0;
        if !hitbox.center.iter().copied().all(finite)
            || !hitbox.radius.is_finite()
            || !(0.0..=1_000_000.0).contains(&hitbox.radius)
            || !hitbox.angle_degrees.is_finite()
            || !(0.0..=362.0).contains(&hitbox.angle_degrees)
            || hitbox.angle_degrees.fract() != 0.0
            || hitbox.group >= 16
            || hitbox.damage > 999
            || !(-1000..=1000).contains(&hitbox.shield_damage)
            || hitbox.growth > 1000
            || hitbox.fixed > 1000
            || hitbox.base > 1000
        {
            return Err(Error::Invalid(format!("{path} contains an invalid hitbox")));
        }
    }
    let _ = move_id;
    Ok(())
}

fn resource_views(
    data: Option<&FighterData>,
    rules: Option<&Rules>,
    check_attacks: bool,
) -> Result<ResourceViews, Error> {
    let Some(data) = data else {
        return Ok((BTreeMap::new(), BTreeMap::new()));
    };
    let Some(specials) = data.specials.as_ref() else {
        return Ok((BTreeMap::new(), BTreeMap::new()));
    };
    let root = JsonValue::Object(specials.resources.values.clone().into_iter().collect());
    let mut values = BTreeMap::new();
    let mut attacks = BTreeMap::new();
    index_value(
        "",
        &root,
        specials,
        Some(data),
        rules,
        check_attacks,
        &mut values,
        &mut attacks,
    )?;
    Ok((values, attacks))
}

fn link_complete_animation_states(data: Option<&FighterData>) -> Arc<[u32]> {
    let Some(data) = data else {
        return Arc::from([]);
    };
    let Some(states) = data.motion_states.as_deref() else {
        return Arc::from([]);
    };
    let Some(specials) = data.specials.as_ref() else {
        return Arc::from([]);
    };
    let Some(animations) = specials.animations.as_ref() else {
        return Arc::from([]);
    };

    // A wrapper whose state table disagrees with the native motion table is
    // not safe to expose piecemeal. Match validation reports the detailed
    // error; the runtime gate simply fails closed.
    if specials.validate_animation_states(Some(states)).is_err()
        || states
            .windows(2)
            .any(|pair| pair[0].state_id >= pair[1].state_id)
    {
        return Arc::from([]);
    }

    let mut complete = Vec::new();
    for profile in states {
        let Ok(animation_id) = u32::try_from(profile.animation_id) else {
            continue;
        };
        let Some(resource) = animations.get(&animation_id) else {
            continue;
        };
        if resource.status != crate::game::script::resources::AnimationResourceStatus::Complete
            || resource.resource.is_none()
            || !specials
                .animation_attack(animation_id)
                .is_some_and(|attack| !attack.frames.is_empty())
        {
            continue;
        }
        complete.push(profile.state_id);
    }
    Arc::from(complete.into_boxed_slice())
}

#[allow(clippy::too_many_arguments)] // Indexing receives the independent resource stores it updates.
fn index_value(
    prefix: &str,
    value: &JsonValue,
    specials: &crate::game::script::resources::Specials,
    data: Option<&FighterData>,
    rules: Option<&Rules>,
    check_attacks: bool,
    values: &mut BTreeMap<String, JsonValue>,
    attacks: &mut BTreeMap<String, AttackMeta>,
) -> Result<(), Error> {
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    for (key, child) in object {
        if key == "character" && prefix.is_empty() {
            continue;
        }
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        let is_attack = specials.resources.attack(&path).is_some();
        values.insert(
            path.clone(),
            if key == "cmd_vars" {
                command_trace_view(child)
            } else if is_attack {
                sanitized_attack_view(child)
            } else {
                sanitized_view(child)
            },
        );
        if let Some(attack) = specials.resources.attack(&path) {
            let samples = attack
                .frames
                .iter()
                .enumerate()
                .map(|(index, frame)| {
                    let mut view = Map::new();
                    view.insert("frame".into(), JsonValue::from(index));
                    view.insert("bone_count".into(), JsonValue::from(frame.bones.len()));
                    view.insert("hitbox_count".into(), JsonValue::from(frame.hitboxes.len()));
                    view.insert(
                        "hurtbox_state_count".into(),
                        JsonValue::from(frame.hurtbox_states.len()),
                    );
                    JsonValue::Object(view)
                })
                .collect::<Vec<_>>();
            let validation = if !check_attacks {
                Ok(())
            } else {
                match (data, rules) {
                    (Some(fighter), Some(rules)) => {
                        if attack.frames.is_empty() {
                            Err("attack must supply 1..4096 complete physics frames".into())
                        } else {
                            crate::fighter::helpers::validate_hitboxes(attack, fighter, rules)
                                .map_err(|error| error.to_string())
                        }
                    }
                    _ => Err("attack validation requires fighter data and match rules".into()),
                }
            };
            attacks.insert(
                path.clone(),
                AttackMeta {
                    samples: samples.into(),
                    validation,
                },
            );
        }
        if !is_attack {
            index_value(
                &path,
                child,
                specials,
                data,
                rules,
                check_attacks,
                values,
                attacks,
            )?;
        }
    }
    Ok(())
}

fn sanitized_view(value: &JsonValue) -> JsonValue {
    if let Some(number) = value.as_number() {
        if number.is_i64() || number.is_u64() {
            return value.clone();
        }
        return number
            .as_f64()
            .and_then(|value| serde_json::Number::from_f64((value as f32) as f64))
            .map(JsonValue::Number)
            .unwrap_or(JsonValue::Null);
    }
    if let Some(array) = value.as_array() {
        return JsonValue::Array(array.iter().map(sanitized_view).collect());
    }
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    if object.contains_key("frames") {
        let mut view = Map::new();
        for (key, child) in object {
            if key != "frames" {
                view.insert(
                    key.clone(),
                    if key == "cmd_vars" {
                        command_trace_view(child)
                    } else {
                        sanitized_view(child)
                    },
                );
            }
        }
        let count = object
            .get("frames")
            .and_then(JsonValue::as_array)
            .map_or(0, Vec::len);
        view.remove("frames");
        view.insert("frame_count".into(), JsonValue::from(count));
        return JsonValue::Object(view);
    }
    JsonValue::Object(
        object
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    if key == "cmd_vars" {
                        command_trace_view(value)
                    } else {
                        sanitized_view(value)
                    },
                )
            })
            .collect(),
    )
}

fn command_trace_view(value: &JsonValue) -> JsonValue {
    if let Some(number) = value.as_number() {
        // Integer spellings are already exact and must remain so for the
        // full u32 range. Float spellings are retained as f64; cmd_vars is
        // registered as an opaque native-linked field below.
        return JsonValue::Number(number.clone());
    }
    if let Some(array) = value.as_array() {
        return JsonValue::Array(array.iter().map(command_trace_view).collect());
    }
    let Some(object) = value.as_object() else {
        return value.clone();
    };
    JsonValue::Object(
        object
            .iter()
            .map(|(key, value)| (key.clone(), command_trace_view(value)))
            .collect(),
    )
}

fn sanitized_attack_view(value: &JsonValue) -> JsonValue {
    let Some(object) = value.as_object() else {
        return JsonValue::Null;
    };
    let mut view = Map::new();
    for (key, child) in object {
        if key != "frames" {
            view.insert(key.clone(), sanitized_view(child));
        }
    }
    let count = object
        .get("frames")
        .and_then(JsonValue::as_array)
        .map_or(0, Vec::len);
    view.insert("frame_count".into(), JsonValue::from(count));
    JsonValue::Object(view)
}

#[cfg(test)]
mod tests {
    use super::{
        GravityProjectileFields, MotionParameterSource, ResourceCache, sanitized_view,
        validate_gravity_projectile_fields,
    };
    use crate::game::script::action_events::ActionEventTable;
    use crate::game::script::definition::{
        ActionDefinition, BehaviorDefinition, FighterDefinition,
    };
    use crate::game::script::resources::Resources;
    use serde_json::json;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn gravity_hitbox() -> crate::game::data::Hitbox {
        serde_json::from_value(json!({
            "group": 0,
            "bone": 0,
            "center": [0.0, 0.0, 0.0],
            "radius": 0.5,
            "damage": 3,
            "angle_degrees": 45.0,
            "growth": 20,
            "fixed": 0,
            "base": 10
        }))
        .unwrap()
    }

    fn metadata_cache(
        values: BTreeMap<String, serde_json::Value>,
        definition: FighterDefinition,
    ) -> ResourceCache {
        ResourceCache {
            special_attributes: None,
            values: Arc::new(values),
            attacks: Arc::default(),
            array_counts: Arc::default(),
            program: Some(Arc::new(crate::game::script::Program {
                source: Arc::from("metadata-only-resource-link-test"),
                dependency_sources: Arc::new(BTreeMap::new()),
                compiled: None,
                hook_indices: [None; crate::game::script::Hook::COUNT],
                callback_bindings: std::array::from_fn(|_| Vec::new()),
                fast_root_callbacks: [false; crate::game::script::Hook::COUNT],
                behavior_bindings: Vec::new(),
                metadata: Arc::new(definition),
                move_registry: Arc::new(
                    crate::game::script::move_registry::MoveRegistry::compile(
                        &FighterDefinition::default(),
                    )
                    .unwrap(),
                ),
            })),
            profiles: Arc::default(),
            action_profiles: Arc::default(),
            action_profiles_custom: Arc::default(),
            action_profiles_by_owner: Arc::default(),
            action_delay_fields: Arc::default(),
            action_delay_fields_custom: Arc::default(),
            action_delay_fields_by_owner: Arc::default(),
            action_animation_loops: Arc::default(),
            action_animation_loops_custom: Arc::default(),
            action_animation_loops_by_owner: Arc::default(),
            action_attacks_by_owner: Arc::default(),
            command_traces: Arc::default(),
            command_traces_by_owner: Arc::default(),
            animation_event_masks: Arc::default(),
            animation_event_masks_by_owner: Arc::default(),
            projectile_resources: Arc::default(),
            articles: Arc::default(),
            action_events: Arc::new(ActionEventTable::compile(vec![], vec![], vec![]).unwrap()),
            complete_animation_states: Arc::default(),
        }
    }

    #[test]
    fn gravity_article_validation_rejects_bad_physics_and_accepts_typed_values() {
        let hitboxes = [gravity_hitbox()];
        validate_gravity_projectile_fields(GravityProjectileFields {
            path: "article 48",
            speed: 1.5,
            angle: 0.0,
            lifetime: 60.0,
            half_life: 30.0,
            gravity: 0.08,
            terminal_velocity: 2.4,
            surface_multiplier: 0.5,
            terrain_stop_speed: 0.2,
            hitboxes: &hitboxes,
            move_id: 20,
        })
        .expect("valid gravity article");
        assert!(
            validate_gravity_projectile_fields(GravityProjectileFields {
                path: "article 48",
                speed: 1.5,
                angle: 0.0,
                lifetime: 60.0,
                half_life: -1.0,
                gravity: 0.08,
                terminal_velocity: 2.4,
                surface_multiplier: 0.5,
                terrain_stop_speed: 0.2,
                hitboxes: &hitboxes,
                move_id: 20,
            })
            .is_err()
        );
        assert!(
            validate_gravity_projectile_fields(GravityProjectileFields {
                path: "article 48",
                speed: 1.5,
                angle: 0.0,
                lifetime: 60.0,
                half_life: f32::NAN,
                gravity: 0.08,
                terminal_velocity: 2.4,
                surface_multiplier: 0.5,
                terrain_stop_speed: f32::NAN,
                hitboxes: &hitboxes,
                move_id: 20,
            })
            .is_err()
        );
    }

    #[test]
    fn special_attribute_projection_requires_native_layout_identity() {
        let mut cache = metadata_cache(BTreeMap::new(), FighterDefinition::default());
        cache.special_attributes = Some(
            serde_json::from_value(json!({
                "layout": 4,
                "words": [[0, 1065353216], [6, 1073741824]]
            }))
            .expect("typed Samus special attributes"),
        );

        // Samus's typed ids use layout 4; a field number from another
        // character must fail closed even when the field id exists.
        assert_eq!(cache.special_attribute(4, 0), Some(1.0));
        assert_eq!(cache.special_attribute(4, 6), Some(2.0));
        assert_eq!(cache.special_attribute(3, 6), None);
    }

    #[test]
    fn motion_number_projection_fails_closed_outside_f32_domain() {
        let cache = metadata_cache(
            BTreeMap::from([
                ("finite".into(), json!(1.25)),
                ("too_large".into(), json!(1.0e40)),
                ("too_small".into(), json!(-1.0e40)),
                ("max_f64".into(), json!(f64::MAX)),
                ("min_f64".into(), json!(-f64::MAX)),
            ]),
            FighterDefinition::default(),
        );
        assert_eq!(
            <ResourceCache as MotionParameterSource>::number_path(&cache, "finite"),
            Some(1.25)
        );
        assert_eq!(
            <ResourceCache as MotionParameterSource>::number_path(&cache, "too_large"),
            None
        );
        assert_eq!(
            <ResourceCache as MotionParameterSource>::number_path(&cache, "too_small"),
            None
        );
        assert_eq!(
            <ResourceCache as MotionParameterSource>::number_path(&cache, "max_f64"),
            None
        );
        assert_eq!(
            <ResourceCache as MotionParameterSource>::number_path(&cache, "min_f64"),
            None
        );
    }

    #[test]
    fn value_path_resolves_nested_objects_and_arrays_without_materializing_roots() {
        let mut values = BTreeMap::new();
        values.insert(
            "neutral".into(),
            json!({
                "thresholds": [0.25, 0.5],
                "laser": {"hitboxes": [{"damage": 3.0}]}
            }),
        );
        let cache = ResourceCache {
            special_attributes: None,
            values: Arc::new(values),
            attacks: Arc::default(),
            array_counts: Arc::default(),
            program: None,
            profiles: Arc::default(),
            action_profiles: Arc::default(),
            action_profiles_custom: Arc::default(),
            action_profiles_by_owner: Arc::default(),
            action_delay_fields: Arc::default(),
            action_delay_fields_custom: Arc::default(),
            action_delay_fields_by_owner: Arc::default(),
            action_animation_loops: Arc::default(),
            action_animation_loops_custom: Arc::default(),
            action_animation_loops_by_owner: Arc::default(),
            action_attacks_by_owner: Arc::default(),
            command_traces: Arc::default(),
            command_traces_by_owner: Arc::default(),
            animation_event_masks: Arc::default(),
            animation_event_masks_by_owner: Arc::default(),
            projectile_resources: Arc::default(),
            articles: Arc::default(),
            action_events: Arc::new(ActionEventTable::compile(vec![], vec![], vec![]).unwrap()),
            complete_animation_states: Arc::default(),
        };
        assert_eq!(cache.value_path("neutral.thresholds[1]"), Some(&json!(0.5)));
        assert_eq!(
            cache.value_path("neutral.laser.hitboxes[0].damage"),
            Some(&json!(3.0))
        );
        assert_eq!(cache.array_length_path("neutral.thresholds"), Some(2));
        assert_eq!(cache.array_length_path("neutral.laser.hitboxes"), Some(1));
    }

    #[test]
    fn fractional_resource_numbers_use_native_f32_precision() {
        let projected = sanitized_view(&json!(0.3));
        let value = projected.as_f64().expect("projected number");
        assert_eq!(value, 0.3_f32 as f64);
        assert_ne!(value, 0.3_f64);
    }

    #[test]
    fn article_cache_rejects_mario_fire_as_ray() {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let mut data: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        data.fighters[0].specials = Some(
            serde_json::from_value(json!({
                "character": "mario",
                "articles": {
                    "48": {
                        "kind": "ray",
                        "lifetime": 20.0,
                        "move_id": 48,
                        "hitboxes": [{
                            "group": 0,
                            "bone": 0,
                            "center": [0.0, 0.0, 0.0],
                            "radius": 0.5,
                            "damage": 3,
                            "angle_degrees": 0.0,
                            "growth": 0,
                            "fixed": 0,
                            "base": 0
                        }]
                    }
                }
            }))
            .unwrap(),
        );
        let error = ResourceCache::build(Some(&data.fighters[0]), None, false)
            .expect_err("Mario fire must not enter the ray fast path");
        assert!(error.to_string().contains("unsupported ray article id 48"));
    }

    #[test]
    fn article_cache_rejects_peach_native_item_as_generic_gravity() {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let mut data: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        data.fighters[0].specials = Some(
            serde_json::from_value(json!({
                "character": "peach",
                "articles": {
                    "99": {
                        "kind": "gravity_projectile",
                        "speed": 1.0,
                        "angle": 0.25,
                        "lifetime": 30.0,
                        "half_life": 10.0,
                        "gravity": 0.08,
                        "terminal_velocity": 2.4,
                        "surface_multiplier": 0.5,
                        "terrain_stop_speed": 0.2,
                        "move_id": 12,
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
            .unwrap(),
        );
        let error = ResourceCache::build(Some(&data.fighters[0]), None, false)
            .expect_err("Peach turnip must not enter the generic gravity path");
        assert!(
            error
                .to_string()
                .contains("unsupported Peach article turnip id 99")
        );
    }

    #[test]
    fn link_article_ids_are_reserved_for_stateful_native_items() {
        for id in 58..=65 {
            assert!(
                super::link_native_article(crate::game::script::resources::ArticleId(id)).is_some(),
                "Link article id {id} must not enter generic gravity handling"
            );
        }
        assert!(
            super::link_native_article(crate::game::script::resources::ArticleId(66)).is_none()
        );
    }

    fn animation_gate_cache(
        status: &str,
        wrapper_state: u32,
        state_animation_id: u32,
        wrapper_animation_id: u32,
    ) -> ResourceCache {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let base: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        let mut data = base.fighters[0].clone();
        data.motion_states = Some(vec![crate::game::data::MotionStateProfile {
            state_id: 381,
            animation_id: i32::try_from(state_animation_id).unwrap(),
            move_id: 20,
            flags: 0,
        }]);
        let resource = if status == "unsupported" {
            serde_json::Value::Null
        } else {
            json!({"frames": [{"bones": [], "hitboxes": []}]})
        };
        data.specials = Some(
            serde_json::from_value(json!({
                "character": "animation-gate",
                "animations": {
                    (wrapper_animation_id.to_string()): {
                        "animation_id": wrapper_animation_id,
                        "state_ids": [wrapper_state],
                        "status": status,
                        "resource": resource
                    }
                }
            }))
            .unwrap(),
        );
        ResourceCache::build(Some(&data), None, false).unwrap()
    }

    #[test]
    fn complete_animation_gate_accepts_only_consistent_executable_wrappers() {
        assert!(animation_gate_cache("complete", 381, 331, 331).has_complete_animation(381));
        assert!(!animation_gate_cache("complete", 382, 331, 331).has_complete_animation(381));
        assert!(!animation_gate_cache("pose_only", 381, 331, 331).has_complete_animation(381));
        assert!(!animation_gate_cache("unsupported", 381, 331, 331).has_complete_animation(381));
        assert!(!animation_gate_cache("complete", 381, 332, 331).has_complete_animation(381));
    }

    #[test]
    fn build_links_projectile_descriptor_with_typed_metadata() {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let base: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        let mut data = base.fighters[0].clone();
        data.specials = Some(crate::game::script::resources::Specials {
            character: "cache-projectile".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::new(BTreeMap::from([(
                "neutral".into(),
                json!({"laser": {
                    "lifetime": 18.0,
                    "move_id": 37,
                    "hitboxes": [{
                        "group": 0, "bone": 0, "center": [0.25, 0.0, 0.0],
                        "radius": 0.5, "damage": 3, "angle_degrees": 45.0,
                        "growth": 20, "fixed": 0, "base": 10
                    }]
                }}),
            )]))
            .unwrap(),
        });
        let cache = ResourceCache::build(Some(&data), None, false).unwrap();
        assert_eq!(cache.projectile_move_id("neutral.laser.move_id"), Some(37));
        assert_eq!(
            cache
                .projectile_hitboxes("neutral.laser.hitboxes")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            cache.projectile_hitboxes("neutral.laser.hitboxes").unwrap()[0].damage,
            3
        );
    }

    #[test]
    fn build_rejects_malformed_projectile_descriptor_atomically() {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let base: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        let mut data = base.fighters[0].clone();
        data.specials = Some(crate::game::script::resources::Specials {
            character: "cache-projectile".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::new(BTreeMap::from([(
                "neutral".into(),
                json!({"laser": {
                    "lifetime": 18.0,
                    "move_id": 37,
                    "hitboxes": []
                }}),
            )]))
            .unwrap(),
        });
        assert!(
            ResourceCache::build(Some(&data), None, false)
                .unwrap_err()
                .to_string()
                .contains("1..=4 hitboxes")
        );
    }

    #[test]
    fn command_trace_links_resource_object_and_integral_float_values() {
        let mut values = BTreeMap::new();
        values.insert(
            "neutral".into(),
            json!({"script": {"start": {"ground": {
                "cmd_vars": [[1.0, null, 4294967295.0, 0]]
            }}}}),
        );
        let cache = ResourceCache {
            special_attributes: None,
            values: Arc::new(values),
            attacks: Arc::default(),
            array_counts: Arc::default(),
            program: None,
            profiles: Arc::default(),
            action_profiles: Arc::default(),
            action_profiles_custom: Arc::default(),
            action_profiles_by_owner: Arc::default(),
            action_delay_fields: Arc::default(),
            action_delay_fields_custom: Arc::default(),
            action_delay_fields_by_owner: Arc::default(),
            action_animation_loops: Arc::default(),
            action_animation_loops_custom: Arc::default(),
            action_animation_loops_by_owner: Arc::default(),
            action_attacks_by_owner: Arc::default(),
            command_traces: Arc::default(),
            command_traces_by_owner: Arc::default(),
            animation_event_masks: Arc::default(),
            animation_event_masks_by_owner: Arc::default(),
            projectile_resources: Arc::default(),
            articles: Arc::default(),
            action_events: Arc::new(ActionEventTable::compile(vec![], vec![], vec![]).unwrap()),
            complete_animation_states: Arc::default(),
        };
        let rows = cache
            .compile_command_trace("neutral.script.start.ground")
            .unwrap();
        assert_eq!(rows[0], [Some(1), None, Some(u32::MAX), Some(0)]);
    }

    #[test]
    fn animation_event_masks_compile_with_zero_suffix_and_frame_bound() {
        let mut values = BTreeMap::new();
        values.insert("attack".into(), json!({"frames": [{}, {}, {}]}));
        values.insert("trace".into(), json!({"animation_event_masks": [0, 1]}));
        let mut cache = metadata_cache(values, FighterDefinition::default());
        cache.array_counts = Arc::new(BTreeMap::from([(String::from("attack"), 3)]));
        assert_eq!(
            cache
                .compile_animation_event_masks("trace", Some("attack"))
                .unwrap()
                .unwrap()
                .as_ref(),
            &[0, 1]
        );

        cache.values = Arc::new(BTreeMap::from([
            (String::from("attack"), json!({"frames": [{}, {}, {}]})),
            (
                String::from("trace"),
                json!({"animation_event_masks": [0, 1, 2, 3]}),
            ),
        ]));
        assert!(
            cache
                .compile_animation_event_masks("trace", Some("attack"))
                .is_err()
        );
    }

    #[test]
    fn animation_event_masks_require_typed_u8_rows_and_attack() {
        let mut cache = metadata_cache(
            BTreeMap::from([(
                String::from("trace"),
                json!({"animation_event_masks": [256]}),
            )]),
            FighterDefinition::default(),
        );
        cache.array_counts = Arc::new(BTreeMap::from([(String::from("attack"), 1)]));
        assert!(
            cache
                .compile_animation_event_masks("trace", Some("attack"))
                .is_err()
        );
        assert!(cache.compile_animation_event_masks("trace", None).is_err());
    }

    #[test]
    fn command_trace_rejects_invalid_values_columns_and_frame_counts() {
        let make = |rows: serde_json::Value| {
            let mut values = BTreeMap::new();
            values.insert("trace".into(), json!({"cmd_vars": rows}));
            ResourceCache {
                special_attributes: None,
                values: Arc::new(values),
                attacks: Arc::default(),
                array_counts: Arc::default(),
                program: None,
                profiles: Arc::default(),
                action_profiles: Arc::default(),
                action_profiles_custom: Arc::default(),
                action_profiles_by_owner: Arc::default(),
                action_delay_fields: Arc::default(),
                action_delay_fields_custom: Arc::default(),
                action_delay_fields_by_owner: Arc::default(),
                action_animation_loops: Arc::default(),
                action_animation_loops_custom: Arc::default(),
                action_animation_loops_by_owner: Arc::default(),
                action_attacks_by_owner: Arc::default(),
                command_traces: Arc::default(),
                command_traces_by_owner: Arc::default(),
                animation_event_masks: Arc::default(),
                animation_event_masks_by_owner: Arc::default(),
                projectile_resources: Arc::default(),
                articles: Arc::default(),
                action_events: Arc::new(ActionEventTable::compile(vec![], vec![], vec![]).unwrap()),
                complete_animation_states: Arc::default(),
            }
        };
        assert!(
            make(json!([[1.5, null, null, null]]))
                .compile_command_trace("trace")
                .is_err()
        );
        assert!(
            make(json!([[1, null, null]]))
                .compile_command_trace("trace")
                .is_err()
        );
        assert!(
            make(json!([[u64::from(u32::MAX) + 1, null, null, null]]))
                .compile_command_trace("trace")
                .is_err()
        );
        assert!(make(json!([])).compile_command_trace("trace").is_err());
    }

    #[test]
    fn command_trace_descriptor_selection_prefers_canonical_key_and_suppresses_alias() {
        let descriptor = |trace: Option<&str>| ActionDefinition {
            action: Some("SpecialNStart".into()),
            command_trace: trace.map(str::to_owned),
            ..ActionDefinition::default()
        };
        let definitions = BTreeMap::from([
            ("alias".into(), descriptor(Some("alias.trace"))),
            ("SpecialNStart".into(), descriptor(None)),
        ]);
        let selected = super::selected_action_definitions(&definitions);
        assert_eq!(selected.len(), 1);
        assert!(
            selected[&crate::game::Action::SpecialNStart]
                .command_trace
                .is_none()
        );

        let definitions = BTreeMap::from([(
            "SpecialNStart".into(),
            ActionDefinition {
                action: None,
                command_trace: Some("alias.trace".into()),
                ..ActionDefinition::default()
            },
        )]);
        let selected = super::selected_action_definitions(&definitions);
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[&crate::game::Action::SpecialNStart]
                .command_trace
                .as_deref(),
            Some("alias.trace")
        );
    }

    #[test]
    fn command_trace_build_links_actual_resources_and_keeps_owner_isolation() {
        let fixture = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let native_match: crate::game::MatchData = serde_json::from_str(fixture).unwrap();
        let base = crate::game::Match::new(native_match, 0).unwrap();
        let mut data = base.data().fighters[0].clone();
        let rows = json!([[4294967295.0, null, 1, null]]);
        let resources = Resources::new(BTreeMap::from([
            ("root".into(), json!({"cmd_vars": rows})),
            ("owner".into(), json!({"cmd_vars": [[2, null, null, null]]})),
            (
                "mask-only".into(),
                json!({
                    "frames": [
                        {"bones": [], "hitboxes": []},
                        {"bones": [], "hitboxes": []}
                    ]
                }),
            ),
        ]))
        .unwrap();
        data.specials = Some(crate::game::script::resources::Specials {
            character: "cache-test".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources,
        });
        let action = |trace: Option<&str>| ActionDefinition {
            action: Some("SpecialNStart".into()),
            command_trace: trace.map(str::to_owned),
            ..ActionDefinition::default()
        };
        let definition = FighterDefinition {
            actions: BTreeMap::from([
                (
                    "special.neutral.ground_start".into(),
                    ActionDefinition {
                        source_behavior: Some("move_0".into()),
                        ..action(Some("root"))
                    },
                ),
                (
                    "special.neutral.air_start".into(),
                    ActionDefinition {
                        action: Some("SpecialAirNStart".into()),
                        command_trace: Some("missing.flattened.trace".into()),
                        source_behavior: Some("move_2".into()),
                        ..ActionDefinition::default()
                    },
                ),
            ]),
            behaviors: vec![
                BehaviorDefinition {
                    id: Some("move_0".into()),
                    actions: BTreeMap::from([("owner".into(), action(Some("owner")))]),
                    ..BehaviorDefinition::default()
                },
                BehaviorDefinition {
                    id: Some("move_1".into()),
                    resource: Some("missing".into()),
                    actions: BTreeMap::from([("owner".into(), action(Some("missing.trace")))]),
                    ..BehaviorDefinition::default()
                },
                BehaviorDefinition {
                    id: Some("move_2".into()),
                    resource: Some("missing-air".into()),
                    actions: BTreeMap::from([(
                        "air_start".into(),
                        ActionDefinition {
                            action: Some("SpecialAirNStart".into()),
                            command_trace: Some("missing.flattened.trace".into()),
                            ..ActionDefinition::default()
                        },
                    )]),
                    ..BehaviorDefinition::default()
                },
            ],
            movesets: BTreeMap::from([(
                "special".into(),
                json!({"neutral": "move_0", "air": "move_2"}),
            )]),
            ..FighterDefinition::default()
        };
        data.script = Some(crate::game::script::Program {
            source: Arc::from("metadata-only-cache-fixture"),
            dependency_sources: Arc::new(BTreeMap::new()),
            compiled: None,
            hook_indices: [None; crate::game::script::Hook::COUNT],
            callback_bindings: std::array::from_fn(|_| Vec::new()),
            fast_root_callbacks: [false; crate::game::script::Hook::COUNT],
            behavior_bindings: Vec::new(),
            metadata: Arc::new(definition),
            move_registry: Arc::new(
                crate::game::script::move_registry::MoveRegistry::compile(
                    &FighterDefinition::default(),
                )
                .unwrap(),
            ),
        });
        let program = data.script.take().unwrap();
        let mut cache = ResourceCache::build(Some(&data), None, false).unwrap();
        cache.program = Some(Arc::new(program));
        cache.link_command_traces().unwrap();
        assert_eq!(
            cache
                .command_trace(None, crate::game::Action::SpecialNStart)
                .unwrap()[0][0],
            Some(u32::MAX)
        );
        assert_eq!(
            cache
                .command_trace(Some(0), crate::game::Action::SpecialNStart)
                .unwrap()[0][0],
            Some(2)
        );
        assert!(
            cache
                .command_trace(Some(1), crate::game::Action::SpecialNStart)
                .is_none()
        );
        assert!(
            cache
                .command_trace(None, crate::game::Action::SpecialAirNStart)
                .is_none()
        );
    }

    #[test]
    fn command_trace_linking_rejects_missing_trace_for_enabled_behavior() {
        let definition = FighterDefinition {
            behaviors: vec![BehaviorDefinition {
                resource: Some("enabled".into()),
                actions: BTreeMap::from([(
                    "owner".into(),
                    ActionDefinition {
                        action: Some("SpecialNStart".into()),
                        command_trace: Some("missing.trace".into()),
                        ..ActionDefinition::default()
                    },
                )]),
                ..BehaviorDefinition::default()
            }],
            ..FighterDefinition::default()
        };
        let mut values = BTreeMap::new();
        values.insert("enabled".into(), json!({"present": true}));
        let mut cache = ResourceCache {
            special_attributes: None,
            values: Arc::new(values),
            attacks: Arc::default(),
            array_counts: Arc::default(),
            program: Some(Arc::new(crate::game::script::Program {
                source: Arc::from("metadata-only-enabled-resource"),
                dependency_sources: Arc::new(BTreeMap::new()),
                compiled: None,
                hook_indices: [None; crate::game::script::Hook::COUNT],
                callback_bindings: std::array::from_fn(|_| Vec::new()),
                fast_root_callbacks: [false; crate::game::script::Hook::COUNT],
                behavior_bindings: Vec::new(),
                metadata: Arc::new(definition),
                move_registry: Arc::new(
                    crate::game::script::move_registry::MoveRegistry::compile(
                        &FighterDefinition::default(),
                    )
                    .unwrap(),
                ),
            })),
            profiles: Arc::default(),
            action_profiles: Arc::default(),
            action_profiles_custom: Arc::default(),
            action_profiles_by_owner: Arc::default(),
            action_delay_fields: Arc::default(),
            action_delay_fields_custom: Arc::default(),
            action_delay_fields_by_owner: Arc::default(),
            action_animation_loops: Arc::default(),
            action_animation_loops_custom: Arc::default(),
            action_animation_loops_by_owner: Arc::default(),
            action_attacks_by_owner: Arc::default(),
            command_traces: Arc::default(),
            command_traces_by_owner: Arc::default(),
            animation_event_masks: Arc::default(),
            animation_event_masks_by_owner: Arc::default(),
            projectile_resources: Arc::default(),
            articles: Arc::default(),
            action_events: Arc::new(ActionEventTable::compile(vec![], vec![], vec![]).unwrap()),
            complete_animation_states: Arc::default(),
        };
        let error = cache.link_command_traces().unwrap_err();
        assert!(error.to_string().contains("unknown command trace resource"));
    }

    #[test]
    fn untagged_dotted_root_sharing_disabled_behavior_action_remains_strict() {
        let action = ActionDefinition {
            action: Some("SpecialNStart".into()),
            command_trace: Some("missing.root".into()),
            ..Default::default()
        };
        let behavior = BehaviorDefinition {
            id: Some("move_0".into()),
            resource: Some("neutral".into()),
            actions: BTreeMap::from([("ground_start".into(), action.clone())]),
            ..Default::default()
        };
        let definition = FighterDefinition {
            actions: BTreeMap::from([("special.neutral.ground_start".into(), action)]),
            behaviors: vec![behavior],
            ..Default::default()
        };
        let error = metadata_cache(BTreeMap::new(), definition)
            .link_command_traces()
            .unwrap_err();
        assert!(error.to_string().contains("unknown command trace resource"));
    }

    #[test]
    fn unknown_root_source_behavior_is_rejected() {
        let action = ActionDefinition {
            action: Some("SpecialNStart".into()),
            command_trace: Some("trace".into()),
            source_behavior: Some("move_missing".into()),
            ..Default::default()
        };
        let definition = FighterDefinition {
            actions: BTreeMap::from([("root".into(), action)]),
            ..Default::default()
        };
        let error = metadata_cache(
            BTreeMap::from([(
                String::from("trace"),
                json!({"cmd_vars": [[1, null, null, null]]}),
            )]),
            definition,
        )
        .link_command_traces()
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("source behavior \"move_missing\" is unknown")
        );
    }

    #[test]
    fn missing_owner_special_does_not_fall_back_to_another_owner_resource() {
        let action = |attack: Option<&str>| ActionDefinition {
            action: Some("SpecialNStart".into()),
            attack: attack.map(str::to_owned),
            ..ActionDefinition::default()
        };
        let definition = FighterDefinition {
            behaviors: vec![
                BehaviorDefinition {
                    actions: BTreeMap::from([("start".into(), action(Some("first")))]),
                    ..BehaviorDefinition::default()
                },
                BehaviorDefinition {
                    actions: BTreeMap::from([("start".into(), action(None))]),
                    ..BehaviorDefinition::default()
                },
            ],
            ..FighterDefinition::default()
        };
        let resources = Resources::new(BTreeMap::from([
            ("first".into(), json!({"move_id": 71, "frames": []})),
            ("second".into(), json!({"move_id": 92, "frames": []})),
        ]))
        .unwrap();
        let links = ResourceCache::attack_links(&definition, &resources);
        assert!(links.contains_key(&(0, crate::game::Action::SpecialNStart)));
        assert!(!links.contains_key(&(1, crate::game::Action::SpecialNStart)));
        assert!(!links.contains_key(&(usize::MAX, crate::game::Action::SpecialNStart)));
    }

    #[test]
    fn attack_consumer_keeps_missing_special_owner_from_borrowing_other_owner() {
        let resource_json = include_str!("../../../../tests/fixtures/game/integration-match.json");
        let match_data: crate::game::data::MatchData =
            serde_json::from_str(resource_json).expect("integration match fixture");
        let native_match = crate::game::Match::new(match_data, 0).expect("native fixture");
        let mut fighter_data = native_match.data().fighters[0].clone();
        let resources = Resources::new(BTreeMap::from([(
            "first".into(),
            json!({"move_id": 71, "frames": []}),
        )]))
        .unwrap();
        fighter_data.specials = Some(crate::game::script::resources::Specials {
            character: "consumer-test".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: resources.clone(),
        });
        let definition = FighterDefinition {
            behaviors: vec![
                BehaviorDefinition {
                    actions: BTreeMap::from([(
                        String::from("start"),
                        ActionDefinition {
                            action: Some("SpecialNStart".into()),
                            attack: Some("first".into()),
                            ..ActionDefinition::default()
                        },
                    )]),
                    ..BehaviorDefinition::default()
                },
                BehaviorDefinition {
                    actions: BTreeMap::from([(
                        String::from("start"),
                        ActionDefinition {
                            action: Some("SpecialNStart".into()),
                            ..ActionDefinition::default()
                        },
                    )]),
                    ..BehaviorDefinition::default()
                },
            ],
            ..FighterDefinition::default()
        };
        let metadata = Arc::new(definition.clone());
        fighter_data.script = Some(crate::game::script::Program {
            source: Arc::from("metadata-only-consumer-fixture"),
            dependency_sources: Arc::new(BTreeMap::new()),
            compiled: None,
            hook_indices: [None; crate::game::script::Hook::COUNT],
            callback_bindings: std::array::from_fn(|_| Vec::new()),
            fast_root_callbacks: [false; crate::game::script::Hook::COUNT],
            behavior_bindings: Vec::new(),
            metadata,
            move_registry: Arc::new(
                crate::game::script::move_registry::MoveRegistry::compile(&definition)
                    .expect("consumer metadata registry"),
            ),
        });
        let mut cache = ResourceCache::build(None, None, false).unwrap();
        cache.program = fighter_data
            .script
            .as_ref()
            .map(|program| Arc::new(program.clone()));
        cache.action_attacks_by_owner =
            Arc::new(ResourceCache::attack_links(&definition, &resources));
        fighter_data.script_resources.replace(Arc::new(cache));
        let state_fighter = &native_match.state().fighters[0];
        assert!(
            fighter_data
                .attack_for_owner(state_fighter, crate::game::Action::SpecialNStart, Some(0))
                .is_some()
        );
        assert!(
            fighter_data
                .script_resources
                .get()
                .expect("resource cache")
                .attack_for_owner(Some(1), crate::game::Action::SpecialNStart)
                .is_none()
        );
        assert!(
            fighter_data
                .attack(crate::game::Action::SpecialNStart, None, false)
                .is_some()
        );
    }
}
