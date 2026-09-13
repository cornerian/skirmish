-- Falco fighter behavior. Reflector shares Fox's source callbacks and policy
-- in the pinned game; it has its own bundled identity for future divergence.
function on_projectile_contact(fighter, ctx)
    local hit = ctx
    if hit and hit.projectile and fighter.flags.reflecting
        and hit.damage <= hit.max_damage then
        ctx.reflect = true
    end
end
