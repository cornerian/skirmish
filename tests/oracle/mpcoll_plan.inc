/* Count/delta selection only; excludes callbacks and world mutations. */
static void oracle_ecb_plan(CollData* coll, float* extra) {
/* BEGIN VERBATIM SUBSTEP PLAN */
    Vec3 vel;

    u8 _[4];

    float dist_right_x;
    float dist_right_y;
    float x;
    float y;
    float dist_left_x;
    float dist_top_y;
    s32 step;  // r31
    s32 steps; // r30
    bool ret;

    lbVector_Diff(&coll->cur_pos, &coll->last_pos, &vel);
    x = ABS(vel.x);
    y = ABS(vel.y);

    dist_left_x = coll->desired_ecb.left.x - coll->ecb.left.x;
    // if (dist_left_x < 0) dist_left_x = -dist_left_x;
    dist_left_x = ABS(dist_left_x);

    dist_right_x = coll->desired_ecb.right.x - coll->ecb.right.x;
    if (dist_right_x < 0) {
        dist_right_x = -dist_right_x;
    }

    if (dist_left_x < dist_right_x) {
        dist_left_x = dist_right_x;
    }

    dist_top_y = coll->desired_ecb.top.y - coll->ecb.top.y;
    // if (dist_top_y < 0) dist_top_y = -dist_top_y;
    dist_top_y = ABS(dist_top_y);

    dist_right_y = coll->desired_ecb.right.y - coll->ecb.right.y;
    if (dist_right_y < 0) {
        dist_right_y = -dist_right_y;
    }

    if (dist_top_y < dist_right_y) {
        dist_top_y = dist_right_y;
    }

    x = max_inline(x, dist_left_x);
    y = max_inline(y, dist_top_y);
    x = max_inline(x, y);

    if (x > 6.0F) {
        steps = x / 6.0F;
        steps = steps + 1;
        vel.x /= steps;
        vel.y /= steps;
        vel.z /= steps;
    } else {
        steps = 1;
    }
/* END VERBATIM SUBSTEP PLAN */
    extra[0]=steps; extra[1]=vel.x; extra[2]=vel.y; extra[3]=vel.z;
}
