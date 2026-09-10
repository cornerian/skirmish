//! Declarative authored-space hit regions, independent of any windowing API.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{ItemId, MenuDefinition};

/// Half-open axis-aligned bounds in the presentation's authored coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitRect {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl HitRect {
    pub fn contains(self, point: [f32; 2]) -> bool {
        point[0].is_finite()
            && point[1].is_finite()
            && self.min[0] <= point[0]
            && point[0] < self.max[0]
            && self.min[1] <= point[1]
            && point[1] < self.max[1]
    }

    fn is_valid(self) -> bool {
        self.min.into_iter().chain(self.max).all(f32::is_finite)
            && self.min[0] < self.max[0]
            && self.min[1] < self.max[1]
    }
}

/// One region may target an item; multiple regions may target the same item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ItemHitRegion {
    pub item: ItemId,
    pub bounds: HitRect,
}

/// Replaceable pointer layout for one menu presentation.
///
/// Regions declared later are treated as visually on top when they overlap.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InteractionMap {
    pub regions: Vec<ItemHitRegion>,
}

impl InteractionMap {
    pub fn validate(&self, definition: &MenuDefinition) -> Result<(), InteractionError> {
        let items = definition
            .items
            .iter()
            .map(|item| &item.id)
            .collect::<BTreeSet<_>>();
        for (index, region) in self.regions.iter().enumerate() {
            if !items.contains(&region.item) {
                return Err(InteractionError::UnknownItem {
                    index,
                    item: region.item.clone(),
                });
            }
            if !region.bounds.is_valid() {
                return Err(InteractionError::InvalidBounds { index });
            }
        }
        Ok(())
    }

    pub fn hit_test(&self, point: [f32; 2]) -> Option<&ItemId> {
        self.regions
            .iter()
            .rev()
            .find(|region| region.bounds.contains(point))
            .map(|region| &region.item)
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InteractionError {
    #[error("hit region {index} references missing menu item {item:?}")]
    UnknownItem { index: usize, item: ItemId },
    #[error("hit region {index} has non-finite, empty, or reversed bounds")]
    InvalidBounds { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{
        AnimationCue, AnimationId, EnableCondition, FrameRange, ItemPresentation, MenuId, MenuItem,
        NavigationAxis, StartBehavior,
    };

    fn definition() -> MenuDefinition {
        let item = |id: &str| MenuItem {
            id: id.into(),
            enabled_when: EnableCondition::Always,
            presentation: ItemPresentation {
                description: None,
                animation: AnimationCue {
                    id: AnimationId::from("idle"),
                    frames: FrameRange {
                        start: 0.0,
                        end: 1.0,
                        loop_start: None,
                    },
                },
            },
            confirm: None,
        };
        MenuDefinition {
            id: MenuId::from("test"),
            items: vec![item("under"), item("over")],
            default_item: "under".into(),
            navigation_axis: NavigationAxis::Vertical,
            initial_cooldown_frames: 0,
            move_sound: None,
            back: None,
            start: StartBehavior::Ignore,
        }
    }

    fn map() -> InteractionMap {
        InteractionMap {
            regions: vec![
                ItemHitRegion {
                    item: "under".into(),
                    bounds: HitRect {
                        min: [10.0, 20.0],
                        max: [30.0, 40.0],
                    },
                },
                ItemHitRegion {
                    item: "over".into(),
                    bounds: HitRect {
                        min: [20.0, 30.0],
                        max: [40.0, 50.0],
                    },
                },
            ],
        }
    }

    #[test]
    fn hit_testing_is_half_open_and_later_regions_are_on_top() {
        let map = map();
        assert_eq!(map.hit_test([10.0, 20.0]).unwrap().as_str(), "under");
        assert_eq!(map.hit_test([20.0, 30.0]).unwrap().as_str(), "over");
        assert_eq!(map.hit_test([39.999, 49.999]).unwrap().as_str(), "over");
        for point in [[9.999, 20.0], [10.0, 50.0], [40.0, 30.0], [f32::NAN, 30.0]] {
            assert_eq!(map.hit_test(point), None, "{point:?}");
        }
    }

    #[test]
    fn validation_rejects_unknown_items_and_invalid_bounds() {
        let definition = definition();
        let mut map = map();
        assert_eq!(map.validate(&definition), Ok(()));

        map.regions[1].item = "missing".into();
        assert_eq!(
            map.validate(&definition),
            Err(InteractionError::UnknownItem {
                index: 1,
                item: "missing".into(),
            })
        );

        map.regions[1].item = "over".into();
        map.regions[1].bounds.max[0] = map.regions[1].bounds.min[0];
        assert_eq!(
            map.validate(&definition),
            Err(InteractionError::InvalidBounds { index: 1 })
        );
        map.regions[1].bounds.max[0] = f32::INFINITY;
        assert_eq!(
            map.validate(&definition),
            Err(InteractionError::InvalidBounds { index: 1 })
        );
    }
}
