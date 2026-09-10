//! Declarative contracts reproduced from Melee's menu source.
//!
//! The values in this module come from the pinned `mnmain.c` and
//! `mn_forward.h` snapshot. Rendering and destination ownership deliberately
//! remain outside the menu state machine.

use super::{
    AnimationCue, AnimationId, DestinationId, EnableCondition, FrameRange, ItemId,
    ItemPresentation, MenuAction, MenuDefinition, MenuId, MenuItem, NavigationAxis, SoundId,
    StartBehavior, TextId,
};

pub const MAIN_MENU_ID: &str = "melee.main";
pub const TITLE_DESTINATION_ID: &str = "melee.scene.title";
pub const MAIN_SELECTION_ANIMATION_ID: &str = "melee.mnmaall.contop.main.selection";

/// Source-order identity of a Main-menu selection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum MainItem {
    #[default]
    OnePlayer = 0,
    Versus = 1,
    Trophies = 2,
    Options = 3,
    Data = 4,
}

impl MainItem {
    pub const ALL: [Self; 5] = [
        Self::OnePlayer,
        Self::Versus,
        Self::Trophies,
        Self::Options,
        Self::Data,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::OnePlayer => "melee.main.one-player",
            Self::Versus => "melee.main.versus",
            Self::Trophies => "melee.main.trophies",
            Self::Options => "melee.main.options",
            Self::Data => "melee.main.data",
        }
    }

    pub const fn destination(self) -> &'static str {
        match self {
            Self::OnePlayer => "melee.menu.one-player",
            Self::Versus => "melee.menu.versus",
            Self::Trophies => "melee.menu.trophies",
            Self::Options => "melee.menu.options",
            Self::Data => "melee.menu.data",
        }
    }

    pub const fn description(self) -> u32 {
        0x81 + self as u32
    }

    pub const fn selection_frames(self) -> FrameRange {
        let start = 50.0 * self as u8 as f32;
        FrameRange {
            start,
            end: start + 49.0,
            loop_start: Some(start + 20.0),
        }
    }
}

/// The source-backed Main menu with its ordinary one-player default.
pub fn main_definition() -> MenuDefinition {
    main_definition_with_default(MainItem::OnePlayer)
}

/// Build the same menu while restoring a source-provided selection.
pub fn main_definition_with_default(default: MainItem) -> MenuDefinition {
    MenuDefinition {
        id: MenuId::from(MAIN_MENU_ID),
        items: MainItem::ALL.into_iter().map(main_item).collect(),
        default_item: ItemId::from(default.id()),
        navigation_axis: NavigationAxis::Vertical,
        initial_cooldown_frames: 20,
        move_sound: Some(SoundId(2)),
        back: Some(MenuAction {
            destination: DestinationId::from(TITLE_DESTINATION_ID),
            sound: Some(SoundId(0)),
            transition: None,
            cooldown_frames: 5,
        }),
        // Physical Start is aliased to PAD_CONFIRM before mn_8022DB10 runs.
        start: StartBehavior::Confirm,
    }
}

fn main_item(item: MainItem) -> MenuItem {
    MenuItem {
        id: ItemId::from(item.id()),
        enabled_when: EnableCondition::Always,
        presentation: ItemPresentation {
            description: Some(TextId(item.description())),
            animation: AnimationCue {
                id: AnimationId::from(MAIN_SELECTION_ANIMATION_ID),
                frames: item.selection_frames(),
            },
        },
        confirm: Some(MenuAction {
            destination: DestinationId::from(item.destination()),
            sound: Some(SoundId(1)),
            // The source plays paired incoming/outgoing clips. That request
            // belongs to the transition service rather than this single-cue
            // compatibility field.
            transition: None,
            cooldown_frames: 5,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{ActionTrigger, Direction, InputFrame, MenuCommand, MenuEffect, MenuRuntime};

    #[test]
    fn main_contract_matches_the_pinned_source_tables() {
        let definition = main_definition();
        definition.validate().unwrap();

        assert_eq!(definition.id.as_str(), MAIN_MENU_ID);
        assert_eq!(definition.default_item.as_str(), MainItem::OnePlayer.id());
        assert_eq!(definition.initial_cooldown_frames, 20);
        assert_eq!(definition.move_sound, Some(SoundId(2)));
        assert_eq!(definition.start, StartBehavior::Confirm);
        assert_eq!(definition.items.len(), 5);

        for (index, (actual, expected)) in definition.items.iter().zip(MainItem::ALL).enumerate() {
            assert_eq!(actual.id.as_str(), expected.id());
            assert_eq!(actual.enabled_when, EnableCondition::Always);
            assert_eq!(
                actual.presentation.description,
                Some(TextId(0x81 + index as u32))
            );
            assert_eq!(
                actual.presentation.animation.id.as_str(),
                MAIN_SELECTION_ANIMATION_ID
            );
            assert_eq!(
                actual.presentation.animation.frames,
                expected.selection_frames()
            );
            let action = actual.confirm.as_ref().unwrap();
            assert_eq!(action.destination.as_str(), expected.destination());
            assert_eq!(action.sound, Some(SoundId(1)));
            assert_eq!(action.cooldown_frames, 5);
        }

        let back = definition.back.as_ref().unwrap();
        assert_eq!(back.destination.as_str(), TITLE_DESTINATION_ID);
        assert_eq!(back.sound, Some(SoundId(0)));
        assert_eq!(back.cooldown_frames, 5);
    }

    #[test]
    fn main_runtime_preserves_vertical_wrap_priority_and_restored_selection() {
        let mut runtime =
            MenuRuntime::new(main_definition_with_default(MainItem::Data), []).unwrap();
        for _ in 0..20 {
            assert!(runtime.tick(&InputFrame::default()).is_empty());
        }

        runtime.tick(&[MenuCommand::Navigate(Direction::Down)].into());
        assert_eq!(runtime.selected().id.as_str(), MainItem::OnePlayer.id());

        let effects = runtime.tick(
            &[
                MenuCommand::Navigate(Direction::Down),
                MenuCommand::Navigate(Direction::Up),
            ]
            .into(),
        );
        assert!(matches!(
            effects.as_slice(),
            [MenuEffect::SelectionChanged { selected, sound: Some(SoundId(2)), .. }]
                if selected.as_str() == MainItem::Data.id()
        ));

        let effects = runtime.tick(&[MenuCommand::Back, MenuCommand::Start].into());
        assert!(matches!(
            effects.as_slice(),
            [MenuEffect::ActionRequested {
                trigger: ActionTrigger::Start,
                item: Some(item),
                action,
            }] if item.as_str() == MainItem::Data.id()
                && action.destination.as_str() == MainItem::Data.destination()
        ));
    }
}
