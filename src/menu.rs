//! Deterministic, renderer-independent menu flow.
//!
//! Hosts translate controller, keyboard, and pointer events into the same
//! [`MenuCommand`] vocabulary. Definitions contain stable resource identities
//! and presentation cues, while this module owns only selection, priority,
//! cooldown, and action dispatch.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod interaction;
pub mod melee;

use interaction::InteractionMap;

macro_rules! string_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }
    };
}

string_id!(MenuId, "Stable identity of a declarative menu.");
string_id!(ItemId, "Stable identity of a selectable menu item.");
string_id!(
    DestinationId,
    "Stable identity consumed by the application's scene/destination service."
);
string_id!(
    ConditionId,
    "Stable identity of an externally supplied availability condition."
);
string_id!(
    AnimationId,
    "Stable identity of an animation owned by the presentation layer."
);

/// One destination-owned menu entry ready for a shared host adapter.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuEntry {
    pub definition: MenuDefinition,
    pub interaction: InteractionMap,
}

/// Original audio-system cue identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SoundId(pub u32);

/// Original SIS message identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TextId(pub u32);

/// Inclusive authored animation interval with an optional loop point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameRange {
    pub start: f32,
    pub end: f32,
    pub loop_start: Option<f32>,
}

impl FrameRange {
    fn is_valid(self) -> bool {
        self.start.is_finite()
            && self.end.is_finite()
            && self.start <= self.end
            && self
                .loop_start
                .is_none_or(|frame| frame.is_finite() && self.start <= frame && frame <= self.end)
    }
}

/// Presentation request associated with a state or transition.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationCue {
    pub id: AnimationId,
    pub frames: FrameRange,
}

/// Presentation resources selected with an item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemPresentation {
    pub description: Option<TextId>,
    pub animation: AnimationCue,
}

/// Availability is data, not a branch embedded in menu navigation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "condition")]
pub enum EnableCondition {
    Always,
    Flag(ConditionId),
}

/// A destination request and all observable cues that accompany it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuAction {
    pub destination: DestinationId,
    pub sound: Option<SoundId>,
    pub transition: Option<AnimationCue>,
    pub cooldown_frames: u16,
}

/// One ordered selectable entry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuItem {
    pub id: ItemId,
    pub enabled_when: EnableCondition,
    pub presentation: ItemPresentation,
    pub confirm: Option<MenuAction>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NavigationAxis {
    Vertical,
    Horizontal,
    Both,
}

impl NavigationAxis {
    fn step(self, direction: Direction) -> Option<Step> {
        match (self, direction) {
            (Self::Vertical | Self::Both, Direction::Up)
            | (Self::Horizontal | Self::Both, Direction::Left) => Some(Step::Previous),
            (Self::Vertical | Self::Both, Direction::Down)
            | (Self::Horizontal | Self::Both, Direction::Right) => Some(Step::Next),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "action")]
pub enum StartBehavior {
    Ignore,
    Confirm,
    Action(MenuAction),
}

/// Fully declarative behavior and presentation references for one menu.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MenuDefinition {
    pub id: MenuId,
    pub items: Vec<MenuItem>,
    pub default_item: ItemId,
    pub navigation_axis: NavigationAxis,
    pub initial_cooldown_frames: u16,
    pub move_sound: Option<SoundId>,
    pub back: Option<MenuAction>,
    pub start: StartBehavior,
}

impl MenuDefinition {
    pub fn validate(&self) -> Result<(), DefinitionError> {
        validate_id("menu", self.id.as_str())?;
        if self.items.is_empty() {
            return Err(DefinitionError::EmptyMenu);
        }

        let mut ids = BTreeSet::new();
        let mut has_default = false;
        for item in &self.items {
            validate_id("item", item.id.as_str())?;
            if !ids.insert(item.id.clone()) {
                return Err(DefinitionError::DuplicateItem(item.id.clone()));
            }
            has_default |= item.id == self.default_item;
            if let EnableCondition::Flag(condition) = &item.enabled_when {
                validate_id("condition", condition.as_str())?;
            }
            validate_animation(&item.presentation.animation)?;
            if let Some(action) = &item.confirm {
                validate_action(action)?;
            }
        }
        if !has_default {
            return Err(DefinitionError::MissingDefault(self.default_item.clone()));
        }
        if let Some(action) = &self.back {
            validate_action(action)?;
        }
        if let StartBehavior::Action(action) = &self.start {
            validate_action(action)?;
        }
        Ok(())
    }
}

fn validate_id(kind: &'static str, value: &str) -> Result<(), DefinitionError> {
    if value.trim().is_empty() {
        Err(DefinitionError::EmptyId { kind })
    } else {
        Ok(())
    }
}

fn validate_animation(animation: &AnimationCue) -> Result<(), DefinitionError> {
    validate_id("animation", animation.id.as_str())?;
    if animation.frames.is_valid() {
        Ok(())
    } else {
        Err(DefinitionError::InvalidFrameRange(animation.id.clone()))
    }
}

fn validate_action(action: &MenuAction) -> Result<(), DefinitionError> {
    validate_id("destination", action.destination.as_str())?;
    if let Some(animation) = &action.transition {
        validate_animation(animation)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Canonical commands emitted by every host input adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuCommand {
    Navigate(Direction),
    Focus(ItemId),
    Confirm,
    Back,
    Start,
}

/// Commands observed during one deterministic menu tick.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InputFrame {
    commands: Vec<MenuCommand>,
}

impl InputFrame {
    pub fn new(commands: impl IntoIterator<Item = MenuCommand>) -> Self {
        Self {
            commands: commands.into_iter().collect(),
        }
    }

    pub fn commands(&self) -> &[MenuCommand] {
        &self.commands
    }
}

impl<const N: usize> From<[MenuCommand; N]> for InputFrame {
    fn from(commands: [MenuCommand; N]) -> Self {
        Self::new(commands)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionCause {
    Navigation,
    Focus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionTrigger {
    Confirm,
    Back,
    Start,
}

/// Observable work emitted by the state machine for external services.
#[derive(Clone, Debug, PartialEq)]
pub enum MenuEffect {
    SelectionChanged {
        previous: ItemId,
        selected: ItemId,
        cause: SelectionCause,
        sound: Option<SoundId>,
        presentation: ItemPresentation,
    },
    ActionRequested {
        trigger: ActionTrigger,
        item: Option<ItemId>,
        action: MenuAction,
    },
}

/// Deterministic state for a validated [`MenuDefinition`].
pub struct MenuRuntime {
    definition: MenuDefinition,
    enabled_flags: BTreeSet<ConditionId>,
    selected_index: usize,
    cooldown_frames: u16,
}

impl MenuRuntime {
    pub fn new(
        definition: MenuDefinition,
        enabled_flags: impl IntoIterator<Item = ConditionId>,
    ) -> Result<Self, RuntimeError> {
        definition.validate()?;
        let enabled_flags = enabled_flags.into_iter().collect::<BTreeSet<_>>();
        let selected_index = definition
            .items
            .iter()
            .position(|item| item.id == definition.default_item)
            .expect("validated default item");
        if !definition
            .items
            .iter()
            .any(|item| enabled(item, &enabled_flags))
        {
            return Err(RuntimeError::NoEnabledItems);
        }
        if !enabled(&definition.items[selected_index], &enabled_flags) {
            return Err(RuntimeError::DefaultDisabled(
                definition.default_item.clone(),
            ));
        }
        let cooldown_frames = definition.initial_cooldown_frames;
        Ok(Self {
            definition,
            enabled_flags,
            selected_index,
            cooldown_frames,
        })
    }

    pub fn definition(&self) -> &MenuDefinition {
        &self.definition
    }

    pub fn selected(&self) -> &MenuItem {
        &self.definition.items[self.selected_index]
    }

    pub fn item_enabled(&self, id: &ItemId) -> bool {
        self.definition
            .items
            .iter()
            .find(|item| item.id == *id)
            .is_some_and(|item| enabled(item, &self.enabled_flags))
    }

    pub const fn cooldown_frames(&self) -> u16 {
        self.cooldown_frames
    }

    pub fn tick(&mut self, input: &InputFrame) -> Vec<MenuEffect> {
        if self.cooldown_frames > 0 {
            self.cooldown_frames -= 1;
            return Vec::new();
        }

        let mut effects = Vec::with_capacity(2);
        // The final Focus command owns this frame. In particular, a rejected
        // pointer target must not fall back to an earlier target or allow its
        // paired Confirm to activate the previous selection.
        let requested_focus = input
            .commands()
            .iter()
            .rev()
            .find_map(|command| match command {
                MenuCommand::Focus(id) => Some(id),
                _ => None,
            });
        let focused_index = requested_focus.and_then(|id| {
            self.definition
                .items
                .iter()
                .position(|item| item.id == *id && enabled(item, &self.enabled_flags))
        });
        if let Some(index) = focused_index
            && index != self.selected_index
        {
            effects.push(self.select(index, SelectionCause::Focus));
        }

        if contains(input, |command| matches!(command, MenuCommand::Confirm)) {
            if requested_focus.is_none() || focused_index.is_some() {
                self.confirm(ActionTrigger::Confirm, &mut effects);
            }
        } else if contains(input, |command| matches!(command, MenuCommand::Start)) {
            match self.definition.start.clone() {
                StartBehavior::Ignore => {}
                StartBehavior::Confirm => self.confirm(ActionTrigger::Start, &mut effects),
                StartBehavior::Action(action) => {
                    self.request_menu_action(ActionTrigger::Start, Some(action), &mut effects);
                }
            }
        } else if contains(input, |command| matches!(command, MenuCommand::Back)) {
            self.request_menu_action(
                ActionTrigger::Back,
                self.definition.back.clone(),
                &mut effects,
            );
        } else if focused_index.is_none()
            && let Some(step) = navigation_step(&self.definition, input)
            && let Some(index) = self.next_enabled(step)
            && index != self.selected_index
        {
            effects.push(self.select(index, SelectionCause::Navigation));
        }
        effects
    }

    fn confirm(&mut self, trigger: ActionTrigger, effects: &mut Vec<MenuEffect>) {
        let (item, action) = {
            let selected = self.selected();
            (selected.id.clone(), selected.confirm.clone())
        };
        if let Some(action) = action {
            self.cooldown_frames = action.cooldown_frames;
            effects.push(MenuEffect::ActionRequested {
                trigger,
                item: Some(item),
                action,
            });
        }
    }

    fn request_menu_action(
        &mut self,
        trigger: ActionTrigger,
        action: Option<MenuAction>,
        effects: &mut Vec<MenuEffect>,
    ) {
        if let Some(action) = action {
            self.cooldown_frames = action.cooldown_frames;
            effects.push(MenuEffect::ActionRequested {
                trigger,
                item: None,
                action,
            });
        }
    }

    fn select(&mut self, index: usize, cause: SelectionCause) -> MenuEffect {
        let previous = self.selected().id.clone();
        self.selected_index = index;
        let selected = self.selected();
        MenuEffect::SelectionChanged {
            previous,
            selected: selected.id.clone(),
            cause,
            sound: self.definition.move_sound,
            presentation: selected.presentation.clone(),
        }
    }

    fn next_enabled(&self, step: Step) -> Option<usize> {
        let count = self.definition.items.len();
        (1..=count)
            .map(|distance| match step {
                Step::Previous => (self.selected_index + count - distance % count) % count,
                Step::Next => (self.selected_index + distance) % count,
            })
            .find(|&index| enabled(&self.definition.items[index], &self.enabled_flags))
    }
}

fn contains(input: &InputFrame, predicate: impl Fn(&MenuCommand) -> bool) -> bool {
    input.commands().iter().any(predicate)
}

fn navigation_step(definition: &MenuDefinition, input: &InputFrame) -> Option<Step> {
    [
        Direction::Up,
        Direction::Down,
        Direction::Left,
        Direction::Right,
    ]
    .into_iter()
    .find(|direction| {
        contains(
            input,
            |command| matches!(command, MenuCommand::Navigate(found) if found == direction),
        )
    })
    .and_then(|direction| definition.navigation_axis.step(direction))
}

fn enabled(item: &MenuItem, flags: &BTreeSet<ConditionId>) -> bool {
    match &item.enabled_when {
        EnableCondition::Always => true,
        EnableCondition::Flag(condition) => flags.contains(condition),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Previous,
    Next,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum DefinitionError {
    #[error("menu definition has no items")]
    EmptyMenu,
    #[error("{kind} identity must not be empty")]
    EmptyId { kind: &'static str },
    #[error("menu definition contains duplicate item {0:?}")]
    DuplicateItem(ItemId),
    #[error("default item {0:?} does not exist")]
    MissingDefault(ItemId),
    #[error("animation {0:?} has a non-finite or unordered frame range")]
    InvalidFrameRange(AnimationId),
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error(transparent)]
    Definition(#[from] DefinitionError),
    #[error("menu has no enabled items for the supplied conditions")]
    NoEnabledItems,
    #[error("default item {0:?} is disabled for the supplied conditions")]
    DefaultDisabled(ItemId),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(id: &str, start: f32, end: f32, loop_start: Option<f32>) -> AnimationCue {
        AnimationCue {
            id: id.into(),
            frames: FrameRange {
                start,
                end,
                loop_start,
            },
        }
    }

    fn action(destination: &str, sound: u32) -> MenuAction {
        MenuAction {
            destination: destination.into(),
            sound: Some(SoundId(sound)),
            transition: Some(cue("transition", 0.0, 10.0, None)),
            cooldown_frames: 5,
        }
    }

    fn item(id: &str, condition: EnableCondition, text: u32) -> MenuItem {
        MenuItem {
            id: id.into(),
            enabled_when: condition,
            presentation: ItemPresentation {
                description: Some(TextId(text)),
                animation: cue(&format!("idle.{id}"), 0.0, 49.0, Some(20.0)),
            },
            confirm: Some(action(&format!("destination.{id}"), 1)),
        }
    }

    fn definition() -> MenuDefinition {
        MenuDefinition {
            id: "test".into(),
            items: vec![
                item("one", EnableCondition::Always, 0x81),
                item("locked", EnableCondition::Flag("unlock".into()), 0x82),
                item("three", EnableCondition::Always, 0x83),
            ],
            default_item: "one".into(),
            navigation_axis: NavigationAxis::Vertical,
            initial_cooldown_frames: 0,
            move_sound: Some(SoundId(2)),
            back: Some(action("title", 0)),
            start: StartBehavior::Confirm,
        }
    }

    #[test]
    fn navigation_wraps_and_skips_disabled_items_with_melee_priority() {
        let mut runtime = MenuRuntime::new(definition(), []).unwrap();
        let effects = runtime.tick(
            &[
                MenuCommand::Navigate(Direction::Down),
                MenuCommand::Navigate(Direction::Up),
            ]
            .into(),
        );
        assert!(matches!(
            effects.as_slice(),
            [MenuEffect::SelectionChanged {
                selected,
                cause: SelectionCause::Navigation,
                sound: Some(SoundId(2)),
                ..
            }] if selected.as_str() == "three"
        ));
        assert_eq!(runtime.selected().id.as_str(), "three");

        runtime.tick(&[MenuCommand::Navigate(Direction::Down)].into());
        assert_eq!(runtime.selected().id.as_str(), "one");
    }

    #[test]
    fn focus_and_confirm_are_atomic_and_share_the_selected_action_path() {
        let mut runtime = MenuRuntime::new(definition(), [ConditionId::from("unlock")]).unwrap();
        let effects =
            runtime.tick(&[MenuCommand::Confirm, MenuCommand::Focus("three".into())].into());
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            &effects[0],
            MenuEffect::SelectionChanged {
                selected,
                cause: SelectionCause::Focus,
                ..
            } if selected.as_str() == "three"
        ));
        assert!(matches!(
            &effects[1],
            MenuEffect::ActionRequested {
                trigger: ActionTrigger::Confirm,
                item: Some(item),
                action,
            } if item.as_str() == "three" && action.destination.as_str() == "destination.three"
        ));
        assert_eq!(runtime.cooldown_frames(), 5);
    }

    #[test]
    fn invalid_or_disabled_focus_never_changes_selection() {
        let mut runtime = MenuRuntime::new(definition(), []).unwrap();
        assert!(
            runtime
                .tick(&[MenuCommand::Focus("missing".into())].into())
                .is_empty()
        );
        assert!(
            runtime
                .tick(&[MenuCommand::Focus("locked".into())].into())
                .is_empty()
        );
        assert_eq!(runtime.selected().id.as_str(), "one");
    }

    #[test]
    fn rejected_atomic_focus_and_confirm_never_activates_the_old_selection() {
        for target in ["missing", "locked"] {
            let mut runtime = MenuRuntime::new(definition(), []).unwrap();
            assert!(
                runtime
                    .tick(
                        &[
                            MenuCommand::Focus("three".into()),
                            MenuCommand::Focus(target.into()),
                            MenuCommand::Confirm,
                        ]
                        .into()
                    )
                    .is_empty()
            );
            assert_eq!(runtime.selected().id.as_str(), "one");
            assert_eq!(runtime.cooldown_frames(), 0);
        }
    }

    #[test]
    fn confirm_back_start_and_cooldown_have_fixed_priority() {
        let mut runtime = MenuRuntime::new(definition(), []).unwrap();
        let effects =
            runtime.tick(&[MenuCommand::Start, MenuCommand::Back, MenuCommand::Confirm].into());
        assert!(matches!(
            effects.as_slice(),
            [MenuEffect::ActionRequested {
                trigger: ActionTrigger::Confirm,
                item: Some(item),
                ..
            }] if item.as_str() == "one"
        ));
        for expected in (0..5).rev() {
            assert!(runtime.tick(&[MenuCommand::Back].into()).is_empty());
            assert_eq!(runtime.cooldown_frames(), expected);
        }
        assert!(matches!(
            runtime.tick(&[MenuCommand::Back].into()).as_slice(),
            [MenuEffect::ActionRequested {
                trigger: ActionTrigger::Back,
                item: None,
                action,
            }] if action.destination.as_str() == "title" && action.sound == Some(SoundId(0))
        ));
    }

    #[test]
    fn start_behavior_precedes_back_like_melees_confirm_alias() {
        let mut runtime = MenuRuntime::new(definition(), []).unwrap();
        assert!(matches!(
            runtime
                .tick(&[MenuCommand::Back, MenuCommand::Start].into())
                .as_slice(),
            [MenuEffect::ActionRequested {
                trigger: ActionTrigger::Start,
                item: Some(item),
                action,
            }] if item.as_str() == "one" && action.destination.as_str() == "destination.one"
        ));

        let mut ignore = definition();
        ignore.start = StartBehavior::Ignore;
        let mut runtime = MenuRuntime::new(ignore, []).unwrap();
        assert!(
            runtime
                .tick(&[MenuCommand::Back, MenuCommand::Start].into())
                .is_empty()
        );
    }

    #[test]
    fn initial_cooldown_discards_commands_for_exactly_its_tick_count() {
        let mut definition = definition();
        definition.initial_cooldown_frames = 2;
        let mut runtime = MenuRuntime::new(definition, []).unwrap();
        assert!(
            runtime
                .tick(&[MenuCommand::Navigate(Direction::Down)].into())
                .is_empty()
        );
        assert!(
            runtime
                .tick(&[MenuCommand::Navigate(Direction::Down)].into())
                .is_empty()
        );
        assert_eq!(runtime.selected().id.as_str(), "one");
        runtime.tick(&[MenuCommand::Navigate(Direction::Down)].into());
        assert_eq!(runtime.selected().id.as_str(), "three");
    }

    #[test]
    fn definitions_reject_ambiguous_or_invalid_resources() {
        let mut invalid = definition();
        invalid.items.clear();
        assert_eq!(invalid.validate(), Err(DefinitionError::EmptyMenu));

        let mut invalid = definition();
        invalid.items[1].id = "one".into();
        assert_eq!(
            invalid.validate(),
            Err(DefinitionError::DuplicateItem("one".into()))
        );

        let mut invalid = definition();
        invalid.default_item = "missing".into();
        assert_eq!(
            invalid.validate(),
            Err(DefinitionError::MissingDefault("missing".into()))
        );

        let mut invalid = definition();
        invalid.items[0].presentation.animation.frames.loop_start = Some(50.0);
        assert_eq!(
            invalid.validate(),
            Err(DefinitionError::InvalidFrameRange("idle.one".into()))
        );

        let mut invalid = definition();
        invalid.items[0].confirm.as_mut().unwrap().destination = " ".into();
        assert_eq!(
            invalid.validate(),
            Err(DefinitionError::EmptyId {
                kind: "destination"
            })
        );
    }

    #[test]
    fn runtime_rejects_missing_availability_and_disabled_default() {
        let mut all_locked = definition();
        for item in &mut all_locked.items {
            item.enabled_when = EnableCondition::Flag("unlock".into());
        }
        assert!(matches!(
            MenuRuntime::new(all_locked, []),
            Err(RuntimeError::NoEnabledItems)
        ));

        let mut default_locked = definition();
        default_locked.items[0].enabled_when = EnableCondition::Flag("unlock".into());
        assert!(matches!(
            MenuRuntime::new(default_locked, []),
            Err(RuntimeError::DefaultDisabled(item)) if item.as_str() == "one"
        ));
    }

    #[test]
    fn definitions_round_trip_as_declarative_json() {
        let definition = definition();
        let encoded = serde_json::to_vec(&definition).unwrap();
        let decoded: MenuDefinition = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, definition);
    }
}
