# ECB response

The headless match composes the translated environmental collision box (ECB),
directed stage queries and resource-driven stage motion. Each movement substep
checks the left-facing wall, right-facing wall and ceiling before resolving the
floor. Opposing wall contacts invoke the original horizontal squeeze arithmetic;
a ceiling and floor in the same pass invoke the original grounded vertical
squeeze arithmetic. The complete pre-squeeze shape is checkpointed in the ECB
state and restored on the next interpolation before returning toward the current
animation sample.

Current and previous sampled stage geometry distinguish motion of the fighter
from motion of the surface. A transformed line can create a contact when its
supporting plane moves from clear to penetrating even if the fighter does not
move. A line moving only along its plane cannot capture a fighter who was
already behind that plane. Rising solid floors land airborne fighters; one-way
floors retain their downward-motion eligibility rule. Stable line IDs remain in
the four contact slots and grounded support state.

`collision::ecb::State::squeeze_horizontal` and `squeeze_vertical` retain the
complete selected `mpCollSqueezeHorizontal` and `mpCollSqueezeVertical` bodies.
Their unit tests cover save-on-first-squeeze and restoration, while
`ecb_differential` compares generated inputs and edge cases with pinned original
C. `game_ecb_response` covers their match composition, moving-line crossings,
platform direction, contact IDs, landing events and deterministic checkpoint
replay.

This profile does not yet reproduce Melee's repeated adjacency walk around
connected corners, wall and ceiling damage callbacks, bounces, wall techs,
ceiling techs, dynamic surface reclassification or stage-object callback graph.
Independent full-game corner and squeeze traces remain required before claiming
parity for the scheduler around these exact arithmetic helpers.
