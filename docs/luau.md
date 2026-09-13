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
contexts expose the committed result: post-hook hit-field edits are ignored,
but post-hook commands and local updates still apply.

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

## Adding a hook or primitive

The API is intended to be composable rather than complete. A rough design
target is 20–40 reusable primitives and 10–20 lifecycle hooks over time; those
numbers are guidance, not a quota or a statement of current coverage. Rust
owns mechanisms and their ordering: collision, hit detection, actions,
physics, controller input, and animation. Luau owns character policy, such as
Marth's Counter decision or Yoshi's armor rule. A fighter source file should
eventually contain about 500–1,000 lines of policy in one
`scripts/fighters/<fighter>.luau` file instead of one file per move.

Start by composing an existing hook and its context. Add a lifecycle hook only
when the native engine lacks an interception point with a clear, reusable
semantic boundary. Add a primitive only when that existing or new boundary
lacks a reusable operation or context field. The primitive should work for
more than one fighter or move. Names alone do not make an API reusable:
`set_armor`, `enable_counter`, and `start_float` are bespoke operations even
if their names sound convenient. Prefer a generic damage, action, state, or
resource boundary whose behavior remains native and whose policy can be
selected by script.

Before adding a lifecycle hook, write its contract down. Specify the exact
phase and order, which participants run it, every mutable field, which derived
values are recomputed after edits, the scope of cancellation, and which side
effects happen once. State what the hook observes and what it returns. Current
fighter hit preparation runs attacker `before_hit` followed by defender
`before_receive_hit`; the defender sees the attacker's restrictive gates, and
the resolver recomputes launch and hitstun when `knockback` changes. Resolution
then runs attacker `after_hit` followed by defender `after_receive_hit`. Those
post hooks observe the committed result: returned hit-field edits are ignored,
while returned commands and locals still apply.

For example, a script can set `ctx.apply_knockback = false` to preserve damage
and hitlag while suppressing launch. Armor and counter policies may combine
these independent effects differently. Full hit cancellation has a separate
contract: it skips normal hit effects and contact bookkeeping while retaining
the hook's explicit commands and local-state updates. Keep both mechanisms
available rather than forcing every policy through an armor toggle.

Keep the boundary deterministic. Define numeric domains and finite-value
handling, error behavior, and rollback behavior. Persist only bounded scalar
locals; keep resource identity explicit and bounded by the VM limits. A hook
must not expose filesystem, clock, network, host randomness, or an unbounded
allocation path. If a hook can be called for simultaneous contacts, specify
whether it runs once per contact and in what stable order.

The current implementation wiring is explicit:

1. Add the `Hook` variant and exact Luau function name in
   [`src/game/script.rs`](../src/game/script.rs), then define its view/context,
   result fields, validation, and dispatch semantics there.
2. Add or reuse a generic `Command` only when the native application path is
   reusable. Wire validation and application through the same script module;
   do not let Luau mutate `State` directly.
3. Call the hook at its owning phase. Frame policy is dispatched by
   [`simulation::advance`](../src/game/simulation.rs); fighter contacts are
   prepared and resolved by [`damage::prepare_hit`](../src/game/damage.rs) and
   [`damage::resolve_prepared_hit`](../src/game/damage.rs); projectile policy
   is currently called from [`projectile::advance`](../src/game/projectile.rs).
   Thread the same contract through fighter, projectile, and throw paths when
   the semantic boundary applies.
4. Expose the hook through resource selection and validation. Check the
   `FighterData.script` field and its deserialization in
   [`game/data.rs`](../src/game/data.rs), bundled source lookup in
   [`script.rs`](../src/game/script.rs), and per-hook fallback selection in
   [`projectile.rs`](../src/game/projectile.rs). Check resource identity
   hashing and script-state storage in [`game/mod.rs`](../src/game/mod.rs),
   plus state validation in [`game/validation.rs`](../src/game/validation.rs),
   when context or locals change.
5. Update this document and add behavior tests for the applicable lifecycle.
   Every new hook or primitive needs missing-hook/no-op compatibility,
   meaningful error handling, and checkpoint rollback coverage. Add
   independent-effects coverage when effects can combine, and trade,
   cancellation, and bookkeeping cases for affected contact paths. Preserve
   local side effects and verify native equivalence; test the behavior rather
   than mirroring implementation structure.

The current six-hook/three-command surface is deliberately smaller than that
future design. `on_projectile_contact` is presently reached only for the
Reflector geometry path when the target is reflecting; it is not a universal
projectile or body-contact callback. Its current projectile context also
leaves unrelated hit fields at their defaults. The migrated Reflector policy
is the only fighter policy migrated so far; the rest of the native move and
action implementation remains the reference behavior while the API grows.
