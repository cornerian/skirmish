//! Native resource fields for the ordinary fighter push callback.
use serde::{Deserialize, Serialize};

pub use crate::fighter::nudge::Rules;

/// Per-fighter values already scaled as `ft_data->x50` by the source loader.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attributes {
    pub center_offset: f32,
    pub half_width: f32,
    #[serde(default)]
    pub nudge_disabled: bool,
    #[serde(default)]
    pub overlap_disabled: bool,
}
