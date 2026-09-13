# Luau fighter behavior

Each fighter will eventually own one maintainable source file under
`scripts/fighters/`, with a target size of roughly 500–1,000 lines for the
complete fighter implementation rather than one file per move. The current
files are `fox.luau` and `falco.luau`; only the Reflector projectile eligibility
policy has migrated so far.

`FighterData.script` may provide an explicit program. When it does not define
the projectile hook, a Fox or Falco resource with a down special uses its
bundled fighter file automatically. The selected source and ABI marker are
included in `Match::resource_id`, so checkpoints and replays cannot silently
mix script revisions.

The supported hooks are `on_frame`, `before_hit`, `before_receive_hit`,
`after_hit`, `after_receive_hit`, and `on_projectile_contact`. Each receives a
fighter table and a mutable context table. Fighter state currently includes
the numeric id, action name, action frame, velocity, grounded state, percent,
hitlag, hitstun, and boolean flags. Hit contexts include frame, attacker and
defender ids, damage, angle, base knockback, knockback growth, computed
knockback, hitbox group, projectile status, and the projectile's maximum
damage gate.

The migrated Reflector policy is deliberately only a disposition decision:

```lua
function on_projectile_contact(fighter, ctx)
    if ctx.projectile and fighter.flags.reflecting
        and ctx.damage <= ctx.max_damage then
        ctx.reflect = true
    end
end
```

The comparison is inclusive. Rust still performs swept collision, bone
geometry, owner swapping, angle changes, per-hitbox damage scaling, event
ordering, lifetime handling, and ordinary shield fallback. The script does
not implement those mechanisms and does not apply Fox's unused `speed_mul`.

Hit hooks can update the mutable context and use the fighter command table:

```lua
function before_receive_hit(self, ctx)
    if self.locals.ignore_small_hits and ctx.knockback < 10 then
        ctx.cancelled = true
        self:set_action("wait")
        self:set_velocity(0, 0)
        table.insert(self.commands, {
            kind = "apply_hitlag", fighter = self.id, frames = 6,
        })
    end
end
```

Changing `ctx.damage` does not implicitly recompute knockback. Changing
`ctx.knockback` causes the native resolver to recompute launch and hitstun;
`ctx.apply_hitstun` controls whether that timer is applied. Post-resolution
contexts are observational, so mutation there has no gameplay effect.

Scripts may retain scalar generic locals in `fighter.locals`; those locals are
serialized in match state and restored by checkpoint rollback. Supported
commands are the generic `set_action`, `set_velocity`, and `apply_hitlag`
operations, subject to the engine's action and finite-value validation.
The currently accepted action strings are `wait`, `walk`, `dash`, `run`,
`fall`, `jump`, `jump_aerial`, `guard_on`, `guard`, `guard_off`, `escape_air`,
`landing`, and `damage`. The fighter view's action read currently uses the
native debug action spelling; scripts should use the accepted lowercase names
when issuing `set_action`.
Custom scripts that omit `on_projectile_contact` retain the bundled fighter
policy when one is applicable. A script with no applicable bundled policy
retains native behavior.

Programs are compiled and executed in a fresh VM for each dispatch. Source is
bounded to 256 KiB, execution to 100,000 interrupt steps, memory to 8 MiB,
locals to 128 entries with 64-byte keys and 256-byte scalar strings, and
commands to 16 per dispatch.
Scripts have no filesystem, clock, network, host randomness, or unbounded
allocation access.

The Reflector's five-phase action state machine, projectile spawning,
animation, generic damage resolution, hitbox creation, resources, and custom
events remain native. Future migrations should expose those through generic
engine mechanisms and preserve the same one-file-per-fighter organization.
