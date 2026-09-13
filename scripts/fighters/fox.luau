-- Fox fighter behavior. The first migrated policy is Reflector's projectile
-- eligibility; collision and reflection physics remain engine mechanisms.
function on_projectile_contact(fighter, ctx)
    local hit = ctx
    if hit and hit.projectile and fighter.flags.reflecting
        and hit.damage <= hit.max_damage then
        ctx.reflect = true
    end
end
