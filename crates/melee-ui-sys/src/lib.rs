//! Safe calls into the currently linked subset of the original Melee menu source.

use std::fmt;

pub const REVISION: &str = "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9";
pub const MNMAIN_SHA256: &str = "cbce53b1c0d06c1eb851cc203b17237c0fef827cb024ba6f6a0544761850b7c2";

#[cfg(skirmish_melee_source)]
unsafe extern "C" {
    fn skirmish_mn_light_color_index(menu_kind: u8, selection: u16) -> i32;
    fn skirmish_mn_digit_at(number: i32, digit: i32) -> i32;
    fn skirmish_mn_build_background_plan(
        topology: *const TopologyNode,
        node_count: u32,
        out: *mut RawBackgroundPlan,
    ) -> i32;
    fn skirmish_mn_build_panel_plan(
        topology: *const TopologyNode,
        node_count: u32,
        out: *mut RawPanelPlan,
    ) -> i32;
}

#[cfg(skirmish_melee_source)]
const SCENE_PLAN_ABI: u32 = 1;
#[cfg(skirmish_melee_source)]
const MODEL_MAIN_BACKGROUND: u32 = 1;
#[cfg(skirmish_melee_source)]
const MODEL_MAIN_PANEL: u32 = 2;
#[cfg(skirmish_melee_source)]
const COMPLETE_SCENE_PLAN_FLAGS: u32 = (1 << 8) - 1;
#[cfg(skirmish_melee_source)]
const MAIN_BACKGROUND_ROOT_OFFSET: u32 = 26_664;
#[cfg(skirmish_melee_source)]
const MAIN_BACKGROUND_NODE_COUNT: usize = 102;
#[cfg(skirmish_melee_source)]
const MAIN_PANEL_ROOT_OFFSET: u32 = 140_584;
#[cfg(skirmish_melee_source)]
const MAIN_PANEL_NODE_COUNT: usize = 106;
#[cfg(skirmish_melee_source)]
const MAIN_PANEL_BACKGROUND_OFFSET: u32 = 140_840;
#[cfg(skirmish_melee_source)]
const MAIN_PANEL_ANIMATION_OFFSET: u32 = 143_976;

#[cfg(skirmish_melee_source)]
static SCENE_RUNTIME: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[repr(C)]
#[derive(Default)]
#[cfg(skirmish_melee_source)]
struct RawBackgroundPlan {
    abi_version: u32,
    model: u32,
    flags: u32,
    classifier: u32,
    process_link: u32,
    process_priority: u32,
    object_kind: u32,
    gx_link: u32,
    render_priority: u32,
    scheduler_slot: u32,
    root_offset: u32,
    node_count: u32,
    frame_requested_nodes: u32,
    evaluated_nodes: u32,
    process_callbacks: u32,
    render_callbacks: u32,
    evaluation_requests: u32,
    pose_evaluated: u32,
    requested_frame: f32,
    selected_frame: f32,
}

#[repr(C)]
#[derive(Default)]
#[cfg(skirmish_melee_source)]
struct RawPanelPlan {
    abi_version: u32,
    model: u32,
    flags: u32,
    classifier: u32,
    process_link: u32,
    process_priority: u32,
    object_kind: u32,
    gx_link: u32,
    render_priority: u32,
    scheduler_slot: u32,
    root_offset: u32,
    background_node_offset: u32,
    panel_node_offset: u32,
    node_count: u32,
    frame_requested_nodes: u32,
    evaluated_nodes: u32,
    evaluation_requests: u32,
    transition_callbacks: u32,
    render_callbacks: u32,
    user_data_released: u32,
    pose_evaluated: u32,
    initial_background_frame: f32,
    initial_panel_frame: f32,
    versus_transition_frame: f32,
    versus_idle_frame: f32,
}

/// One node in source preorder. Parent indices refer to earlier entries; the
/// root uses `-1`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TopologyNode {
    pub source_offset: u32,
    pub parent_index: i32,
}

/// Stable source asset selected by an original menu scene constructor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Model {
    MainBackground,
    MainPanel,
}

/// Pointer-free result of executing `mn_80229B2C` against the host scene runtime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundPlan {
    pub model: Model,
    pub classifier: u32,
    pub process_link: u32,
    pub process_priority: u32,
    pub object_kind: u32,
    pub gx_link: u32,
    pub render_priority: u32,
    pub scheduler_slot: u32,
    pub root_offset: u32,
    pub node_count: u32,
    pub frame_requested_nodes: u32,
    pub evaluated_nodes: u32,
    pub process_callbacks: u32,
    pub render_callbacks: u32,
    pub evaluation_requests: u32,
    pub requested_frame: f32,
    pub selected_frame: f32,
    /// False until a companion descriptor decoder evaluates JObj, material and
    /// shape tracks. The source request is retained without claiming success.
    pub pose_evaluated: bool,
}

/// Pointer-free result of executing `mn_80229DC0` and its original panel
/// process callback against the host scene runtime.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelPlan {
    pub model: Model,
    pub classifier: u32,
    pub process_link: u32,
    pub process_priority: u32,
    pub object_kind: u32,
    pub gx_link: u32,
    pub render_priority: u32,
    pub scheduler_slot: u32,
    pub root_offset: u32,
    pub background_node_offset: u32,
    pub panel_node_offset: u32,
    pub node_count: u32,
    pub frame_requested_nodes: u32,
    pub evaluated_nodes: u32,
    pub evaluation_requests: u32,
    pub transition_callbacks: u32,
    pub render_callbacks: u32,
    pub user_data_released: u32,
    pub initial_background_frame: f32,
    pub initial_panel_frame: f32,
    pub versus_transition_frame: f32,
    pub versus_idle_frame: f32,
    /// False until a companion descriptor decoder evaluates JObj, material and
    /// shape tracks. The host frame-clock state is retained without claiming a
    /// rendered pose.
    pub pose_evaluated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceRuntimeError {
    Invocation(i32),
    InvalidPlan(&'static str),
}

impl fmt::Display for SourceRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invocation(code) => {
                write!(formatter, "Melee host runtime failed with code {code}")
            }
            Self::InvalidPlan(message) => write!(formatter, "invalid Melee scene plan: {message}"),
        }
    }
}

impl std::error::Error for SourceRuntimeError {}

pub const fn available() -> bool {
    cfg!(skirmish_melee_source)
}

/// Execute the original `mn_80229B2C` background constructor.
///
/// The returned plan records the GObj/proc registrations, host frame-clock
/// selection and configured render-callback invocation produced by the source
/// over the validated exported topology. It keeps `pose_evaluated` false
/// until the archive hierarchy and animation descriptors have a real host
/// decoder; merely observing `HSD_JObjAnimAll` is not reported as visual
/// evaluation.
pub fn background_plan(
    topology: &[TopologyNode],
) -> Result<Option<BackgroundPlan>, SourceRuntimeError> {
    #[cfg(skirmish_melee_source)]
    {
        validate_topology(topology, Model::MainBackground)?;
        let _guard = SCENE_RUNTIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut raw = RawBackgroundPlan::default();
        // SAFETY: `raw` is a writable fixed-layout snapshot, access to the
        // C-owned scene runtime is serialized, and no C pointer escapes.
        let status = unsafe {
            skirmish_mn_build_background_plan(topology.as_ptr(), topology.len() as u32, &mut raw)
        };
        if status != 0 {
            return Err(SourceRuntimeError::Invocation(status));
        }
        validate_background_plan(&raw)?;
        Ok(Some(BackgroundPlan {
            model: Model::MainBackground,
            classifier: raw.classifier,
            process_link: raw.process_link,
            process_priority: raw.process_priority,
            object_kind: raw.object_kind,
            gx_link: raw.gx_link,
            render_priority: raw.render_priority,
            scheduler_slot: raw.scheduler_slot,
            root_offset: raw.root_offset,
            node_count: raw.node_count,
            frame_requested_nodes: raw.frame_requested_nodes,
            evaluated_nodes: raw.evaluated_nodes,
            process_callbacks: raw.process_callbacks,
            render_callbacks: raw.render_callbacks,
            evaluation_requests: raw.evaluation_requests,
            requested_frame: raw.requested_frame,
            selected_frame: raw.selected_frame,
            pose_evaluated: raw.pose_evaluated != 0,
        }))
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = topology;
        Ok(None)
    }
}

/// Execute the original `mn_80229DC0` panel constructor and drive its original
/// `fn_80229BF4` process through the main-menu-to-VS transition.
///
/// The returned plan records the GObj/proc registrations, original preorder
/// lookup over the validated exported topology, panel frame requests, configured
/// render-callback invocation and user-data teardown. It deliberately keeps
/// `pose_evaluated` false until the archive hierarchy and animation descriptors
/// have a real host decoder.
pub fn panel_plan(topology: &[TopologyNode]) -> Result<Option<PanelPlan>, SourceRuntimeError> {
    #[cfg(skirmish_melee_source)]
    {
        validate_topology(topology, Model::MainPanel)?;
        let _guard = SCENE_RUNTIME
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut raw = RawPanelPlan::default();
        // SAFETY: `raw` is a writable fixed-layout snapshot, access to the
        // C-owned scene runtime is serialized, and no C pointer escapes.
        let status = unsafe {
            skirmish_mn_build_panel_plan(topology.as_ptr(), topology.len() as u32, &mut raw)
        };
        if status != 0 {
            return Err(SourceRuntimeError::Invocation(status));
        }
        validate_panel_plan(&raw)?;
        Ok(Some(PanelPlan {
            model: Model::MainPanel,
            classifier: raw.classifier,
            process_link: raw.process_link,
            process_priority: raw.process_priority,
            object_kind: raw.object_kind,
            gx_link: raw.gx_link,
            render_priority: raw.render_priority,
            scheduler_slot: raw.scheduler_slot,
            root_offset: raw.root_offset,
            background_node_offset: raw.background_node_offset,
            panel_node_offset: raw.panel_node_offset,
            node_count: raw.node_count,
            frame_requested_nodes: raw.frame_requested_nodes,
            evaluated_nodes: raw.evaluated_nodes,
            evaluation_requests: raw.evaluation_requests,
            transition_callbacks: raw.transition_callbacks,
            render_callbacks: raw.render_callbacks,
            user_data_released: raw.user_data_released,
            initial_background_frame: raw.initial_background_frame,
            initial_panel_frame: raw.initial_panel_frame,
            versus_transition_frame: raw.versus_transition_frame,
            versus_idle_frame: raw.versus_idle_frame,
            pose_evaluated: raw.pose_evaluated != 0,
        }))
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = topology;
        Ok(None)
    }
}

#[cfg(skirmish_melee_source)]
fn validate_topology(topology: &[TopologyNode], model: Model) -> Result<(), SourceRuntimeError> {
    let invalid = |message| Err(SourceRuntimeError::InvalidPlan(message));
    let (root_offset, node_count) = match model {
        Model::MainBackground => (MAIN_BACKGROUND_ROOT_OFFSET, MAIN_BACKGROUND_NODE_COUNT),
        Model::MainPanel => (MAIN_PANEL_ROOT_OFFSET, MAIN_PANEL_NODE_COUNT),
    };
    if topology.len() != node_count
        || topology.first()
            != Some(&TopologyNode {
                source_offset: root_offset,
                parent_index: -1,
            })
    {
        return invalid("topology does not match the pinned model root and node count");
    }
    if model == Model::MainPanel
        && (topology[4].source_offset != MAIN_PANEL_BACKGROUND_OFFSET
            || topology[41].source_offset != MAIN_PANEL_ANIMATION_OFFSET)
    {
        return invalid("panel preorder identities do not match the pinned archive");
    }
    for (index, node) in topology.iter().enumerate().skip(1) {
        let Ok(parent) = usize::try_from(node.parent_index) else {
            return invalid("non-root topology node has no parent");
        };
        if node.source_offset == 0 || parent >= index {
            return invalid("topology parent must precede its nonzero child");
        }
        let mut cursor = index - 1;
        loop {
            if cursor == parent {
                break;
            }
            let Ok(next) = usize::try_from(topology[cursor].parent_index) else {
                return invalid("topology is not in depth-first preorder");
            };
            cursor = next;
        }
        if topology[..index]
            .iter()
            .any(|previous| previous.source_offset == node.source_offset)
        {
            return invalid("duplicate source offset in topology");
        }
    }
    Ok(())
}

#[cfg(skirmish_melee_source)]
fn validate_background_plan(raw: &RawBackgroundPlan) -> Result<(), SourceRuntimeError> {
    let invalid = |message| Err(SourceRuntimeError::InvalidPlan(message));
    if raw.abi_version != SCENE_PLAN_ABI {
        return invalid("unsupported ABI version");
    }
    if raw.model != MODEL_MAIN_BACKGROUND {
        return invalid("unexpected model identity");
    }
    if raw.flags != COMPLETE_SCENE_PLAN_FLAGS {
        return invalid("source constructor did not complete every traced host operation");
    }
    if (raw.classifier, raw.process_link, raw.process_priority) != (4, 5, 0x80) {
        return invalid("unexpected GObj scheduling fields");
    }
    if (raw.object_kind, raw.gx_link, raw.render_priority) != (2, 2, 0x80) {
        return invalid("unexpected JObj/GX attachment fields");
    }
    if raw.root_offset != MAIN_BACKGROUND_ROOT_OFFSET
        || raw.node_count != MAIN_BACKGROUND_NODE_COUNT as u32
    {
        return invalid("background runtime did not retain the pinned archive topology");
    }
    if raw.scheduler_slot != 0 || raw.process_callbacks != 1 {
        return invalid("background process was not registered in slot zero and invoked once");
    }
    if raw.frame_requested_nodes != MAIN_BACKGROUND_NODE_COUNT as u32
        || raw.evaluated_nodes != MAIN_BACKGROUND_NODE_COUNT as u32
        || raw.evaluation_requests != (MAIN_BACKGROUND_NODE_COUNT * 2) as u32
        || raw.render_callbacks != 1
    {
        return invalid("background did not request and evaluate its complete archive subtree");
    }
    if raw.pose_evaluated != 0 {
        return invalid("host runtime must not claim archive pose evaluation");
    }
    if !raw.requested_frame.is_finite() || !raw.selected_frame.is_finite() {
        return invalid("nonfinite animation frame");
    }
    if raw.requested_frame.to_bits() != 0 || raw.selected_frame != 1.0 {
        return invalid("background did not advance from requested frame zero to frame one");
    }
    Ok(())
}

#[cfg(skirmish_melee_source)]
fn validate_panel_plan(raw: &RawPanelPlan) -> Result<(), SourceRuntimeError> {
    let invalid = |message| Err(SourceRuntimeError::InvalidPlan(message));
    if raw.abi_version != SCENE_PLAN_ABI {
        return invalid("unsupported ABI version");
    }
    if raw.model != MODEL_MAIN_PANEL {
        return invalid("unexpected model identity");
    }
    if raw.flags != COMPLETE_SCENE_PLAN_FLAGS {
        return invalid("source panel constructor did not complete every traced host operation");
    }
    if (raw.classifier, raw.process_link, raw.process_priority) != (5, 6, 0x80) {
        return invalid("unexpected panel GObj scheduling fields");
    }
    if (raw.object_kind, raw.gx_link, raw.render_priority) != (2, 3, 0x80) {
        return invalid("unexpected panel JObj/GX attachment fields");
    }
    if raw.root_offset != MAIN_PANEL_ROOT_OFFSET
        || raw.background_node_offset != MAIN_PANEL_BACKGROUND_OFFSET
        || raw.panel_node_offset != MAIN_PANEL_ANIMATION_OFFSET
        || raw.node_count != MAIN_PANEL_NODE_COUNT as u32
    {
        return invalid("panel runtime did not retain the pinned archive topology");
    }
    if raw.frame_requested_nodes != 102
        || raw.evaluated_nodes != 102
        || raw.evaluation_requests != 5_304
    {
        return invalid("panel did not request and evaluate both pinned archive subtrees");
    }
    if raw.scheduler_slot != 0 || raw.transition_callbacks != 51 {
        return invalid("panel transition did not use the expected tree and callback timing");
    }
    if raw.render_callbacks != 1 || raw.user_data_released != 1 {
        return invalid(
            "panel did not invoke one render callback and release its source user data",
        );
    }
    if raw.pose_evaluated != 0 {
        return invalid("host runtime must not claim archive pose evaluation");
    }
    let frames = [
        raw.initial_background_frame,
        raw.initial_panel_frame,
        raw.versus_transition_frame,
        raw.versus_idle_frame,
    ];
    if !frames.iter().all(|frame| frame.is_finite()) {
        return invalid("nonfinite panel animation frame");
    }
    if frames != [0.0, 0.0, 400.0, 500.0] {
        return invalid("panel did not follow the original main-to-VS frame sequence");
    }
    Ok(())
}

/// Execute the original `mn_8022C010` menu-light selection helper.
pub fn light_color_index(menu_kind: u8, selection: u16) -> Option<i32> {
    // MENU_KIND_34 is a sentinel/unused entry. The original function has no
    // return after its switch, so crossing the FFI boundary with 34+ would be
    // undefined behavior rather than a meaningful source result.
    if menu_kind > 33 {
        return None;
    }
    #[cfg(skirmish_melee_source)]
    {
        // SAFETY: every value through 33 has an explicit return in the pinned
        // source, `selection` widens losslessly to C `int`, and the result is a
        // scalar value.
        Some(unsafe { skirmish_mn_light_color_index(menu_kind, selection) })
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = (menu_kind, selection);
        None
    }
}

/// Execute the original `mn_GetDigitAt` helper for a nonnegative decimal digit.
pub fn digit_at(number: i32, digit: i32) -> Option<i32> {
    if number < 0 || !(0..=9).contains(&digit) {
        return None;
    }
    #[cfg(skirmish_melee_source)]
    {
        // SAFETY: the guard excludes negative exponents and the values use the
        // exact fixed-width integer ABI declared by the C bridge.
        Some(unsafe { skirmish_mn_digit_at(number, digit) })
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = (number, digit);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn background_topology() -> Vec<TopologyNode> {
        (0..102)
            .map(|index| TopologyNode {
                source_offset: 26_664 + index as u32,
                parent_index: if index == 0 { -1 } else { 0 },
            })
            .collect()
    }

    fn panel_topology() -> Vec<TopologyNode> {
        (0..106)
            .map(|index| TopologyNode {
                source_offset: match index {
                    4 => 140_840,
                    41 => 143_976,
                    _ => 140_584 + index as u32,
                },
                parent_index: match index {
                    0 => -1,
                    1..=4 | 41 => 0,
                    5..=40 => 4,
                    _ => 41,
                },
            })
            .collect()
    }

    #[test]
    fn availability_matches_callable_source() {
        assert_eq!(light_color_index(0, 0).is_some(), available());
        assert_eq!(digit_at(123, 1).is_some(), available());
        assert_eq!(
            background_plan(&background_topology()).unwrap().is_some(),
            available()
        );
        assert_eq!(
            panel_plan(&panel_topology()).unwrap().is_some(),
            available()
        );
    }

    #[cfg(skirmish_melee_source)]
    #[test]
    fn exact_pinned_source_executes_on_the_host() {
        assert_eq!(light_color_index(0, 4), Some(4));
        assert_eq!(light_color_index(2, 0), Some(1));
        assert_eq!(digit_at(12_345, 2), Some(3));
        assert_eq!(
            background_plan(&background_topology()).unwrap(),
            Some(BackgroundPlan {
                model: Model::MainBackground,
                classifier: 4,
                process_link: 5,
                process_priority: 0x80,
                object_kind: 2,
                gx_link: 2,
                render_priority: 0x80,
                scheduler_slot: 0,
                root_offset: 26_664,
                node_count: 102,
                frame_requested_nodes: 102,
                evaluated_nodes: 102,
                process_callbacks: 1,
                render_callbacks: 1,
                evaluation_requests: 204,
                requested_frame: 0.0,
                selected_frame: 1.0,
                pose_evaluated: false,
            })
        );
        assert_eq!(
            panel_plan(&panel_topology()).unwrap(),
            Some(PanelPlan {
                model: Model::MainPanel,
                classifier: 5,
                process_link: 6,
                process_priority: 0x80,
                object_kind: 2,
                gx_link: 3,
                render_priority: 0x80,
                scheduler_slot: 0,
                root_offset: 140_584,
                background_node_offset: 140_840,
                panel_node_offset: 143_976,
                node_count: 106,
                frame_requested_nodes: 102,
                evaluated_nodes: 102,
                evaluation_requests: 5_304,
                transition_callbacks: 51,
                render_callbacks: 1,
                user_data_released: 1,
                initial_background_frame: 0.0,
                initial_panel_frame: 0.0,
                versus_transition_frame: 400.0,
                versus_idle_frame: 500.0,
                pose_evaluated: false,
            })
        );
    }

    #[cfg(skirmish_melee_source)]
    #[test]
    fn source_scene_runtime_is_serialized_without_leaking_state() {
        let workers: Vec<_> = (0..8)
            .map(|index| {
                std::thread::spawn(move || {
                    if index % 2 == 0 {
                        background_plan(&background_topology())
                            .unwrap()
                            .unwrap()
                            .render_callbacks
                    } else {
                        panel_plan(&panel_topology())
                            .unwrap()
                            .unwrap()
                            .render_callbacks
                    }
                })
            })
            .collect();
        for worker in workers {
            assert_eq!(worker.join().unwrap(), 1);
        }
    }

    #[cfg(skirmish_melee_source)]
    #[test]
    fn malformed_or_wrong_archive_topology_is_rejected_before_ffi() {
        let mut background = background_topology();
        background[2].parent_index = 1;
        background[3].parent_index = 0;
        background[4].parent_index = 2;
        assert_eq!(
            background_plan(&background),
            Err(SourceRuntimeError::InvalidPlan(
                "topology is not in depth-first preorder"
            ))
        );
        let mut panel = panel_topology();
        panel[41].source_offset += 1;
        assert_eq!(
            panel_plan(&panel),
            Err(SourceRuntimeError::InvalidPlan(
                "panel preorder identities do not match the pinned archive"
            ))
        );
    }

    #[test]
    fn invalid_digit_requests_do_not_cross_the_ffi_boundary() {
        assert_eq!(digit_at(123, -1), None);
        assert_eq!(digit_at(-123, 1), None);
        assert_eq!(digit_at(123, 10), None);
    }

    #[test]
    fn invalid_menu_kinds_do_not_cross_the_ffi_boundary() {
        assert_eq!(light_color_index(34, 0), None);
        assert_eq!(light_color_index(u8::MAX, 0), None);
    }
}
