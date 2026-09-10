#ifndef SKIRMISH_MELEE_UI_SCENE_RUNTIME_H
#define SKIRMISH_MELEE_UI_SCENE_RUNTIME_H

#include <stdint.h>

enum {
    SKIRMISH_MN_SCENE_PLAN_ABI = 1,
    SKIRMISH_MN_MODEL_MAIN_BACKGROUND = 1,
    SKIRMISH_MN_MODEL_MAIN_PANEL = 2,
};

enum {
    SKIRMISH_MN_PLAN_OBJECT_ATTACHED = 1U << 0,
    SKIRMISH_MN_PLAN_GX_LINK_CONFIGURED = 1U << 1,
    SKIRMISH_MN_PLAN_PROCESS_CONFIGURED = 1U << 2,
    SKIRMISH_MN_PLAN_JOINT_ANIM_ATTACHED = 1U << 3,
    SKIRMISH_MN_PLAN_MATERIAL_ANIM_ATTACHED = 1U << 4,
    SKIRMISH_MN_PLAN_SHAPE_ANIM_ATTACHED = 1U << 5,
    SKIRMISH_MN_PLAN_FRAME_REQUESTED = 1U << 6,
    SKIRMISH_MN_PLAN_EVALUATION_REQUESTED = 1U << 7,
};

typedef struct SkirmishMnTopologyNode {
    uint32_t source_offset;
    int32_t parent_index;
} SkirmishMnTopologyNode;

/* Fixed-width snapshot only. Pointer-rich HSD objects remain owned by C. */
typedef struct SkirmishMnBackgroundPlan {
    uint32_t abi_version;
    uint32_t model;
    uint32_t flags;
    uint32_t classifier;
    uint32_t process_link;
    uint32_t process_priority;
    uint32_t object_kind;
    uint32_t gx_link;
    uint32_t render_priority;
    uint32_t scheduler_slot;
    uint32_t root_offset;
    uint32_t node_count;
    uint32_t frame_requested_nodes;
    uint32_t evaluated_nodes;
    uint32_t process_callbacks;
    uint32_t render_callbacks;
    uint32_t evaluation_requests;
    uint32_t pose_evaluated;
    float requested_frame;
    float selected_frame;
} SkirmishMnBackgroundPlan;

typedef struct SkirmishMnPanelPlan {
    uint32_t abi_version;
    uint32_t model;
    uint32_t flags;
    uint32_t classifier;
    uint32_t process_link;
    uint32_t process_priority;
    uint32_t object_kind;
    uint32_t gx_link;
    uint32_t render_priority;
    uint32_t scheduler_slot;
    uint32_t root_offset;
    uint32_t background_node_offset;
    uint32_t panel_node_offset;
    uint32_t node_count;
    uint32_t frame_requested_nodes;
    uint32_t evaluated_nodes;
    uint32_t evaluation_requests;
    uint32_t transition_callbacks;
    uint32_t render_callbacks;
    uint32_t user_data_released;
    uint32_t pose_evaluated;
    float initial_background_frame;
    float initial_panel_frame;
    float versus_transition_frame;
    float versus_idle_frame;
} SkirmishMnPanelPlan;

int32_t skirmish_mn_build_background_plan(
    const SkirmishMnTopologyNode* topology, uint32_t node_count,
    SkirmishMnBackgroundPlan* out);
int32_t skirmish_mn_build_panel_plan(
    const SkirmishMnTopologyNode* topology, uint32_t node_count,
    SkirmishMnPanelPlan* out);

#endif
