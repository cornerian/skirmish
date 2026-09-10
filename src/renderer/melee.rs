//! Melee menu archive identities used by the native UI bridge.
//!
//! These are source offsets from the pinned `MnMaAll.dat` reference export.
//! The original `mnMain_Scene_OnEnter` selects the same four public symbols;
//! keeping that selection explicit prevents unrelated archive roots from being
//! overlaid. This module remains a default-pose bridge while the archive
//! animation decoder and the complete original source scheduler are connected.

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
    let roots = MAIN_MENU_ROOTS.map(|root| root.offset);
    let mut scene = Scene::load_joint_roots(path, &roots)
        .with_context(|| format!("load {ARCHIVE} main-menu roots"))?;
    ensure!(
        scene.source.as_deref() == Some(ARCHIVE),
        "expected {ARCHIVE}, found {}",
        scene.source.as_deref().unwrap_or("unnamed scene")
    );
    #[cfg(feature = "melee-ui-source")]
    let source_light = melee_ui_sys::light_color_index(0, 0);
    #[cfg(feature = "melee-ui-source")]
    let source_background =
        melee_ui_sys::background_plan(&source_preorder(&scene, MAIN_MENU_ROOTS[0].offset)?)
            .context("execute original mn_80229B2C background constructor")?;
    #[cfg(feature = "melee-ui-source")]
    let source_panel =
        melee_ui_sys::panel_plan(&source_preorder(&scene, MAIN_MENU_ROOTS[1].offset)?)
            .context("execute original mn_80229DC0 panel constructor and process")?;
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
    scene
        .warnings
        .push(match (source_light, source_background, source_panel) {
        (Some(source_light), Some(background), Some(panel)) => format!(
            "Pinned Melee source executed natively over the exported archive topology: mn_8022C010 selected light {source_light}; mn_80229B2C built the {}-node background GObj, registered its process and invoked it {} time, advanced its host frame clock to {}, and invoked its configured render callback {} time; mn_80229DC0 built the {}-node panel GObj, and fn_80229BF4 requested the Main-to-VS transition at frame {} and the idle loop at frame {} across {} direct callback invocations before original user-data teardown. Archive animation-track evaluation is still pending.",
            background.node_count,
            background.process_callbacks,
            background.selected_frame,
            background.render_callbacks,
            panel.node_count,
            panel.versus_transition_frame,
            panel.versus_idle_frame,
            panel.transition_callbacks,
        ),
        (None, None, None) => "Pinned Melee source was not found at build time; the serialized menu preview remains available, but the native source slice was not linked."
            .into(),
        _ => "Pinned Melee source availability was internally inconsistent."
            .into(),
    });
    Ok(scene)
}

#[cfg(feature = "melee-ui-source")]
fn source_preorder(scene: &Scene, root: u32) -> Result<Vec<melee_ui_sys::TopologyNode>> {
    let mut children: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
    ensure!(
        scene.joints.iter().any(|joint| joint.offset == root),
        "missing source topology root {root}"
    );
    for joint in &scene.joints {
        if let Some(parent) = joint.parent {
            children.entry(parent).or_default().push(joint.offset);
        }
    }
    for siblings in children.values_mut() {
        // HSD descriptors in this pinned archive were emitted in sibling order;
        // the sys boundary verifies the known panel preorder identities.
        siblings.sort_unstable();
    }
    fn visit(
        offset: u32,
        parent_index: i32,
        children: &std::collections::HashMap<u32, Vec<u32>>,
        output: &mut Vec<melee_ui_sys::TopologyNode>,
    ) {
        let index = output.len() as i32;
        output.push(melee_ui_sys::TopologyNode {
            source_offset: offset,
            parent_index,
        });
        if let Some(nodes) = children.get(&offset) {
            for &child in nodes {
                visit(child, index, children, output);
            }
        }
    }
    let mut output = Vec::new();
    visit(root, -1, &children, &mut output);
    Ok(output)
}

fn srgb8_to_linear(channel: u8) -> f32 {
    let encoded = f32::from(channel) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}
