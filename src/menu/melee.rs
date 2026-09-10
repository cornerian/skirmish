//! Declarative contracts reproduced from Melee's menu source.
//!
//! The values in this module come from the pinned `mnmain.c` and
//! `mn_forward.h` snapshot. Rendering and destination ownership deliberately
//! remain outside the menu state machine.

use super::{
    AnimationCue, AnimationId, DestinationId, EnableCondition, FrameRange, ItemId,
    ItemPresentation, MenuAction, MenuDefinition, MenuId, MenuItem, NavigationAxis, SoundId,
    StartBehavior, TextId,
    interaction::{HitRect, InteractionMap, ItemHitRegion},
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

/// Replaceable host mouse policy derived from the five visible label quads.
///
/// Melee has no mouse hitboxes. These authored 640×480 rectangles project the
/// source archive's frame-five `MenMainConTop` anchors and `MenMainCursor`
/// label geometry through its neutral camera, add eight horizontal pixels of
/// pointer tolerance, and partition vertical gaps at their midpoints. They are
/// deliberately presentation data rather than menu-runtime branches.
pub const MAIN_LABEL_HIT_RECTS: [(MainItem, HitRect); 5] = [
    (
        MainItem::OnePlayer,
        HitRect {
            min: [137.910_2, 123.670_8],
            max: [353.420_5, 171.103_8],
        },
    ),
    (
        MainItem::Versus,
        HitRect {
            min: [85.575_3, 171.103_8],
            max: [300.879_4, 219.339_4],
        },
    ),
    (
        MainItem::Trophies,
        HitRect {
            min: [42.916_2, 219.339_4],
            max: [257.620_3, 269.427_7],
        },
    ),
    (
        MainItem::Options,
        HitRect {
            min: [75.242, 269.427_7],
            max: [288.768_6, 319.238_4],
        },
    ),
    (
        MainItem::Data,
        HitRect {
            min: [54.287_3, 319.238_4],
            max: [268.156_8, 370.299_2],
        },
    ),
];

pub fn main_interaction_map() -> InteractionMap {
    InteractionMap {
        regions: MAIN_LABEL_HIT_RECTS
            .into_iter()
            .map(|(item, bounds)| ItemHitRegion {
                item: ItemId::from(item.id()),
                bounds,
            })
            .collect(),
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
    fn main_pointer_policy_is_valid_and_has_unambiguous_vertical_cuts() {
        let definition = main_definition();
        let interaction = main_interaction_map();
        interaction.validate(&definition).unwrap();

        for ((item, bounds), region) in MAIN_LABEL_HIT_RECTS.iter().zip(&interaction.regions) {
            let center = [
                (bounds.min[0] + bounds.max[0]) * 0.5,
                (bounds.min[1] + bounds.max[1]) * 0.5,
            ];
            assert_eq!(interaction.hit_test(center).unwrap().as_str(), item.id());
            assert_eq!(region.item.as_str(), item.id());
        }
        for rows in MAIN_LABEL_HIT_RECTS.windows(2) {
            assert_eq!(rows[0].1.max[1], rows[1].1.min[1]);
        }
    }

    #[test]
    fn main_pointer_policy_matches_pinned_frame_five_projection() {
        // Independently recorded from the pinned source/archive review. These
        // are the visible label AABBs before the host-only pointer tolerance
        // and gap partitioning encoded by MAIN_LABEL_HIT_RECTS.
        const MELEE_REVISION: &str = "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9";
        const ARCHIVE: &str = "MnMaAll.dat";
        const ARCHIVE_SHA256: &str =
            "895e1895e004f84b2cec5583402dedc25ff6173b179f294d50e76945bae32ff0";
        const SOURCE_ROOTS: [(&str, u32); 2] = [
            ("MenMainConTop_Top_joint", 435_944),
            ("MenMainCursor_Top_joint", 756_884),
        ];
        const SOURCE_FRAME: f32 = 5.0;
        const CANVAS: [f32; 2] = [640.0, 480.0];
        const CAMERA_EYE: [f32; 3] = [0.0, 0.0, 51.0];
        const CAMERA_VERTICAL_FOV_DEGREES: f64 = 41.538_997_65;
        const CAMERA_ASPECT: f64 = 1.333_333_015;
        const POINTER_TOLERANCE: f32 = 8.0;
        const EPSILON: f32 = 0.000_1;
        const PROJECTED_LABELS: [(MainItem, HitRect); 5] = [
            (
                MainItem::OnePlayer,
                HitRect {
                    min: [145.910_2, 131.670_8],
                    max: [345.420_5, 164.772_1],
                },
            ),
            (
                MainItem::Versus,
                HitRect {
                    min: [93.575_3, 177.435_5],
                    max: [292.879_4, 211.196_7],
                },
            ),
            (
                MainItem::Trophies,
                HitRect {
                    min: [50.916_2, 227.482_1],
                    max: [249.620_3, 261.855_3],
                },
            ),
            (
                MainItem::Options,
                HitRect {
                    min: [83.242, 277.0],
                    max: [280.768_6, 312.084],
                },
            ),
            (
                MainItem::Data,
                HitRect {
                    min: [62.287_3, 326.392_8],
                    max: [260.156_8, 362.299_2],
                },
            ),
        ];

        let provenance = format!(
            "{ARCHIVE} ({ARCHIVE_SHA256}) at Melee {MELEE_REVISION}; roots {SOURCE_ROOTS:?}; \
             frame {SOURCE_FRAME}; {CANVAS:?} camera eye {CAMERA_EYE:?}, vertical FOV \
             {CAMERA_VERTICAL_FOV_DEGREES}, aspect {CAMERA_ASPECT}"
        );
        assert!(
            (f64::from(CANVAS[0]) / f64::from(CANVAS[1]) - CAMERA_ASPECT).abs() <= 0.000_001,
            "invalid pinned projection provenance: {provenance}"
        );

        for (index, ((actual_item, actual), (expected_item, label))) in MAIN_LABEL_HIT_RECTS
            .iter()
            .zip(PROJECTED_LABELS)
            .enumerate()
        {
            assert_eq!(actual_item, &expected_item, "{provenance}");
            let expected = HitRect {
                min: [
                    label.min[0] - POINTER_TOLERANCE,
                    if index == 0 {
                        label.min[1] - POINTER_TOLERANCE
                    } else {
                        (PROJECTED_LABELS[index - 1].1.max[1] + label.min[1]) * 0.5
                    },
                ],
                max: [
                    label.max[0] + POINTER_TOLERANCE,
                    if index + 1 == PROJECTED_LABELS.len() {
                        label.max[1] + POINTER_TOLERANCE
                    } else {
                        (label.max[1] + PROJECTED_LABELS[index + 1].1.min[1]) * 0.5
                    },
                ],
            };

            for axis in 0..2 {
                assert!(
                    (actual.min[axis] - expected.min[axis]).abs() <= EPSILON,
                    "{expected_item:?} minimum axis {axis} drifted from {expected:?}: \
                     {actual:?}; {provenance}"
                );
                assert!(
                    (actual.max[axis] - expected.max[axis]).abs() <= EPSILON,
                    "{expected_item:?} maximum axis {axis} drifted from {expected:?}: \
                     {actual:?}; {provenance}"
                );
            }
        }
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
