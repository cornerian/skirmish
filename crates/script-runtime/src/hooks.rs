//! The finite event hook registry shared by the script compiler and hosts.
//!
//! `HookKind` is the runtime vocabulary.  It is intentionally small and
//! closed: source code cannot add a polling callback, a wildcard callback, or
//! a new combat category.  Source decorator spellings are compile-time
//! metadata and are resolved by [`map_source_decorator`]; aliases therefore do
//! not become additional runtime kinds.

use serde::{Deserialize, Serialize};
use std::fmt;

/// The namespace in which compiler-recognized hook decorators live.
pub const HOOK_NAMESPACE: &str = "skirmish.hook";

/// Stable finite event kinds exposed by the native script boundary.
///
/// The discriminants are part of the callback table ABI.  Keep the order in
/// [`HookKind::ALL`] stable when adding a future kind; append only after the
/// corresponding event contract has been reviewed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[repr(u8)]
pub enum HookKind {
    InputPressed,
    InputReleased,
    StickChanged,
    ActionAvailabilityChanged,
    ActionEntered,
    ActionExited,
    AnimationEnded,
    ScheduledDeadline,
    CommandTraceChanged,
    BeforeHit,
    BeforeReceiveHit,
    AfterHit,
    AfterReceiveHit,
    ProjectileContact,
    Landed,
    SurfaceContact,
    GroundAirChanged,
    PlatformDropDecision,
}

impl HookKind {
    /// Number of callback slots in the stable native ABI.
    pub const COUNT: usize = 18;

    /// Kinds in stable dispatch-table order.
    pub const ALL: &'static [Self] = &[
        Self::InputPressed,
        Self::InputReleased,
        Self::StickChanged,
        Self::ActionAvailabilityChanged,
        Self::ActionEntered,
        Self::ActionExited,
        Self::AnimationEnded,
        Self::ScheduledDeadline,
        Self::CommandTraceChanged,
        Self::BeforeHit,
        Self::BeforeReceiveHit,
        Self::AfterHit,
        Self::AfterReceiveHit,
        Self::ProjectileContact,
        Self::Landed,
        Self::SurfaceContact,
        Self::GroundAirChanged,
        Self::PlatformDropDecision,
    ];

    /// Borrow the stable registry in APIs that prefer a method over a
    /// constant.
    pub const fn all() -> &'static [Self] {
        Self::ALL
    }

    /// Return the stable callback-table index for this kind.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Resolve a stable callback-table index.
    pub const fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::InputPressed),
            1 => Some(Self::InputReleased),
            2 => Some(Self::StickChanged),
            3 => Some(Self::ActionAvailabilityChanged),
            4 => Some(Self::ActionEntered),
            5 => Some(Self::ActionExited),
            6 => Some(Self::AnimationEnded),
            7 => Some(Self::ScheduledDeadline),
            8 => Some(Self::CommandTraceChanged),
            9 => Some(Self::BeforeHit),
            10 => Some(Self::BeforeReceiveHit),
            11 => Some(Self::AfterHit),
            12 => Some(Self::AfterReceiveHit),
            13 => Some(Self::ProjectileContact),
            14 => Some(Self::Landed),
            15 => Some(Self::SurfaceContact),
            16 => Some(Self::GroundAirChanged),
            17 => Some(Self::PlatformDropDecision),
            _ => None,
        }
    }

    /// The canonical serialized/source-independent runtime name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::InputPressed => "input_pressed",
            Self::InputReleased => "input_released",
            Self::StickChanged => "stick_changed",
            Self::ActionAvailabilityChanged => "action_availability_changed",
            Self::ActionEntered => "action_entered",
            Self::ActionExited => "action_exited",
            Self::AnimationEnded => "animation_ended",
            Self::ScheduledDeadline => "scheduled_deadline",
            Self::CommandTraceChanged => "command_trace_changed",
            Self::BeforeHit => "before_hit",
            Self::BeforeReceiveHit => "before_receive_hit",
            Self::AfterHit => "after_hit",
            Self::AfterReceiveHit => "after_receive_hit",
            Self::ProjectileContact => "projectile_contact",
            Self::Landed => "landed",
            Self::SurfaceContact => "surface_contact",
            Self::GroundAirChanged => "ground_air_changed",
            Self::PlatformDropDecision => "platform_drop_decision",
        }
    }

    /// Resolve only canonical runtime names.
    ///
    /// Compile-time decorator aliases such as `animation_end` and `frame`
    /// belong to [`map_source_decorator`], and are deliberately not accepted
    /// here.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|kind| kind.name() == name)
    }

    /// Return the fixed argument contract for this callback.
    pub const fn argument_contract(self) -> HookArgumentContract {
        match self {
            Self::BeforeHit
            | Self::BeforeReceiveHit
            | Self::AfterHit
            | Self::AfterReceiveHit
            | Self::ProjectileContact => HookArgumentContract::FighterHit,
            Self::InputPressed
            | Self::InputReleased
            | Self::StickChanged
            | Self::ActionAvailabilityChanged
            | Self::ActionEntered
            | Self::ActionExited
            | Self::AnimationEnded
            | Self::ScheduledDeadline
            | Self::CommandTraceChanged
            | Self::Landed
            | Self::SurfaceContact
            | Self::GroundAirChanged
            | Self::PlatformDropDecision => HookArgumentContract::FighterContext,
        }
    }

    /// Whether the callback belongs to the five fighter/hit hooks.
    pub const fn is_combat(self) -> bool {
        matches!(self.argument_contract(), HookArgumentContract::FighterHit)
    }

    /// Return the fixed callback result contract for this hook. Decision
    /// points accept either an explicit boolean or an omitted/unit result;
    /// notifications and combat hooks are side-effect-only unit callbacks.
    pub const fn return_contract(self) -> HookReturnContract {
        match self {
            Self::InputPressed
            | Self::InputReleased
            | Self::StickChanged
            | Self::ActionAvailabilityChanged
            | Self::Landed
            | Self::SurfaceContact
            | Self::PlatformDropDecision => HookReturnContract::BoolOrUnit,
            Self::ActionEntered
            | Self::ActionExited
            | Self::AnimationEnded
            | Self::ScheduledDeadline
            | Self::CommandTraceChanged
            | Self::BeforeHit
            | Self::BeforeReceiveHit
            | Self::AfterHit
            | Self::AfterReceiveHit
            | Self::ProjectileContact
            | Self::GroundAirChanged => HookReturnContract::Unit,
        }
    }

    /// Compile-time source spellings for this canonical kind.
    fn source_names(self) -> &'static [&'static str] {
        match self {
            Self::InputPressed => &["input_pressed"],
            Self::InputReleased => &["input_released"],
            Self::StickChanged => &["stick_changed"],
            Self::ActionAvailabilityChanged => &["action_availability_changed"],
            Self::ActionEntered => &["action_entered", "action_enter"],
            Self::ActionExited => &["action_exited", "action_exit"],
            Self::AnimationEnded => &["animation_ended", "animation_end"],
            Self::ScheduledDeadline => &["scheduled_deadline", "frame", "marker", "countdown"],
            Self::CommandTraceChanged => &["command_trace_changed", "command_changed"],
            Self::BeforeHit => &["before_hit"],
            Self::BeforeReceiveHit => &["before_receive_hit"],
            Self::AfterHit => &["after_hit"],
            Self::AfterReceiveHit => &["after_receive_hit"],
            Self::ProjectileContact => &["projectile_contact"],
            Self::Landed => &["landed"],
            Self::SurfaceContact => &["surface_contact"],
            Self::GroundAirChanged => &["ground_air_changed"],
            Self::PlatformDropDecision => &["platform_drop_decision"],
        }
    }

    /// Whether the source decorator carries metadata that must be statically
    /// represented by the compiler.
    fn requires_static_arguments(self) -> bool {
        matches!(
            self,
            Self::InputPressed
                | Self::InputReleased
                | Self::ActionAvailabilityChanged
                | Self::ActionEntered
                | Self::ActionExited
                | Self::AnimationEnded
                | Self::ScheduledDeadline
                | Self::CommandTraceChanged
        )
    }
}

/// The fixed host arguments supplied to a callback.
///
/// Lifecycle callbacks receive `(fighter, context)` and the five combat
/// callbacks receive `(fighter, hit)`. The names are part of the authoring
/// contract even though the native runtime represents the values opaquely.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookArgumentContract {
    FighterContext,
    FighterHit,
}

/// Fixed return values accepted by a callback.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookReturnContract {
    /// A decision hook may return `bool` or omit its return (`unit`).
    BoolOrUnit,
    /// A notification/combat hook is side-effect-only and returns `unit`.
    Unit,
}

impl HookReturnContract {
    pub const fn allows_bool(self) -> bool {
        matches!(self, Self::BoolOrUnit)
    }

    pub const fn allows_unit(self) -> bool {
        true
    }

    pub const fn is_optional_decision(self) -> bool {
        matches!(self, Self::BoolOrUnit)
    }
}

impl HookArgumentContract {
    pub const fn parameter_names(self) -> &'static [&'static str; 2] {
        match self {
            Self::FighterContext => &["fighter", "context"],
            Self::FighterHit => &["fighter", "hit"],
        }
    }

    pub const fn arity(self) -> usize {
        2
    }

    pub const fn is_combat(self) -> bool {
        matches!(self, Self::FighterHit)
    }
}

/// Metadata attached to a compiler-recognized source decorator.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct HookMetadata {
    /// The decorator's payload must be a compile-time static value. In
    /// particular, `@hook.frame(..., at=...)` cannot use a runtime variable.
    pub requires_static_arguments: bool,
    /// Deadline decorators carry a static action-selection filter. This is
    /// compile-time metadata; it is never a new runtime event kind.
    pub action_selection_filter: bool,
    /// Approved source form for a scheduled deadline, when applicable.
    pub deadline_source: Option<DeadlineSource>,
}

impl HookMetadata {
    pub const fn requires_static_args(self) -> bool {
        self.requires_static_arguments
    }

    pub const fn has_action_selection_filter(self) -> bool {
        self.action_selection_filter
    }
}

/// Compile-time source form that lowers to [`HookKind::ScheduledDeadline`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadlineSource {
    Frame,
    Marker,
    Countdown,
}

/// A validated compiler hook binding. This is metadata used while lowering;
/// event dispatch should retain only its canonical [`HookKind`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CompileTimeHook {
    kind: HookKind,
    source_name: &'static str,
    metadata: HookMetadata,
}

impl CompileTimeHook {
    pub const fn kind(self) -> HookKind {
        self.kind
    }

    pub const fn source_name(self) -> &'static str {
        self.source_name
    }

    pub const fn metadata(self) -> HookMetadata {
        self.metadata
    }

    pub const fn requires_static_arguments(self) -> bool {
        self.metadata.requires_static_arguments
    }

    pub const fn requires_static_args(self) -> bool {
        self.requires_static_arguments()
    }

    pub const fn has_action_selection_filter(self) -> bool {
        self.metadata.has_action_selection_filter()
    }
}

/// Errors from source decorator validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HookMappingError {
    InvalidNamespace {
        expected: &'static str,
        found: String,
    },
    UnknownDecorator {
        namespace: String,
        name: String,
    },
}

impl fmt::Display for HookMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidNamespace { expected, found } => write!(
                formatter,
                "hook decorator namespace must be `{expected}`, got `{found}`"
            ),
            Self::UnknownDecorator { namespace, name } => {
                write!(formatter, "unknown hook decorator `@{namespace}.{name}`")
            }
        }
    }
}

impl std::error::Error for HookMappingError {}

/// Map a statically parsed `@hook.<name>` decorator (resolved to the canonical
/// `skirmish.hook` namespace) to one canonical runtime
/// kind. This is the single source-to-runtime mapping operation. Aliases are
/// accepted only in the validated `skirmish.hook` namespace; arbitrary
/// qualified names cannot bypass the allowlist by sharing a leaf name.
pub fn map_source_decorator(
    namespace: &str,
    name: &str,
) -> Result<CompileTimeHook, HookMappingError> {
    if namespace != HOOK_NAMESPACE {
        return Err(HookMappingError::InvalidNamespace {
            expected: HOOK_NAMESPACE,
            found: namespace.to_owned(),
        });
    }
    let kind = HookKind::ALL
        .iter()
        .copied()
        .find(|kind| kind.source_names().contains(&name))
        .ok_or_else(|| HookMappingError::UnknownDecorator {
            namespace: namespace.to_owned(),
            name: name.to_owned(),
        })?;
    Ok(CompileTimeHook {
        kind,
        source_name: kind
            .source_names()
            .iter()
            .copied()
            .find(|candidate| *candidate == name)
            .expect("source name was found in the same registry entry"),
        metadata: metadata_for(kind, name),
    })
}

fn metadata_for(kind: HookKind, source_name: &str) -> HookMetadata {
    let deadline_source = if kind != HookKind::ScheduledDeadline {
        None
    } else {
        match source_name {
            "frame" => Some(DeadlineSource::Frame),
            "marker" => Some(DeadlineSource::Marker),
            "countdown" => Some(DeadlineSource::Countdown),
            _ => None,
        }
    };
    HookMetadata {
        requires_static_arguments: kind.requires_static_arguments(),
        action_selection_filter: deadline_source.is_some(),
        deadline_source,
    }
}

/// Short alias retained for compiler callers.
pub fn map_decorator(namespace: &str, name: &str) -> Result<CompileTimeHook, HookMappingError> {
    map_source_decorator(namespace, name)
}

/// Validate a compiler decorator without exposing a runtime event kind to the
/// parser. The returned value is compile-time metadata and can then be lowered
/// to `CompileTimeHook::kind()` after static argument validation.
pub fn validate_compile_time_hook(
    namespace: &str,
    name: &str,
) -> Result<CompileTimeHook, HookMappingError> {
    map_source_decorator(namespace, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_names_are_unique_and_index_round_trip_is_stable() {
        assert_eq!(HookKind::ALL.len(), HookKind::COUNT);
        for (index, kind) in HookKind::ALL.iter().copied().enumerate() {
            assert_eq!(kind.index(), index);
            assert_eq!(HookKind::from_index(index), Some(kind));
            assert_eq!(HookKind::from_name(kind.name()), Some(kind));
        }
        for (left_index, left) in HookKind::ALL.iter().enumerate() {
            assert!(
                HookKind::ALL[left_index + 1..]
                    .iter()
                    .all(|right| left.name() != right.name()),
                "duplicate hook name `{}`",
                left.name()
            );
        }
        assert_eq!(HookKind::from_index(HookKind::COUNT), None);
    }

    #[test]
    fn registry_has_no_polling_or_umbrella_aliases() {
        for name in [
            "on_frame",
            "on_tick",
            "update",
            "periodic",
            "every_frame",
            "interval",
            "combat",
            "frame",
            "animation_end",
        ] {
            assert_eq!(
                HookKind::from_name(name),
                None,
                "unexpected runtime alias {name}"
            );
        }
    }

    #[test]
    fn source_aliases_map_only_in_hook_namespace() {
        let animation = map_source_decorator("skirmish.hook", "animation_end").unwrap();
        assert_eq!(animation.kind(), HookKind::AnimationEnded);
        assert_eq!(animation.source_name(), "animation_end");

        let frame = map_source_decorator("skirmish.hook", "frame").unwrap();
        assert_eq!(frame.kind(), HookKind::ScheduledDeadline);
        assert!(frame.requires_static_arguments());

        assert!(matches!(
            map_source_decorator("unrelated", "animation_end"),
            Err(HookMappingError::InvalidNamespace { .. })
        ));
        assert!(matches!(
            map_source_decorator("skirmish.hook", "on_frame"),
            Err(HookMappingError::UnknownDecorator { .. })
        ));

        let marker = map_source_decorator("skirmish.hook", "marker").unwrap();
        assert_eq!(marker.kind(), HookKind::ScheduledDeadline);
        assert_eq!(
            marker.metadata().deadline_source,
            Some(DeadlineSource::Marker)
        );
        assert!(marker.has_action_selection_filter());

        let countdown = map_source_decorator("skirmish.hook", "countdown").unwrap();
        assert_eq!(countdown.kind(), HookKind::ScheduledDeadline);
        assert_eq!(
            countdown.metadata().deadline_source,
            Some(DeadlineSource::Countdown)
        );
        assert!(countdown.has_action_selection_filter());

        let command = map_source_decorator("skirmish.hook", "command_changed").unwrap();
        assert_eq!(command.kind(), HookKind::CommandTraceChanged);
    }

    #[test]
    fn argument_contracts_are_lifecycle_or_the_five_combat_hooks() {
        let combat = HookKind::ALL
            .iter()
            .filter(|kind| kind.argument_contract().is_combat())
            .count();
        assert_eq!(combat, 5);
        assert_eq!(
            HookKind::InputPressed.argument_contract().parameter_names(),
            &["fighter", "context"]
        );
        assert_eq!(
            HookKind::BeforeHit.argument_contract().parameter_names(),
            &["fighter", "hit"]
        );
    }

    #[test]
    fn return_contract_is_exhaustive_and_keeps_decisions_optional() {
        let decision_hooks = [
            HookKind::InputPressed,
            HookKind::InputReleased,
            HookKind::StickChanged,
            HookKind::ActionAvailabilityChanged,
            HookKind::Landed,
            HookKind::SurfaceContact,
            HookKind::PlatformDropDecision,
        ];
        let notification_and_combat_hooks = [
            HookKind::ActionEntered,
            HookKind::ActionExited,
            HookKind::AnimationEnded,
            HookKind::ScheduledDeadline,
            HookKind::CommandTraceChanged,
            HookKind::BeforeHit,
            HookKind::BeforeReceiveHit,
            HookKind::AfterHit,
            HookKind::AfterReceiveHit,
            HookKind::ProjectileContact,
            HookKind::GroundAirChanged,
        ];
        assert_eq!(
            decision_hooks.len() + notification_and_combat_hooks.len(),
            HookKind::COUNT
        );
        for kind in decision_hooks {
            assert_eq!(kind.return_contract(), HookReturnContract::BoolOrUnit);
            assert!(kind.return_contract().allows_bool());
            assert!(kind.return_contract().allows_unit());
        }
        for kind in notification_and_combat_hooks {
            assert_eq!(kind.return_contract(), HookReturnContract::Unit);
            assert!(!kind.return_contract().allows_bool());
            assert!(kind.return_contract().allows_unit());
        }
    }
}
