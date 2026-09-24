# Mario special parity audit

This audit records the command and transition timing found in the pinned
decompilation for Mario’s four special moves. It covers the fighter callbacks
represented by [`scripts/fighters/mario.py`](../../scripts/fighters/mario.py);
article, cape item, effect, physics, and collision ownership remains at the
native host boundary.

The authoritative source is revision
`0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`, recorded in
[`upstream.lock.json`](../../upstream.lock.json), under
`src/melee/ft/kinds/ftMario/`.

| Move | Source timing and transitions | Python coverage | Status |
| --- | --- | --- | --- |
| Fireball (343/344) | `ftMr_SpecialN_Enter` clears command slot 0 and `throw_flags`; B0 spawns the article; animation completion exits to Wait on ground and Fall in air; ground/air collision callbacks preserve the motion pair. | `B0ArticleSpecial` owns B0 input, spawn, paired transitions, and command reset. Mario’s entry callback now also clears `throw_flags`. | Entry reset and action transitions are regression-covered; article behavior remains host-owned. |
| Cape (345/346) | `changeAction` clears command slots 0–2 and the internal reflect latch. `ftMr_SpecialS_Phys` opens reflection when command 1 is one and closes it when command 1 returns to zero. Ground animation ends in Wait; air animation ends in Fall. | Entry reset, command-1 reflect window, projectile eligibility, and paired surface transitions are exposed through hooks and tests. | Fighter-level reflect timing is covered; cape item callbacks and hit geometry remain host-owned. |
| Super Jump Punch (347/348) | Both entries clear command slot 0 and `throw_flags`; aerial entry also resets velocity through source attributes. Both animation callbacks call `ftCo_80096900` with Mario's freefall mobility and landing-lag attributes. | Entry resets and the optional `enter_fall_special` callback represent the source terminal path; surface pairing is preserved. | Velocity, landing-lag values, and the resulting fall-special action remain host-owned. |
| Tornado (349/350) | `doStartMotion` clears command slots 0 and 1. The aerial animation callback consumes command 1 on every animation tick and latches the persistent tornado charge before its terminal landing/fall decision. Ground physics can re-enter aerial state on the command-2 tap path. | Entry reset, aerial command-1 consumption, and ground/air transitions are represented; the persistent physics flag and motion velocity remain host-owned. | Command cue and transitions are covered; full physics/collision parity requires native attributes and host callbacks. |

The independent Python contracts are in
[`scripts/api/tests/test_mario.py`](../../scripts/api/tests/test_mario.py) and
[`scripts/api/tests/test_mario_audit.py`](../../scripts/api/tests/test_mario_audit.py).
These tests establish callback boundaries and state resets; they do not claim
complete decoded animation, hitbox, article, or item parity.
