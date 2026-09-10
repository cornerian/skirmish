//! Melee menu archive identities used by the native UI bridge.
//!
//! These are source offsets from the pinned `MnMaAll.dat` reference export.
//! The original `mnMain_Scene_OnEnter` selects the same four public symbols;
//! keeping that selection explicit prevents unrelated archive roots from being
//! overlaid. This module is a default-pose bridge while JObj animation and the
//! original source scheduler are connected.

use super::scene::{Camera, Scene};
use anyhow::{Context, Result, ensure};
use std::path::Path;

pub const ARCHIVE: &str = "MnMaAll.dat";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootSymbol {
    pub name: &'static str,
    pub offset: u32,
}

pub const MAIN_MENU_ROOTS: [RootSymbol; 4] = [
    RootSymbol {
        name: "MenMainBack_Top_joint",
        offset: 26_664,
    },
    RootSymbol {
        name: "MenMainPanel_Top_joint",
        offset: 140_584,
    },
    RootSymbol {
        name: "MenMainConTop_Top_joint",
        offset: 435_944,
    },
    RootSymbol {
        name: "MenMainCursor_Top_joint",
        offset: 756_884,
    },
];

/// Load the four roots selected by `mnMain_Scene_OnEnter` in their serialized
/// default pose and apply the archive's authored camera/fog clear color.
///
/// This is intentionally not called a working menu: the original code still
/// needs to attach and evaluate its joint/material/shape animations, clone
/// cursors, and drive visibility before the screen is behaviorally complete.
pub fn load_main_menu_default_pose(path: &Path) -> Result<Scene> {
    #[cfg(feature = "melee-ui-source")]
    let source_light = melee_ui_sys::light_color_index(0, 0);
    let roots = MAIN_MENU_ROOTS.map(|root| root.offset);
    let mut scene = Scene::load_joint_roots(path, &roots)
        .with_context(|| format!("load {ARCHIVE} main-menu roots"))?;
    ensure!(
        scene.source.as_deref() == Some(ARCHIVE),
        "expected {ARCHIVE}, found {}",
        scene.source.as_deref().unwrap_or("unnamed scene")
    );
    scene.camera = Some(Camera {
        eye: [0.0, 0.0, 51.0],
        interest: [0.0, 0.0, 0.0],
        up: [0.0, 1.0, 0.0],
        vertical_fov_radians: 41.538_998_f32.to_radians(),
        aspect: 4.0 / 3.0,
        near: 1.0,
        far: 5_000.0,
    });
    scene.clear_color = [0.0, 0.0, srgb8_to_linear(25), 1.0];
    scene.warnings.push(
        "MnMaAll main-menu roots are in their serialized default pose; original JObj, material and shape animation is not evaluated yet."
            .into(),
    );
    #[cfg(feature = "melee-ui-source")]
    scene.warnings.push(match source_light {
        Some(source_light) => format!(
            "Pinned mnmain.c executed natively for the initial menu light index ({source_light}); scene construction is the next source-runtime boundary."
        ),
        None => "Pinned mnmain.c was not found at build time; the serialized menu preview remains available, but the native source slice was not linked."
            .into(),
    });
    Ok(scene)
}

fn srgb8_to_linear(channel: u8) -> f32 {
    let encoded = f32::from(channel) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}
