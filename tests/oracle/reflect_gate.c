/* Host oracle for the Reflector's own damage-eligibility gate
 * (`ftColl_80077464`, already pinned whole-file as `combat_knockback.c` --
 * see that adapter's own sibling aliases, `damage_angle_range`/
 * `shield_environment`/`clank_response`). A verbatim excerpt (like
 * `hit_direction.c`'s own pattern), not a full extraction: the surrounding
 * branches touch `ReflectAttr`/hit-direction bookkeeping this port has no
 * equivalent for at all (see `docs/fox-neutral-special.md`), so only the
 * damage-derivation idiom and the exact `>` eligibility comparison are
 * reproduced, using the pinned source's own exact variable names so the
 * excerpt can be checked verbatim against the pinned snapshot by
 * `tests/reflect_gate_differential.rs`'s own
 * `adapter_statements_are_verbatim_in_the_pinned_sources` test. */
typedef int s32;
typedef float f32;
typedef struct {
    f32 damage;
} HitCapsule;
typedef struct {
    struct {
        s32 x1A30_maxDamage;
    } ReflectAttr;
} Fighter;

int oracle_reflect_eligible(f32 hit_damage_in, s32 max_damage_in, s32* out_damage)
{
    HitCapsule hit_storage;
    hit_storage.damage = hit_damage_in;
    HitCapsule* hit = &hit_storage;
    Fighter fp_storage;
    fp_storage.ReflectAttr.x1A30_maxDamage = max_damage_in;
    Fighter* fp = &fp_storage;
    s32 damage;

/* BEGIN VERBATIM REFLECT DAMAGE AND GATE */
    if (hit->damage) {
        if ((s32) hit->damage) {
            damage = hit->damage;
        } else {
            damage = 1;
        }
    } else {
        damage = 0;
    }

    if (damage > fp->ReflectAttr.x1A30_maxDamage) {
/* END VERBATIM REFLECT DAMAGE AND GATE */
        *out_damage = damage;
        return 0;
    }
    *out_damage = damage;
    return 1;
}
