//! Exact blast-line ordering and top-death selection from `ftCo_800D3158`.

use crate::random::HsdRng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Left,
    Right,
    Down,
    Up,
    UpStar,
    UpStarIce,
    UpScreen,
    UpScreenIce,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Query {
    pub excluded: [bool; 5],
    pub position: [f32; 2],
    pub blast: [f32; 4],
    pub grounded: bool,
    pub forced_top_eligible: bool,
    pub knockback_y: f32,
    pub top_knockback_threshold: f32,
    pub force_normal_top: bool,
    pub camera_disables_screen: bool,
    pub screen_chance_percent: i32,
    pub ice: bool,
}

/// Returns the selected death and advances RNG exactly where the source does.
pub fn select(query: Query, rng: &mut HsdRng) -> Option<Kind> {
    if query.excluded.into_iter().any(|value| value) {
        return None;
    }
    let [left, right, bottom, top] = query.blast;
    let [x, y] = query.position;
    if x > right {
        return Some(Kind::Right);
    }
    if x < left {
        return Some(Kind::Left);
    }
    if y > top
        && (query.grounded
            || query.forced_top_eligible
            || query.knockback_y > query.top_knockback_threshold)
    {
        if query.force_normal_top {
            return Some(Kind::Up);
        }
        let roll = rng.randi(100) + 1;
        if !query.camera_disables_screen && query.screen_chance_percent >= roll {
            return Some(if query.ice {
                Kind::UpScreenIce
            } else {
                Kind::UpScreen
            });
        }
        return Some(if query.ice {
            Kind::UpStarIce
        } else {
            Kind::UpStar
        });
    }
    if y < bottom {
        return Some(Kind::Down);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query() -> Query {
        Query {
            excluded: [false; 5],
            position: [0.0, 11.0],
            blast: [-10.0, 10.0, -10.0, 10.0],
            grounded: false,
            forced_top_eligible: false,
            knockback_y: 2.0,
            top_knockback_threshold: 1.0,
            force_normal_top: false,
            camera_disables_screen: false,
            screen_chance_percent: 100,
            ice: false,
        }
    }

    #[test]
    fn side_order_precedes_top_and_only_random_top_selection_draws() {
        let mut rng = HsdRng::new(7);
        let initial = rng.seed();
        let mut q = query();
        q.position[0] = 11.0;
        assert_eq!(select(q, &mut rng), Some(Kind::Right));
        assert_eq!(rng.seed(), initial);

        q.position[0] = 0.0;
        q.force_normal_top = true;
        assert_eq!(select(q, &mut rng), Some(Kind::Up));
        assert_eq!(rng.seed(), initial);

        q.force_normal_top = false;
        q.camera_disables_screen = true;
        assert_eq!(select(q, &mut rng), Some(Kind::UpStar));
        assert_ne!(rng.seed(), initial);
    }
}
