//! Generic, dotted-path lookup for character resources.

use crate::game::data::{Attack, HitElement, Hitbox};
use crate::game::grab::Attachment;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct AttackId(u32);

#[derive(Clone, Debug, Default, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    #[serde(flatten)]
    pub values: BTreeMap<String, Value>,
    #[serde(skip)]
    attacks: Vec<Attack>,
    #[serde(skip)]
    attack_ids: BTreeMap<String, AttackId>,
    #[serde(skip)]
    captain_dive_capture: Option<CaptainDiveCapture>,
}

impl Resources {
    pub fn new(values: BTreeMap<String, Value>) -> Result<Self, String> {
        let captain_dive_capture = values
            .get("up")
            .and_then(|up| up.get("capture"))
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|error| format!("invalid up.capture resource: {error}"))
            })
            .transpose()?;
        let mut result = Self {
            values,
            attacks: Vec::new(),
            attack_ids: BTreeMap::new(),
            captain_dive_capture,
        };
        result.index_attacks()?;
        Ok(result)
    }

    pub fn lookup(&self, path: &str) -> Option<&Value> {
        let mut parts = path.split('.');
        let mut value = self.values.get(parts.next()?)?;
        for part in parts {
            value = value.get(part)?;
        }
        Some(value)
    }

    pub fn attack(&self, path: &str) -> Option<&Attack> {
        self.attack_id(path).and_then(|id| self.attack_by_id(id))
    }

    pub(crate) fn attack_id(&self, path: &str) -> Option<AttackId> {
        self.attack_ids.get(path).copied()
    }

    pub(crate) fn attack_by_id(&self, id: AttackId) -> Option<&Attack> {
        self.attacks.get(id.0 as usize)
    }

    /// Populate the typed cache once at resource load.  Simulation code never
    /// serializes or reparses the large pose/hitbox arrays per frame.
    pub fn index_attacks(&mut self) -> Result<(), String> {
        self.attacks.clear();
        self.attack_ids.clear();
        let keys: Vec<String> = self.values.keys().cloned().collect();
        for key in keys {
            self.index_value(&key)?;
        }
        Ok(())
    }

    fn index_value(&mut self, path: &str) -> Result<(), String> {
        let Some(value) = self.lookup(path) else {
            return Ok(());
        };
        if value.get("frames").is_some() {
            // The typed cache owns one copy of each attack. Keep traversal
            // borrowed so nested resource trees are not cloned repeatedly.
            let attack = serde_json::from_value::<Attack>(value.clone())
                .map_err(|error| format!("invalid attack resource {path}: {error}"))?;
            let id = AttackId(self.attacks.len() as u32);
            self.attacks.push(attack);
            self.attack_ids.insert(path.to_owned(), id);
            return Ok(());
        }
        if let Some(object) = value.as_object() {
            let children: Vec<String> = object.keys().map(|k| format!("{path}.{k}")).collect();
            for child in children {
                self.index_value(&child)?;
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for Resources {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let values = BTreeMap::<String, Value>::deserialize(deserializer)?;
        Self::new(values).map_err(serde::de::Error::custom)
    }
}

impl PartialEq for Resources {
    fn eq(&self, other: &Self) -> bool {
        self.values == other.values
    }
}

/// Stable native article identity.  Article ids are the game's numeric item
/// kinds; keeping them typed prevents script callbacks from selecting an
/// article through an ad-hoc string name.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ArticleId(pub u16);

impl ArticleId {
    pub const MARIO_FIRE: Self = Self(48);
    pub const DR_MARIO_VITAMIN: Self = Self(49);
    pub const FOX_LASER: Self = Self(54);
    pub const FALCO_LASER: Self = Self(55);
    pub const FOX_ILLUSION: Self = Self(56);
    pub const FALCO_PHANTASM: Self = Self(57);
    pub const SAMUS_CHARGE: Self = Self(94);
    pub const SAMUS_MISSILE: Self = Self(95);
    pub const LUIGI_FIRE: Self = Self(105);
    pub const LINK_BOMB: Self = Self(58);
    pub const YOUNG_LINK_BOMB: Self = Self(59);
    pub const LINK_BOOMERANG: Self = Self(60);
    pub const YOUNG_LINK_BOOMERANG: Self = Self(61);
    pub const LINK_HOOKSHOT: Self = Self(62);
    pub const YOUNG_LINK_HOOKSHOT: Self = Self(63);
    pub const LINK_ARROW: Self = Self(64);
    pub const YOUNG_LINK_ARROW: Self = Self(65);
    pub const NESS_PK_FIRE: Self = Self(66);
    pub const NESS_PK_FIRE_FLAME: Self = Self(67);
    pub const NESS_PK_FLASH: Self = Self(68);
    pub const NESS_PK_THUNDER: Self = Self(69);
    pub const NESS_PK_THUNDER_TRAIL_1: Self = Self(70);
    pub const NESS_PK_THUNDER_TRAIL_2: Self = Self(71);
    pub const NESS_PK_THUNDER_TRAIL_3: Self = Self(72);
    pub const NESS_PK_THUNDER_TRAIL_4: Self = Self(73);
    pub const LINK_BOW: Self = Self(76);
    pub const YOUNG_LINK_BOW: Self = Self(77);
    pub const NESS_PK_FLASH_EXPLOSION: Self = Self(78);
    pub const SHEIK_NEEDLE_THROW: Self = Self(79);
    pub const SHEIK_NEEDLE_HELD: Self = Self(80);
    pub const SAMUS_BOMB: Self = Self(93);
    pub const SAMUS_GRAPPLE: Self = Self(96);
    pub const SHEIK_VANISH: Self = Self(85);
    pub const SHEIK_CHAIN: Self = Self(97);
    pub const BOWSER_FLAME: Self = Self(100);
    pub const NESS_BAT: Self = Self(101);
    pub const NESS_YOYO: Self = Self(102);
    pub const YOSHI_EGG_THROW: Self = Self(86);
    pub const YOSHI_EGG_LAY: Self = Self(87);
    pub const YOSHI_STAR: Self = Self(88);
    pub const PIKACHU_THUNDER: Self = Self(81);
    pub const PIKACHU_TJOLT_GROUND: Self = Self(89);
    pub const PIKACHU_TJOLT_AIR: Self = Self(90);
    pub const PICHU_THUNDER: Self = Self(82);
    pub const PICHU_TJOLT_GROUND: Self = Self(91);
    pub const PICHU_TJOLT_AIR: Self = Self(92);
    pub const ZELDA_DIN_FIRE: Self = Self(108);
    pub const ZELDA_DIN_FIRE_EXPLOSION: Self = Self(109);
    pub const MEWTWO_DISABLE: Self = Self(110);
    pub const MEWTWO_SHADOW_BALL: Self = Self(112);
    pub const ICE_CLIMBER_ICE: Self = Self(106);
    pub const ICE_CLIMBER_BLIZZARD: Self = Self(107);
    pub const ICE_CLIMBER_GUM_STRINGS: Self = Self(113);
    pub const MARIO_CAPE: Self = Self(83);
    pub const DR_MARIO_SHEET: Self = Self(84);
    pub const PEACH_BOMBER: Self = Self(98);
    pub const PEACH_TURNIP: Self = Self(99);
    pub const PEACH_PARASOL: Self = Self(103);
    pub const PEACH_TOAD: Self = Self(104);
    pub const PEACH_TOAD_SPORE: Self = Self(111);
    pub const GAMEWATCH_GREENHOUSE: Self = Self(114);
    pub const GAMEWATCH_MANHOLE: Self = Self(115);
    pub const GAMEWATCH_FIRE: Self = Self(116);
    pub const GAMEWATCH_PARACHUTE: Self = Self(117);
    pub const GAMEWATCH_TURTLE: Self = Self(118);
    pub const GAMEWATCH_BREATH: Self = Self(119);
    pub const GAMEWATCH_JUDGE: Self = Self(120);
    pub const GAMEWATCH_PANIC: Self = Self(121);
    pub const GAMEWATCH_CHEF: Self = Self(122);
    pub const GAMEWATCH_RESCUE: Self = Self(124);

    /// Character article kinds currently exposed by native fighter data.
    /// These are the pinned `ItemKind` ordinals from `melee/it/forward.h`.
    pub const FIGHTER_ARTICLE_IDS: [Self; 70] = [
        Self::MARIO_FIRE,
        Self::DR_MARIO_VITAMIN,
        Self::FOX_LASER,
        Self::FALCO_LASER,
        Self::LINK_BOMB,
        Self::YOUNG_LINK_BOMB,
        Self::LINK_BOOMERANG,
        Self::YOUNG_LINK_BOOMERANG,
        Self::LINK_HOOKSHOT,
        Self::YOUNG_LINK_HOOKSHOT,
        Self::LINK_ARROW,
        Self::YOUNG_LINK_ARROW,
        Self::NESS_PK_FIRE,
        Self::NESS_PK_FIRE_FLAME,
        Self::NESS_PK_FLASH,
        Self::NESS_PK_THUNDER,
        Self::NESS_PK_THUNDER_TRAIL_1,
        Self::NESS_PK_THUNDER_TRAIL_2,
        Self::NESS_PK_THUNDER_TRAIL_3,
        Self::NESS_PK_THUNDER_TRAIL_4,
        Self::LINK_BOW,
        Self::YOUNG_LINK_BOW,
        Self::NESS_PK_FLASH_EXPLOSION,
        Self::SHEIK_NEEDLE_THROW,
        Self::SHEIK_NEEDLE_HELD,
        Self::SAMUS_BOMB,
        Self::SAMUS_CHARGE,
        Self::SAMUS_MISSILE,
        Self::PEACH_BOMBER,
        Self::PEACH_TURNIP,
        Self::PEACH_PARASOL,
        Self::PEACH_TOAD,
        Self::PEACH_TOAD_SPORE,
        Self::LUIGI_FIRE,
        Self::GAMEWATCH_GREENHOUSE,
        Self::GAMEWATCH_MANHOLE,
        Self::GAMEWATCH_FIRE,
        Self::GAMEWATCH_PARACHUTE,
        Self::GAMEWATCH_TURTLE,
        Self::GAMEWATCH_BREATH,
        Self::GAMEWATCH_JUDGE,
        Self::GAMEWATCH_PANIC,
        Self::GAMEWATCH_CHEF,
        Self::GAMEWATCH_RESCUE,
        Self::FOX_ILLUSION,
        Self::FALCO_PHANTASM,
        Self::SHEIK_VANISH,
        Self::YOSHI_EGG_THROW,
        Self::YOSHI_EGG_LAY,
        Self::YOSHI_STAR,
        Self::MARIO_CAPE,
        Self::DR_MARIO_SHEET,
        Self::PIKACHU_THUNDER,
        Self::PICHU_THUNDER,
        Self::PIKACHU_TJOLT_GROUND,
        Self::PIKACHU_TJOLT_AIR,
        Self::PICHU_TJOLT_GROUND,
        Self::PICHU_TJOLT_AIR,
        Self::SAMUS_GRAPPLE,
        Self::SHEIK_CHAIN,
        Self::BOWSER_FLAME,
        Self::NESS_BAT,
        Self::NESS_YOYO,
        Self::ICE_CLIMBER_ICE,
        Self::ICE_CLIMBER_BLIZZARD,
        Self::ZELDA_DIN_FIRE,
        Self::ZELDA_DIN_FIRE_EXPLOSION,
        Self::MEWTWO_DISABLE,
        Self::MEWTWO_SHADOW_BALL,
        Self::ICE_CLIMBER_GUM_STRINGS,
    ];

    /// Articles whose state machines are owned by native fighter callbacks.
    /// They are catalogued for identity and diagnostics; generic projectile
    /// descriptors must not be used as a substitute for their behavior.
    pub const NATIVE_CALLBACK_ARTICLE_IDS: [Self; 63] = [
        Self::LINK_BOMB,
        Self::YOUNG_LINK_BOMB,
        Self::LINK_BOOMERANG,
        Self::YOUNG_LINK_BOOMERANG,
        Self::LINK_HOOKSHOT,
        Self::YOUNG_LINK_HOOKSHOT,
        Self::LINK_ARROW,
        Self::YOUNG_LINK_ARROW,
        Self::NESS_PK_FIRE,
        Self::NESS_PK_FIRE_FLAME,
        Self::NESS_PK_FLASH,
        Self::NESS_PK_THUNDER,
        Self::NESS_PK_THUNDER_TRAIL_1,
        Self::NESS_PK_THUNDER_TRAIL_2,
        Self::NESS_PK_THUNDER_TRAIL_3,
        Self::NESS_PK_THUNDER_TRAIL_4,
        Self::LINK_BOW,
        Self::YOUNG_LINK_BOW,
        Self::NESS_PK_FLASH_EXPLOSION,
        Self::SHEIK_NEEDLE_THROW,
        Self::SHEIK_NEEDLE_HELD,
        Self::SAMUS_BOMB,
        Self::PEACH_BOMBER,
        Self::PEACH_TURNIP,
        Self::PEACH_PARASOL,
        Self::PEACH_TOAD,
        Self::PEACH_TOAD_SPORE,
        Self::GAMEWATCH_GREENHOUSE,
        Self::GAMEWATCH_MANHOLE,
        Self::GAMEWATCH_FIRE,
        Self::GAMEWATCH_PARACHUTE,
        Self::GAMEWATCH_TURTLE,
        Self::GAMEWATCH_BREATH,
        Self::GAMEWATCH_JUDGE,
        Self::GAMEWATCH_PANIC,
        Self::GAMEWATCH_CHEF,
        Self::GAMEWATCH_RESCUE,
        Self::FOX_ILLUSION,
        Self::FALCO_PHANTASM,
        Self::SHEIK_VANISH,
        Self::SHEIK_CHAIN,
        Self::BOWSER_FLAME,
        Self::NESS_BAT,
        Self::NESS_YOYO,
        Self::YOSHI_EGG_THROW,
        Self::YOSHI_EGG_LAY,
        Self::YOSHI_STAR,
        Self::PIKACHU_THUNDER,
        Self::PIKACHU_TJOLT_GROUND,
        Self::PIKACHU_TJOLT_AIR,
        Self::PICHU_THUNDER,
        Self::PICHU_TJOLT_GROUND,
        Self::PICHU_TJOLT_AIR,
        Self::SAMUS_GRAPPLE,
        Self::ZELDA_DIN_FIRE,
        Self::ZELDA_DIN_FIRE_EXPLOSION,
        Self::MEWTWO_DISABLE,
        Self::MEWTWO_SHADOW_BALL,
        Self::ICE_CLIMBER_ICE,
        Self::ICE_CLIMBER_BLIZZARD,
        Self::ICE_CLIMBER_GUM_STRINGS,
        Self::MARIO_CAPE,
        Self::DR_MARIO_SHEET,
    ];

    pub const fn is_native_callback_owned(self) -> bool {
        matches!(self.0, 56..=104 | 106..=124)
    }
}

/// The small set of article families currently consumed by the simulation.
/// This is deliberately a discriminated resource, not a generic article VM:
/// each family is linked to one native engine path at registration time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ArticleResource {
    Ray {
        lifetime: f32,
        hitboxes: Vec<Hitbox>,
        move_id: u16,
    },
    GravityProjectile {
        speed: f32,
        angle: f32,
        lifetime: f32,
        half_life: f32,
        gravity: f32,
        terminal_velocity: f32,
        surface_multiplier: f32,
        terrain_stop_speed: f32,
        hitboxes: Vec<Hitbox>,
        move_id: u16,
        contact: ProjectileContactPolicy,
    },
}

/// Article-owned contact behavior.  These are closed native policies rather
/// than script-selected strings; unsupported policy combinations are rejected
/// at resource registration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileReflection {
    None,
    ReverseOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectileShield {
    Despawn,
    Bounce,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectilePersistence {
    Despawn,
    Persist,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectileContactPolicy {
    pub reflection: ProjectileReflection,
    pub shield: ProjectileShield,
    pub persistence: ProjectilePersistence,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
pub struct Specials {
    pub character: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub special_attributes: Option<SpecialAttributes>,
    /// Native animation resources keyed by their numeric animation id.  This
    /// is optional so older packs retain their legacy resource shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animations: Option<BTreeMap<u32, NativeAnimationResource>>,
    /// Native article resources keyed by numeric article kind.  JSON object
    /// keys are necessarily strings on the wire, but deserialize directly to
    /// `ArticleId` so scripts and runtime code never carry article names.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articles: Option<BTreeMap<ArticleId, ArticleResource>>,
    #[serde(flatten)]
    pub resources: Resources,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SpecialAttributes {
    pub layout: u8,
    words: Vec<SpecialAttributeWord>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SpecialAttributeWord {
    field_id: u16,
    value: f32,
}

impl SpecialAttributes {
    pub fn get(&self, field_id: u16) -> Option<f32> {
        self.words
            .binary_search_by_key(&field_id, |word| word.field_id)
            .ok()
            .map(|index| self.words[index].value)
    }
}

impl Serialize for SpecialAttributes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut output = serializer.serialize_struct("SpecialAttributes", 2)?;
        output.serialize_field("layout", &self.layout)?;
        let words: Vec<[u32; 2]> = self
            .words
            .iter()
            .map(|word| [u32::from(word.field_id), word.value.to_bits()])
            .collect();
        output.serialize_field("words", &words)?;
        output.end()
    }
}

impl<'de> Deserialize<'de> for SpecialAttributes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            layout: u8,
            words: Vec<[u64; 2]>,
        }
        let wire = Wire::deserialize(deserializer)?;
        if !(1..=5).contains(&wire.layout) {
            return Err(serde::de::Error::custom("unknown special attribute layout"));
        }
        let mut words = Vec::with_capacity(wire.words.len());
        for [field_id, raw] in wire.words {
            let field_id = u16::try_from(field_id).map_err(|_| {
                serde::de::Error::custom("special attribute field id is out of range")
            })?;
            let raw = u32::try_from(raw)
                .map_err(|_| serde::de::Error::custom("special attribute word is out of range"))?;
            let value = f32::from_bits(raw);
            if !value.is_finite() {
                return Err(serde::de::Error::custom("special attribute must be finite"));
            }
            if words
                .last()
                .is_some_and(|word: &SpecialAttributeWord| word.field_id >= field_id)
            {
                return Err(serde::de::Error::custom(
                    "special attribute field ids must be strictly increasing",
                ));
            }
            words.push(SpecialAttributeWord { field_id, value });
        }
        Ok(Self {
            layout: wire.layout,
            words,
        })
    }
}

/// Resource status emitted by the native animation exporter.  Pose-only and
/// unsupported entries are deliberately not attacks: gameplay may use their
/// metadata for diagnostics, but it must never invent hitboxes or a terminal
/// animation edge from them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimationResourceStatus {
    Complete,
    PoseOnly,
    Unsupported,
}

/// One deduplicated native animation resource.  `resource` remains a JSON
/// value because the exporter uses the same wrapper for complete attack
/// samples and pose-only samples; complete entries are decoded once into the
/// private attack cache when `Specials` is loaded.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NativeAnimationResource {
    pub animation_id: u32,
    pub state_ids: Vec<u32>,
    pub status: AnimationResourceStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(skip)]
    attack: Option<Attack>,
}

impl PartialEq for NativeAnimationResource {
    fn eq(&self, other: &Self) -> bool {
        self.animation_id == other.animation_id
            && self.state_ids == other.state_ids
            && self.status == other.status
            && self.resource == other.resource
            && self.reason == other.reason
    }
}

/// Resource-backed semantic description of Falcon Dive's dedicated capture.
/// This is intentionally separate from ordinary grab/throw parameters: the
/// native interaction has its own victim motion and release callback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptainDiveCapture {
    pub attachment: Attachment,
    /// Optional percent applied by the native capture callback. Older
    /// exports omit this field and retain the relation-only lifecycle.
    #[serde(default)]
    pub damage: Option<u32>,
    pub throw: CaptainDiveThrow,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptainDiveThrow {
    pub release_frame: u32,
    pub hit: CaptainDiveHit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptainDiveHit {
    pub damage: u32,
    pub angle_raw: u32,
    pub growth: u32,
    pub fixed: u32,
    pub base: u32,
    pub element: u8,
}

impl CaptainDiveHit {
    /// Decode the native HitElement field emitted by the throw-hit resource.
    /// The exported value is the original zero-based enum, not the reduced
    /// active/inert classification used by the collision resolver.
    pub(crate) fn semantic_element(self) -> Result<HitElement, String> {
        match self.element {
            0 => Ok(HitElement::Normal),
            1 => Ok(HitElement::Fire),
            11 => Ok(HitElement::Inert),
            value => Err(format!("unsupported Captain Dive hit element {value}")),
        }
    }
}

impl<'de> Deserialize<'de> for Specials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mut fields = BTreeMap::<String, Value>::deserialize(deserializer)?;
        let character = match fields.remove("character") {
            Some(Value::String(value)) => value,
            Some(_) => {
                return Err(serde::de::Error::custom(
                    "specials.character must be a string",
                ));
            }
            None => return Err(serde::de::Error::missing_field("character")),
        };
        let animations = fields
            .remove("animations")
            .map(serde_json::from_value::<BTreeMap<u32, NativeAnimationResource>>)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let articles = fields
            .remove("articles")
            .map(serde_json::from_value::<BTreeMap<ArticleId, ArticleResource>>)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let special_attributes = fields
            .remove("special_attributes")
            .map(serde_json::from_value::<SpecialAttributes>)
            .transpose()
            .map_err(serde::de::Error::custom)?;
        let resources = Resources::new(fields).map_err(serde::de::Error::custom)?;
        let mut result = Self {
            character,
            special_attributes,
            animations,
            articles,
            resources,
        };
        result
            .prepare_animations()
            .map_err(serde::de::Error::custom)?;
        Ok(result)
    }
}

impl Specials {
    pub fn lookup(&self, path: &str) -> Option<&Value> {
        self.resources.lookup(path)
    }
    pub fn attack(&self, path: &str) -> Option<&Attack> {
        self.resources.attack(path)
    }

    /// Return a complete native animation attack, if one was exported for
    /// `animation_id`.  Pose-only and unsupported entries intentionally return
    /// no attack resource.
    pub(crate) fn animation_attack(&self, animation_id: u32) -> Option<&Attack> {
        self.animations
            .as_ref()?
            .get(&animation_id)
            .and_then(|resource| resource.attack.as_ref())
    }

    pub(crate) fn complete_animation_attacks(&self) -> impl Iterator<Item = (u32, &Attack)> {
        self.animations
            .iter()
            .flat_map(|animations| animations.iter())
            .filter_map(|(id, resource)| resource.attack.as_ref().map(|attack| (*id, attack)))
    }

    /// Decode complete native animation resources once at the resource
    /// boundary.  Runtime pose and collision paths then borrow the typed
    /// attack without reparsing JSON.
    pub(crate) fn prepare_animations(&mut self) -> Result<(), String> {
        let Some(animations) = self.animations.as_mut() else {
            return Ok(());
        };
        for (key, resource) in animations {
            if resource.animation_id != *key {
                return Err(format!(
                    "animation resource key {key} disagrees with animation_id {}",
                    resource.animation_id
                ));
            }
            if resource.state_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(format!(
                    "animation {key} state_ids must be strictly increasing"
                ));
            }
            resource.attack = match resource.status {
                AnimationResourceStatus::Complete => {
                    let value = resource
                        .resource
                        .as_ref()
                        .ok_or_else(|| format!("complete animation {key} is missing resource"))?;
                    Some(serde_json::from_value(value.clone()).map_err(|error| {
                        format!("invalid complete animation {key} resource: {error}")
                    })?)
                }
                AnimationResourceStatus::PoseOnly => {
                    if resource.resource.is_none() {
                        return Err(format!("pose-only animation {key} is missing resource"));
                    }
                    None
                }
                AnimationResourceStatus::Unsupported => {
                    if resource.resource.is_some() {
                        return Err(format!(
                            "unsupported animation {key} must not include resource"
                        ));
                    }
                    None
                }
            };
        }
        Ok(())
    }

    /// Validate the numeric wrapper's linkage to the native motion-state
    /// table. Every character-specific state must appear under the animation
    /// id selected by its profile, and no wrapper entry may claim a different
    /// state or an unrelated animation.
    pub(crate) fn validate_animation_states(
        &self,
        motion_states: Option<&[super::super::data::MotionStateProfile]>,
    ) -> Result<(), String> {
        let Some(animations) = self.animations.as_ref() else {
            return Ok(());
        };
        let states = motion_states
            .ok_or_else(|| "specials.animations requires motion_states metadata".to_string())?;
        let mut expected = BTreeMap::<u32, Vec<u32>>::new();
        for profile in states.iter().filter(|profile| profile.state_id >= 341) {
            if profile.animation_id < 0 {
                continue;
            }
            expected
                .entry(profile.animation_id as u32)
                .or_default()
                .push(profile.state_id);
        }
        if expected.len() != animations.len() {
            return Err(format!(
                "specials.animations has {} entries, expected {} from motion states",
                animations.len(),
                expected.len()
            ));
        }
        for (animation_id, resource) in animations {
            let Some(states) = expected.get(animation_id) else {
                return Err(format!(
                    "animation {animation_id} is not selected by a motion state"
                ));
            };
            if resource.state_ids != *states {
                return Err(format!(
                    "animation {animation_id} state_ids do not match motion states"
                ));
            }
        }
        Ok(())
    }
    pub fn character_key(&self) -> String {
        self.character.to_ascii_lowercase()
    }

    /// Compare a character key without allocating a normalized copy.
    pub(crate) fn character_key_is(&self, expected: &str) -> bool {
        self.character.eq_ignore_ascii_case(expected)
    }

    pub(crate) fn captain_dive_capture(&self) -> Option<&CaptainDiveCapture> {
        self.resources.captain_dive_capture.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::{AnimationResourceStatus, ArticleId, ArticleResource, Resources, Specials};
    use crate::game::data::MotionStateProfile;
    use serde_json::json;

    #[test]
    fn special_attributes_decode_wire_words_once_and_query_by_id() {
        let specials: Specials = serde_json::from_value(json!({
            "character": "donkey-kong",
            "special_attributes": {
                "layout": 3,
                "words": [[6, 1065353216], [300, 1073741824]]
            }
        }))
        .unwrap();
        let attributes = specials.special_attributes.as_ref().unwrap();
        assert_eq!(attributes.get(6), Some(1.0));
        assert_eq!(attributes.get(300), Some(2.0));
        assert_eq!(
            serde_json::to_value(attributes).unwrap()["words"][1],
            json!([300, 1073741824])
        );
    }

    #[test]
    fn special_attributes_reject_bad_layout_order_and_non_finite_words() {
        for value in [
            json!({"layout": 9, "words": []}),
            json!({"layout": 3, "words": [[2, 0], [1, 0]]}),
            json!({"layout": 3, "words": [[1, 2143289344]]}),
        ] {
            assert!(serde_json::from_value::<super::SpecialAttributes>(value).is_err());
        }
    }

    #[test]
    fn numeric_article_catalog_round_trips_without_string_identity() {
        let specials: Specials = serde_json::from_value(json!({
            "character": "fox",
            "articles": {
                "54": {
                    "kind": "ray",
                    "lifetime": 35.0,
                    "move_id": 20,
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
        .expect("numeric article catalog");
        assert!(
            specials
                .articles
                .as_ref()
                .unwrap()
                .contains_key(&ArticleId::FOX_LASER)
        );
        let wire = serde_json::to_value(&specials).expect("article catalog serialization");
        assert_eq!(wire["articles"]["54"]["kind"], "ray");
        assert!(wire["articles"].get("fox_laser").is_none());
    }

    #[test]
    fn fighter_article_ids_match_pinned_item_kind_values() {
        assert_eq!(
            ArticleId::FIGHTER_ARTICLE_IDS.map(|id| id.0),
            [
                48, 49, 54, 55, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 76,
                77, 78, 79, 80, 93, 94, 95, 98, 99, 103, 104, 111, 105, 114, 115, 116, 117, 118,
                119, 120, 121, 122, 124, 56, 57, 85, 86, 87, 88, 83, 84, 81, 82, 89, 90, 91, 92,
                96, 97, 100, 101, 102, 106, 107, 108, 109, 110, 112, 113
            ]
        );
    }

    #[test]
    fn callback_owned_article_catalog_is_explicit() {
        assert!(ArticleId::LINK_BOMB.is_native_callback_owned());
        assert!(ArticleId::SAMUS_BOMB.is_native_callback_owned());
        assert!(ArticleId::PEACH_TURNIP.is_native_callback_owned());
        assert!(!ArticleId::FOX_LASER.is_native_callback_owned());
    }

    #[test]
    fn gravity_article_catalog_round_trips_typed_contact_policy() {
        let specials: Specials = serde_json::from_value(json!({
            "character": "mario",
            "articles": {
                "48": {
                    "kind": "gravity_projectile",
                    "speed": 1.5,
                    "angle": 0.0,
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
        .expect("gravity article catalog");
        let wire = serde_json::to_value(&specials).expect("gravity catalog serialization");
        assert_eq!(wire["articles"]["48"]["kind"], "gravity_projectile");
        assert_eq!(wire["articles"]["48"]["contact"]["shield"], "bounce");
        let mut missing_half_life = wire;
        missing_half_life["articles"]["48"]
            .as_object_mut()
            .expect("gravity article object")
            .remove("half_life");
        assert!(serde_json::from_value::<Specials>(missing_half_life).is_err());
    }

    #[test]
    fn character_key_is_case_insensitive_without_changing_public_key() {
        let specials = Specials {
            character: "Captain-Falcon".into(),
            special_attributes: None,
            animations: None,
            articles: None,
            resources: Resources::default(),
        };

        assert!(specials.character_key_is("captain-falcon"));
        assert!(specials.character_key_is("CAPTAIN-FALCON"));
        assert!(!specials.character_key_is("falco"));
        assert_eq!(specials.character_key(), "captain-falcon");
    }

    #[test]
    fn samus_article_ids_match_native_item_kinds() {
        assert_eq!(ArticleId::SAMUS_CHARGE.0, 94);
        assert_eq!(ArticleId::SAMUS_MISSILE.0, 95);
    }

    #[test]
    fn numeric_animation_wrapper_links_dk_state_to_complete_attack() {
        let specials: Specials = serde_json::from_value(serde_json::json!({
            "character": "donkey-kong",
            "animations": {
                "331": {
                    "animation_id": 331,
                    "state_ids": [381],
                    "status": "complete",
                    "resource": {"frames": [{"bones": [], "hitboxes": []}]}
                }
            }
        }))
        .expect("typed native animation wrapper");
        let states = [MotionStateProfile {
            state_id: 381,
            animation_id: 331,
            move_id: 20,
            flags: 0,
        }];

        specials
            .validate_animation_states(Some(&states))
            .expect("state 381 linkage");
        let attack = specials
            .animation_attack(331)
            .expect("complete animation attack");
        assert_eq!(attack.frames.len(), 1);
        assert_eq!(
            specials.animations.as_ref().unwrap()[&331].status,
            AnimationResourceStatus::Complete
        );
    }

    #[test]
    fn animation_terminal_delivery_is_one_shot_for_dk_resource() {
        let specials: Specials = serde_json::from_value(serde_json::json!({
            "character": "donkey-kong",
            "animations": {
                "331": {
                    "animation_id": 331,
                    "state_ids": [381],
                    "status": "complete",
                    "resource": {"frames": [{"bones": [], "hitboxes": []}]}
                }
            }
        }))
        .expect("typed native animation wrapper");
        let animation = specials.animation_attack(331).expect("animation 331");
        assert_eq!(animation.frames.len(), 1);
        let mut events = crate::game::script::events::NativeEventState::default();
        assert!(events.animation_due());
        events.mark_animation_delivered();
        assert!(!events.animation_due());
    }
}
