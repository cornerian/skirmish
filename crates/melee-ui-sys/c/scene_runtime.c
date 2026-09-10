#include "scene_runtime.h"

#include <setjmp.h>
#include <string.h>

#include <melee/mn/mnmain.h>
#include <sysdolphin/baselib/aobj.h>
#include <sysdolphin/baselib/gobj.h>
#include <sysdolphin/baselib/gobjgxlink.h>
#include <sysdolphin/baselib/gobjobject.h>
#include <sysdolphin/baselib/gobjproc.h>
#include <sysdolphin/baselib/gobjuserdata.h>
#include <sysdolphin/baselib/jobj.h>
#include <sysdolphin/baselib/memory.h>
#include <sysdolphin/baselib/mobj.h>
#include <sysdolphin/baselib/pobj.h>

void* calloc(size_t count, size_t size);
void free(void* pointer);

enum HostModel {
    HOST_MODEL_NONE,
    HOST_MODEL_BACKGROUND,
    HOST_MODEL_PANEL,
};

typedef struct HostJObj {
    HSD_JObj base;
    HSD_AObj clock;
    struct HostJObj* last_child;
    uint32_t source_offset;
    HSD_AnimJoint* joint_anim;
    HSD_MatAnimJoint* material_anim;
    HSD_ShapeAnimJoint* shape_anim;
    float requested_frame;
    uint32_t frame_requested;
    uint32_t evaluation_requests;
} HostJObj;

typedef struct HostRuntime {
    HSD_GObj* gobj;
    HSD_GObjProc* process;
    HostJObj* nodes;
    const SkirmishMnTopologyNode* topology;
    void* user_data_allocation;
    uint32_t node_count;
    uint32_t render_callbacks;
    uint32_t process_callbacks;
    uint32_t user_data_released;
    int32_t error;
    enum HostModel model;
    uint32_t active;
} HostRuntime;

static HostRuntime runtime;
static jmp_buf runtime_abort;
static uint32_t runtime_abort_ready;
static HSD_Joint back_joint_token;
static HSD_AnimJoint back_joint_anim_token;
static HSD_MatAnimJoint back_material_anim_token;
static HSD_ShapeAnimJoint back_shape_anim_token;
static HSD_Joint panel_joint_token;
static HSD_AnimJoint panel_joint_anim_token;
static HSD_MatAnimJoint panel_material_anim_token;
static HSD_ShapeAnimJoint panel_shape_anim_token;

/* The original engine registers JObj as object kind two during GObj startup. */
u8 HSD_GObj_JObjKind = 2;

static void fail_runtime(int32_t code)
{
    if (runtime.error == 0) {
        runtime.error = code;
    }
    if (runtime_abort_ready) {
        longjmp(runtime_abort, 1);
    }
}

static HostJObj* host_jobj(HSD_JObj* jobj)
{
    uint32_t i;
    if (!runtime.active || runtime.nodes == NULL || jobj == NULL) {
        fail_runtime(-20);
        return NULL;
    }
    for (i = 0; i < runtime.node_count; i++) {
        if (jobj == &runtime.nodes[i].base) {
            return &runtime.nodes[i];
        }
    }
    fail_runtime(-20);
    return NULL;
}

static HSD_Joint* expected_joint(void)
{
    switch (runtime.model) {
    case HOST_MODEL_BACKGROUND:
        return MenMainBack_Top.joint;
    case HOST_MODEL_PANEL:
        return MenMainPanel_Top.joint;
    default:
        return NULL;
    }
}

static void attach_subtree(HostJObj* node, HSD_AnimJoint* joint_anim,
                           HSD_MatAnimJoint* material_anim,
                           HSD_ShapeAnimJoint* shape_anim)
{
    HSD_JObj* child;
    node->joint_anim = joint_anim;
    node->material_anim = material_anim;
    node->shape_anim = shape_anim;
    for (child = node->base.child; child != NULL; child = child->next) {
        HostJObj* host = host_jobj(child);
        if (host != NULL) {
            attach_subtree(host, joint_anim, material_anim, shape_anim);
        }
    }
}

static void request_subtree(HostJObj* node, f32 frame)
{
    HSD_JObj* child;
    node->requested_frame = frame;
    node->frame_requested = 1;
    node->clock.curr_frame = frame;
    node->clock.flags &= ~AOBJ_NO_ANIM;
    node->clock.flags |= AOBJ_FIRST_PLAY;
    for (child = node->base.child; child != NULL; child = child->next) {
        HostJObj* host = host_jobj(child);
        if (host != NULL) {
            request_subtree(host, frame);
        }
    }
}

static void evaluate_subtree(HostJObj* node)
{
    HSD_JObj* child;
    if (!node->frame_requested) {
        fail_runtime(-31);
        return;
    }
    if (node->clock.flags & AOBJ_FIRST_PLAY) {
        node->clock.flags &= ~AOBJ_FIRST_PLAY;
    } else {
        node->clock.curr_frame += node->clock.framerate;
    }
    node->evaluation_requests += 1;
    for (child = node->base.child; child != NULL; child = child->next) {
        HostJObj* host = host_jobj(child);
        if (host != NULL) {
            evaluate_subtree(host);
        }
    }
}

static int32_t validate_topology(enum HostModel model,
                                 const SkirmishMnTopologyNode* topology,
                                 uint32_t node_count)
{
    uint32_t i;
    uint32_t j;
    uint32_t expected_root;
    uint32_t expected_count;

    if (topology == NULL || node_count == 0 || node_count > 4096 ||
        topology[0].parent_index != -1 || topology[0].source_offset == 0)
    {
        return -40;
    }
    expected_root = model == HOST_MODEL_BACKGROUND ? 26664U : 140584U;
    expected_count = model == HOST_MODEL_BACKGROUND ? 102U : 106U;
    if (topology[0].source_offset != expected_root ||
        node_count != expected_count)
    {
        return -41;
    }
    if (model == HOST_MODEL_PANEL &&
        (topology[4].source_offset != 140840U ||
         topology[41].source_offset != 143976U))
    {
        return -42;
    }
    for (i = 1; i < node_count; i++) {
        int32_t parent = topology[i].parent_index;
        int32_t cursor = (int32_t) i - 1;
        if (topology[i].source_offset == 0 || parent < 0 ||
            (uint32_t) parent >= i)
        {
            return -43;
        }
        while (cursor >= 0 && cursor != parent) {
            cursor = topology[cursor].parent_index;
        }
        if (cursor != parent) {
            return -44;
        }
        for (j = 0; j < i; j++) {
            if (topology[j].source_offset == topology[i].source_offset) {
                return -45;
            }
        }
    }
    return 0;
}

static int32_t begin_runtime(enum HostModel model,
                             const SkirmishMnTopologyNode* topology,
                             uint32_t node_count)
{
    int32_t status;
    if (runtime.active || model == HOST_MODEL_NONE) {
        return -1;
    }
    status = validate_topology(model, topology, node_count);
    if (status != 0) {
        return status;
    }
    memset(&runtime, 0, sizeof(runtime));
    runtime.active = 1;
    runtime.model = model;
    runtime.topology = topology;
    runtime.node_count = node_count;
    return 0;
}

static void cleanup_runtime(void)
{
    enum HostModel model = runtime.model;
    if (runtime.user_data_allocation != NULL) {
        free(runtime.user_data_allocation);
    }
    free(runtime.process);
    free(runtime.nodes);
    free(runtime.gobj);
    memset(&runtime, 0, sizeof(runtime));
    if (model == HOST_MODEL_BACKGROUND) {
        memset(&MenMainBack_Top, 0, sizeof(MenMainBack_Top));
    } else if (model == HOST_MODEL_PANEL) {
        memset(&MenMainPanel_Top, 0, sizeof(MenMainPanel_Top));
        memset(&mn_804A04F0, 0, sizeof(mn_804A04F0));
    }
}

HSD_GObj* GObj_Create(u16 classifier, u8 p_link, u8 priority)
{
    HSD_GObj* gobj;
    if (!runtime.active || runtime.gobj != NULL) {
        fail_runtime(-21);
        return NULL;
    }
    gobj = calloc(1, sizeof(*gobj));
    if (gobj == NULL) {
        fail_runtime(-22);
        return NULL;
    }
    gobj->classifier = classifier;
    gobj->p_link = p_link;
    gobj->p_priority = priority;
    gobj->gx_link = HSD_GOBJ_GXLINK_NONE;
    gobj->obj_kind = HSD_GOBJ_OBJ_NONE;
    gobj->user_data_kind = HSD_GOBJ_USER_DATA_NONE;
    runtime.gobj = gobj;
    return gobj;
}

HSD_JObj* HSD_JObjLoadJoint(HSD_Joint* joint)
{
    HostJObj* nodes;
    uint32_t i;
    if (!runtime.active || runtime.nodes != NULL || runtime.topology == NULL ||
        joint == NULL || joint != expected_joint())
    {
        fail_runtime(-23);
        return NULL;
    }
    nodes = calloc(runtime.node_count, sizeof(*nodes));
    if (nodes == NULL) {
        fail_runtime(-24);
        return NULL;
    }
    runtime.nodes = nodes;
    for (i = 0; i < runtime.node_count; i++) {
        nodes[i].source_offset = runtime.topology[i].source_offset;
        nodes[i].base.scale.x = 1.0F;
        nodes[i].base.scale.y = 1.0F;
        nodes[i].base.scale.z = 1.0F;
        nodes[i].base.aobj = &nodes[i].clock;
        nodes[i].clock.flags = AOBJ_NO_ANIM;
        nodes[i].clock.framerate = 1.0F;
    }
    for (i = 1; i < runtime.node_count; i++) {
        HostJObj* parent = &nodes[runtime.topology[i].parent_index];
        nodes[i].base.parent = &parent->base;
        if (parent->base.child == NULL) {
            parent->base.child = &nodes[i].base;
        } else {
            parent->last_child->base.next = &nodes[i].base;
        }
        parent->last_child = &nodes[i];
    }
    return &nodes[0].base;
}

void HSD_GObjObject_80390A70(HSD_GObj* gobj, u8 kind, void* object)
{
    if (!runtime.active || gobj == NULL || gobj != runtime.gobj ||
        runtime.nodes == NULL || object != &runtime.nodes[0].base ||
        gobj->obj_kind != HSD_GOBJ_OBJ_NONE)
    {
        fail_runtime(-25);
        return;
    }
    gobj->obj_kind = kind;
    gobj->hsd_obj = object;
}

void GObj_SetupGXLink(HSD_GObj* gobj, GObj_RenderFunc render_cb, u8 gx_link,
                      u32 priority)
{
    if (!runtime.active || gobj == NULL || gobj != runtime.gobj ||
        render_cb == NULL || gobj->gx_link != HSD_GOBJ_GXLINK_NONE)
    {
        fail_runtime(-26);
        return;
    }
    gobj->render_cb = render_cb;
    gobj->gx_link = gx_link;
    gobj->render_priority = priority;
}

HSD_GObjProc* HSD_GObj_SetupProc(HSD_GObj* gobj, HSD_GObjEvent callback,
                                 u8 priority)
{
    HSD_GObjProc* process;
    if (!runtime.active || gobj == NULL || gobj != runtime.gobj ||
        callback == NULL || runtime.process != NULL)
    {
        fail_runtime(-27);
        return NULL;
    }
    process = calloc(1, sizeof(*process));
    if (process == NULL) {
        fail_runtime(-28);
        return NULL;
    }
    process->gobj = gobj;
    process->on_invoke = callback;
    process->s_link = priority;
    gobj->proc = process;
    runtime.process = process;
    return process;
}

void HSD_JObjAddAnimAll(HSD_JObj* jobj, HSD_AnimJoint* joint_anim,
                        HSD_MatAnimJoint* material_anim,
                        HSD_ShapeAnimJoint* shape_anim)
{
    HostJObj* host = host_jobj(jobj);
    if (host == NULL || joint_anim == NULL || material_anim == NULL ||
        shape_anim == NULL)
    {
        fail_runtime(-29);
        return;
    }
    attach_subtree(host, joint_anim, material_anim, shape_anim);
}

void HSD_JObjReqAnimAll(HSD_JObj* jobj, f32 frame)
{
    HostJObj* host = host_jobj(jobj);
    if (host == NULL || host->joint_anim == NULL ||
        host->material_anim == NULL || host->shape_anim == NULL)
    {
        fail_runtime(-30);
        return;
    }
    request_subtree(host, frame);
}

void HSD_JObjAnimAll(HSD_JObj* jobj)
{
    HostJObj* host = host_jobj(jobj);
    if (host != NULL) {
        /* Advance the exact HSD frame clock. A companion runtime manifest must
         * still evaluate the descriptor's SRT/material/shape tracks, so plans
         * explicitly keep pose_evaluated false. */
        evaluate_subtree(host);
    }
}

void HSD_GObj_JObjCallback(HSD_GObj* gobj, int pass)
{
    (void) pass;
    if (!runtime.active || gobj == NULL || gobj != runtime.gobj ||
        runtime.nodes == NULL || gobj->obj_kind != HSD_GObj_JObjKind ||
        gobj->hsd_obj != &runtime.nodes[0].base)
    {
        fail_runtime(-32);
        return;
    }
    runtime.render_callbacks += 1;
}

void* HSD_MemAlloc(ssize_t size)
{
    void* result;
    if (!runtime.active || size <= 0 || runtime.user_data_allocation != NULL) {
        fail_runtime(-33);
        return NULL;
    }
    result = calloc(1, (size_t) size);
    if (result == NULL) {
        fail_runtime(-34);
        return NULL;
    }
    runtime.user_data_allocation = result;
    return result;
}

void HSD_Free(void* pointer)
{
    if (!runtime.active || pointer == NULL ||
        pointer != runtime.user_data_allocation)
    {
        fail_runtime(-35);
        return;
    }
    free(pointer);
    runtime.user_data_allocation = NULL;
    runtime.user_data_released += 1;
}

void GObj_InitUserData(HSD_GObj* gobj, u8 kind,
                       void (*remove_func)(void*), void* data)
{
    if (!runtime.active || gobj == NULL || gobj != runtime.gobj ||
        data == NULL || data != runtime.user_data_allocation ||
        remove_func == NULL || gobj->user_data != NULL)
    {
        fail_runtime(-36);
        return;
    }
    gobj->user_data_kind = kind;
    gobj->user_data_remove_func = remove_func;
    gobj->user_data = data;
}

void OSReport(char* format, ...)
{
    (void) format;
    fail_runtime(-37);
}

void __assert(char* file, u32 line, char* message)
{
    (void) file;
    (void) line;
    (void) message;
    fail_runtime(-38);
    __builtin_trap();
}

static void configure_background(void)
{
    MenMainBack_Top.joint = &back_joint_token;
    MenMainBack_Top.animjoint = &back_joint_anim_token;
    MenMainBack_Top.matanim_joint = &back_material_anim_token;
    MenMainBack_Top.shapeanim_joint = &back_shape_anim_token;
}

static void configure_panel(void)
{
    MenMainPanel_Top.joint = &panel_joint_token;
    MenMainPanel_Top.animjoint = &panel_joint_anim_token;
    MenMainPanel_Top.matanim_joint = &panel_material_anim_token;
    MenMainPanel_Top.shapeanim_joint = &panel_shape_anim_token;
}

static uint32_t common_plan_flags(HSD_GObj* gobj, HSD_GObjProc* process,
                                  HostJObj* root, HSD_GObjEvent callback,
                                  StaticModelDesc* model)
{
    uint32_t flags = 0;
    uint32_t i;
    uint32_t frame_requested = 0;
    uint32_t evaluation_requested = 0;
    uint32_t joint_animation_attached = 1;
    uint32_t material_animation_attached = 1;
    uint32_t shape_animation_attached = 1;
    for (i = 0; i < runtime.node_count; i++) {
        frame_requested |= runtime.nodes[i].frame_requested;
        evaluation_requested |= runtime.nodes[i].evaluation_requests;
        joint_animation_attached &=
            runtime.nodes[i].joint_anim == model->animjoint;
        material_animation_attached &=
            runtime.nodes[i].material_anim == model->matanim_joint;
        shape_animation_attached &=
            runtime.nodes[i].shape_anim == model->shapeanim_joint;
    }
    if (gobj->hsd_obj == &root->base && gobj->obj_kind == HSD_GObj_JObjKind) {
        flags |= SKIRMISH_MN_PLAN_OBJECT_ATTACHED;
    }
    if (gobj->gx_link != HSD_GOBJ_GXLINK_NONE &&
        gobj->render_cb == HSD_GObj_JObjCallback)
    {
        flags |= SKIRMISH_MN_PLAN_GX_LINK_CONFIGURED;
    }
    if (gobj->proc == process && process->on_invoke == callback) {
        flags |= SKIRMISH_MN_PLAN_PROCESS_CONFIGURED;
    }
    if (joint_animation_attached != 0) {
        flags |= SKIRMISH_MN_PLAN_JOINT_ANIM_ATTACHED;
    }
    if (material_animation_attached != 0) {
        flags |= SKIRMISH_MN_PLAN_MATERIAL_ANIM_ATTACHED;
    }
    if (shape_animation_attached != 0) {
        flags |= SKIRMISH_MN_PLAN_SHAPE_ANIM_ATTACHED;
    }
    if (frame_requested != 0) {
        flags |= SKIRMISH_MN_PLAN_FRAME_REQUESTED;
    }
    if (evaluation_requested != 0) {
        flags |= SKIRMISH_MN_PLAN_EVALUATION_REQUESTED;
    }
    return flags;
}

static void animation_stats(uint32_t* requested_nodes,
                            uint32_t* evaluated_nodes,
                            uint32_t* evaluation_requests)
{
    uint32_t i;
    *requested_nodes = 0;
    *evaluated_nodes = 0;
    *evaluation_requests = 0;
    for (i = 0; i < runtime.node_count; i++) {
        *requested_nodes += runtime.nodes[i].frame_requested != 0;
        *evaluated_nodes += runtime.nodes[i].evaluation_requests != 0;
        *evaluation_requests += runtime.nodes[i].evaluation_requests;
    }
}

int32_t skirmish_mn_build_background_plan(
    const SkirmishMnTopologyNode* topology, uint32_t node_count,
    SkirmishMnBackgroundPlan* out)
{
    HSD_GObj* result;
    HostJObj* root;
    HSD_GObj* gobj;
    HSD_GObjProc* process;
    int32_t status;

    if (out == NULL) {
        return -1;
    }
    status = begin_runtime(HOST_MODEL_BACKGROUND, topology, node_count);
    if (status != 0) {
        return status;
    }
    if (setjmp(runtime_abort) != 0) {
        status = runtime.error != 0 ? runtime.error : -38;
        runtime_abort_ready = 0;
        cleanup_runtime();
        return status;
    }
    runtime_abort_ready = 1;
    memset(out, 0, sizeof(*out));
    configure_background();
    result = mn_80229B2C();
    gobj = runtime.gobj;
    root = runtime.nodes;
    process = runtime.process;
    if (runtime.error == 0 && result != gobj) {
        fail_runtime(-2);
    }
    if (runtime.error == 0 && process != NULL) {
        process->on_invoke(gobj);
        runtime.process_callbacks += 1;
    }
    if (runtime.error == 0 && gobj != NULL && gobj->render_cb != NULL) {
        gobj->render_cb(gobj, 0);
    }
    if (runtime.error == 0 && gobj != NULL && root != NULL && process != NULL) {
        out->abi_version = SKIRMISH_MN_SCENE_PLAN_ABI;
        out->model = SKIRMISH_MN_MODEL_MAIN_BACKGROUND;
        out->flags = common_plan_flags(gobj, process, root, mn_8022EAE0,
                                       &MenMainBack_Top);
        out->classifier = gobj->classifier;
        out->process_link = gobj->p_link;
        out->process_priority = gobj->p_priority;
        out->object_kind = gobj->obj_kind;
        out->gx_link = gobj->gx_link;
        out->render_priority = gobj->render_priority;
        out->scheduler_slot = process->s_link;
        out->root_offset = root->source_offset;
        out->node_count = runtime.node_count;
        animation_stats(&out->frame_requested_nodes, &out->evaluated_nodes,
                        &out->evaluation_requests);
        out->process_callbacks = runtime.process_callbacks;
        out->render_callbacks = runtime.render_callbacks;
        out->pose_evaluated = 0;
        out->requested_frame = root->requested_frame;
        out->selected_frame = root->clock.curr_frame;
    }
    status = runtime.error;
    runtime_abort_ready = 0;
    cleanup_runtime();
    return status;
}

int32_t skirmish_mn_build_panel_plan(
    const SkirmishMnTopologyNode* topology, uint32_t node_count,
    SkirmishMnPanelPlan* out)
{
    HSD_GObj* result;
    HSD_GObj* gobj;
    HSD_GObjProc* process;
    MainMenuPanelData* data;
    float initial_background_frame = 0.0F;
    float initial_panel_frame = 0.0F;
    float transition_frame = 0.0F;
    float idle_frame = 0.0F;
    int32_t status;

    if (out == NULL) {
        return -1;
    }
    status = begin_runtime(HOST_MODEL_PANEL, topology, node_count);
    if (status != 0) {
        return status;
    }
    if (setjmp(runtime_abort) != 0) {
        status = runtime.error != 0 ? runtime.error : -38;
        runtime_abort_ready = 0;
        cleanup_runtime();
        return status;
    }
    runtime_abort_ready = 1;
    memset(out, 0, sizeof(*out));
    configure_panel();
    memset(&mn_804A04F0, 0, sizeof(mn_804A04F0));
    mn_804A04F0.cur_menu = MENU_KIND_MAIN;
    mn_804A04F0.prev_menu = MENU_KIND_MAIN;
    mn_804A04F0.entering_menu = 1;

    result = mn_80229DC0();
    gobj = runtime.gobj;
    process = runtime.process;
    if (runtime.error == 0 && result != gobj) {
        fail_runtime(-3);
    }
    if (runtime.error == 0 && gobj != NULL && process != NULL &&
        runtime.nodes != NULL && gobj->user_data != NULL)
    {
        initial_background_frame = runtime.nodes[4].clock.curr_frame;
        initial_panel_frame = runtime.nodes[41].clock.curr_frame;
        data = gobj->user_data;
        mn_804A04F0.cur_menu = MENU_KIND_VS;
        mn_804A04F0.entering_menu = 1;
        process->on_invoke(gobj);
        runtime.process_callbacks += 1;
        transition_frame = runtime.nodes[41].clock.curr_frame;
        while (runtime.error == 0 && data->state != 2 &&
               runtime.process_callbacks < 64)
        {
            process->on_invoke(gobj);
            runtime.process_callbacks += 1;
        }
        if (data->state != 2) {
            fail_runtime(-4);
        } else {
            idle_frame = runtime.nodes[41].clock.curr_frame;
        }
    }
    if (runtime.error == 0 && gobj != NULL && gobj->render_cb != NULL) {
        gobj->render_cb(gobj, 0);
    }
    if (runtime.error == 0 && gobj != NULL && process != NULL &&
        runtime.nodes != NULL)
    {
        out->abi_version = SKIRMISH_MN_SCENE_PLAN_ABI;
        out->model = SKIRMISH_MN_MODEL_MAIN_PANEL;
        out->flags = common_plan_flags(gobj, process, &runtime.nodes[0],
                                       fn_80229BF4, &MenMainPanel_Top);
        out->classifier = gobj->classifier;
        out->process_link = gobj->p_link;
        out->process_priority = gobj->p_priority;
        out->object_kind = gobj->obj_kind;
        out->gx_link = gobj->gx_link;
        out->render_priority = gobj->render_priority;
        out->scheduler_slot = process->s_link;
        out->root_offset = runtime.nodes[0].source_offset;
        out->background_node_offset = runtime.nodes[4].source_offset;
        out->panel_node_offset = runtime.nodes[41].source_offset;
        out->node_count = runtime.node_count;
        animation_stats(&out->frame_requested_nodes, &out->evaluated_nodes,
                        &out->evaluation_requests);
        out->transition_callbacks = runtime.process_callbacks;
        out->render_callbacks = runtime.render_callbacks;
        out->pose_evaluated = 0;
        out->initial_background_frame = initial_background_frame;
        out->initial_panel_frame = initial_panel_frame;
        out->versus_transition_frame = transition_frame;
        out->versus_idle_frame = idle_frame;
        if (gobj->user_data != NULL && gobj->user_data_remove_func != NULL) {
            gobj->user_data_remove_func(gobj->user_data);
            gobj->user_data = NULL;
        }
        out->user_data_released = runtime.user_data_released;
    }
    status = runtime.error;
    runtime_abort_ready = 0;
    cleanup_runtime();
    return status;
}
