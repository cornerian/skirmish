# Fighter nudges

`fighter::nudge::velocity` ports ftCommon_8007E0E4, ftCommon_8007DD7C and
ftCommon_8007DFD0 from the pinned `src/melee/ft/ftcommon.c`; Body::effective_position
ports ftCommon_8007F8B4. It returns fixed X/Z velocity increments for one fighter
in an ordered body list. It does not correct penetration, prevent every crossing,
or currently run inside `game::Match`.

Every coefficient is supplied explicitly. `center_offset` and `half_width` are
the fighter DAT's x50 Vec2 after the scale-Y modification in ftCo_800D105C;
hurtboxes, bones and ECB dimensions do not supply this push range. `Rules` maps
the common-data fields x450, x454, x458, x45C and x460. `Neighbors` contains
resolved mpLineGetPrev/Next results, available from `collision::stage::Stage`.
The alternate-link enable, visibility and strict endpoint-distance checks must
run before populating it. The nudge helper only checks same/adjacent floor IDs.

The strict horizontal overlap check includes each facing-relative center offset.
It uses entity-list order for coincident-center ties and excludes same-player
targets. Source eligibility differs between the subject, ordinary targets and
the follower's owner; the Body field comments preserve the individual bits.
A frozen target can still push another fighter. A follower receives its owner's
extra negative-depth bias and has zero horizontal output. When overlap nudges
are disabled, depth still recenters. Zero-crossing changes a temporary depth
before the cap checks; the original sequence is retained rather than replaced
with a conventional final-position clamp.

Future native integration must preserve the priority-1 per-entity Anim→nudge
order, followed by priority-3 input dispatch, priority-4 movement and priority-6
map callbacks. Calculate nudge before any fighter's physical position advances;
do not let an earlier fighter's integrated movement change a later query. Earlier
animation callbacks may change facing or grounded state visible to later actors.
Nudge resets in the animation pass, not on every action change. Preserve the
velocity and persistent depth in checkpoints, apply X/Z before self velocity and
knockback, and include depth in physics-bone/contact transforms. Wait/Walk also
branch on backward nudge in ft_80084280; complete ledge behavior needs that map
branch. Follower ownership and deferred-position state must remain explicit.

The C tests compile all four original functions from the existing snapshot,
using documented entity/neighbor storage adapters. Finite physical inputs use
unit facing, nonnegative widths/coefficients and valid floor/owner references.
The safe API rejects malformed inputs and nonfinite effective positions or final
nudges; effective_position itself is the unchecked three-addition primitive.
Original-C comparisons check every output bit, including signed zero. Ordered
world tests compose the helper with stage links and bone transforms; they do not
claim native match scheduler or PowerPC binary equivalence.

Verification artifacts for this batch live at
`/mnt/archive/runs/skirmish-nudge-kernel-20260909`.
