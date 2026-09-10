/* Host oracle for fighter-contact and captured-victim damage direction. */
typedef struct { float x, y, z; } Vec3;
typedef struct { float facing_dir_1; } Damage;
typedef struct Fighter {
    Vec3 cur_pos;
    float facing_dir;
    Damage dmg;
} Fighter;

float oracle_fighter_hit_direction(float victim_x, float attacker_x)
{
    Fighter victim = { .cur_pos.x = victim_x };
    Fighter attacker = { .cur_pos.x = attacker_x };
    Fighter* fp = &victim;
    Fighter* attacker_fp = &attacker;
    float dir;

/* BEGIN VERBATIM FIGHTER DIRECTION */
        dir = (fp->cur_pos.x > attacker_fp->cur_pos.x) ? -1.0F : 1.0F;
/* END VERBATIM FIGHTER DIRECTION */
    return dir;
}

float oracle_throw_hit_direction(float attacker_facing)
{
    Fighter attacker = { .facing_dir = attacker_facing };
    Fighter victim = { 0 };
    Fighter* fp = &attacker;
    Fighter* fp2 = &victim;

/* BEGIN VERBATIM THROW DIRECTION */
    fp2->dmg.facing_dir_1 = -(fp->facing_dir);
/* END VERBATIM THROW DIRECTION */
    return fp2->dmg.facing_dir_1;
}
