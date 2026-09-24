# Captain Falcon Dive capture lifecycle

This document is a source-backed specification for Falcon Dive's capture and
throw relation.  It records fighter-visible behavior that the host can test;
it does not treat animation or state coverage as proof of full decomp parity.
The pinned upstream revision is `0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9`,
recorded in [`upstream.lock.json`](../../upstream.lock.json).  Paths below are
relative to the pinned checkout at
`/mnt/shared/Projects/Code/External/melee/`.

## Source callback chain

The Raptor Boost start callback `ftCa_SpecialS_OnDetect` only accepts a
fighter or eligible item contact after the animation command has set
`cmd_vars[0]`.  A startup contact before that cue must leave Falcon in
`SpecialSStart` / `SpecialAirSStart`; once the cue is present, grounded and
airborne contacts enter `SpecialS` / `SpecialAirS` respectively.  This gate is
independent of the later movement and landing callbacks and is represented by
Captain's concrete `before_hit` callback.

The entry callbacks are `ftCa_SpecialHi_Enter` and
`ftCa_SpecialAirHi_Enter` in
`src/melee/ft/kinds/ftCaptain/ftcaptainspecialhi.c`.  They select motion
states 353 and 354, clear/start the special-hi animation, and register the
common capture callback `ftCo_8009CA0C` through `ftCommon_8007E2D0`.
Animation and collision callbacks in that file provide the normal fall,
landing, ground/air conversion, and ledge boundaries when no capture occurs.

When the dive catches, `ftCa_SpecialLw_800E5128` in the same file enters
`ftCa_MS_SpecialHiCatch` (state 355), clears movement, and splits attachment
by the victim's ground/air state:

* An airborne victim uses the common attachment path and leaves the holder's
  grounded attachment flag clear.
* A grounded victim calls `ftCo_800DB368(victim, holder)`, sets the grounded
  attachment flag, and installs `ftCa_SpecialLw_800E550C`, whose callback keeps
  the holder root at the victim root.

The common implementation is in
`src/melee/ft/kinds/ftCommon/ftCo_CaptureCaptain.c`.
`ftCo_8009CA0C` establishes the holder's victim object pointers, clears the
capture transient flag, reverses holder facing relative to the victim, and
selects `ftCo_MS_CaptureCaptain` (motion state 275).  The airborne path uses
`ftCo_800DB464` as its accessory attachment callback.  The exact pointer and
callback representation is native object state; the host may use a stable
relation record as long as its observable owner/victim transitions match.

The grounded low-special collision callback has one additional state rule:
when command slot 0 is set and the wall is on Falcon's facing side,
`ftCa_SpecialLw_Coll` clears command slots 0 through 2 before entering
`ftCa_MS_SpecialHiThrow1` (state 363).  This prevents the same contact cue
from retriggering during the rebound motion.

When the catch animation reaches its throw command,
`ftCa_SpecialHiCatch_Anim` calls `doCatchAnim` in
`ftcaptainspecialhi.c`.  That function selects `ftCa_MS_SpecialHiThrow`
(state 356), clears the capture command state, and calls
`ftCo_800DE2A8`/`ftCo_800DE7C0` from
`src/melee/ft/kinds/ftCommon/ftCo_Throw.c` to initialize the victim throw
and its hit/damage setup.  `ftCa_SpecialHiThrow1_Coll` delegates to
`ftCo_AirCatchHit_Coll` in
`src/melee/ft/kinds/ftCommon/ftCo_AirCatch.c`, which is the wall-rebound
boundary for the throw's later phase.  The special-hi animation and physics
callbacks then select ordinary fall or landing-fall-special exits.

## State and relation contract

| Event | Holder (P1) | Victim (P2) | Relation and observable effects |
|---|---|---|---|
| Dive starts | `SpecialHi` / motion 353 or 354 | vulnerable, free of a conflicting grab/capture | Holder's dive callbacks are active. No capture relation exists yet. |
| Capture accepted | `SpecialHiCatch` / motion 355 | `CaptureCaptain` / motion 275 | P1 owns P2. Holder facing becomes the opposite of P2's facing. Movement and knockback are suppressed while the pair is valid. |
| Grounded attachment | Catch | CaptureCaptain | P1 follows P2's root through the grounded holder-to-victim attachment callback. |
| Airborne attachment | Catch | CaptureCaptain | P2 follows P1 through the airborne victim-to-holder attachment callback. |
| Catch command | `SpecialHiThrow` / motion 356 | throw setup from `ftCo_800DE7C0` | The pair remains associated until release; throw hit/damage is initialized once. |
| Release or broken pair | throw/fall/landing according to the current callback | thrown victim or ordinary fall/damage state | Owner/victim relation is cleared exactly once. A wall-rebound path may enter motion 363 (`SpecialHiThrow1`) before its air-catch collision callback resolves. |

There is no independent Captain Dive article object in
`ftcaptainspecialhi.c`: the move uses fighter object pointers, motion states,
common capture/throw callbacks, and animation/effect callback state.  An
implementation should therefore not infer an `Item_GObj` spawn dependency.
Visual effects and command callbacks remain dependencies of the native
animation/resource layer.  `ftCo_800DD168` clears an earlier victim relation,
and `ftCo_800DE7C0` performs the throw-side victim setup; both are part of the
common throw source rather than separate Dive articles.

## Exact deterministic test vector

Use two fighters in a fixed 60 Hz host step.  The values below make the
relation and release result independently assertable:

```json
{
  "holder": {"player": 1, "action": "SpecialHi", "motion": 353,
             "ground_or_air": "ground", "facing": -1},
  "victim": {"player": 2, "action": "Wait", "ground_or_air": "ground",
             "facing": 1, "percent": 23},
  "capture_event": {"frame": 12, "accepted": true},
  "expected_capture": {
    "holder_action": "SpecialHiCatch", "holder_motion": 355,
    "victim_action": "CaptureCaptain", "victim_motion": 275,
    "owner": 1, "victim": 2, "holder_facing": -1,
    "attachment": "GroundedHolderToVictim",
    "holder_velocity": [0, 0], "victim_velocity": [0, 0]
  },
  "throw": {"end_catch_frame": 30,
            "holder_action": "SpecialHiThrow", "holder_motion": 356},
  "release_hit": {"damage": 12, "angle_raw": 361, "growth": 82,
                   "fixed": 0, "base": 40},
  "expected_release": {"hit_count": 1, "relation": null}
}
```

Repeat the same vector with both fighters airborne before frame 12 and expect
`attachment: AirborneVictimToHolder`; all other relation assertions remain the
same.  A release test must also assert that the hit is emitted once and that
the pair is cleared even if no optional release resource is present.  The
resource-backed values in this vector correspond to the current Captain Dive
throw payload; a fixture that omits that payload can validate lifecycle only,
not exact hit parity.

## Current host gap

`src/game/special_capture.rs` intentionally represents this native object
relation as a checkpoint-safe state: a holder `victim` slot, a victim `captor`
slot, and `Attachment::{GroundedHolderToVictim, AirborneVictimToHolder}`.
That covers ownership, facing, velocity reset, attachment selection, release,
and relation cleanup.  Its resource-backed release path consumes the Captain
Dive capture/throw payload and can apply capture damage and the release hit.

The authoring layer now exposes the deterministic terminal callback for
motion 363: `ftCa_SpecialHiThrow1_Anim` transitions the holder to ordinary
fall when the rebound animation ends.  The remaining gap is the native
callback graph.  The host has no direct
`accessory1_cb`/`accessory4_cb`, `HSD_GObj` pointer, or effect-object
lifecycle.  It uses resource-driven attachment helpers instead;
missing XRotN/TransN2 pose data therefore prevents an exact pose snap.  Native
special-hi physics, victim throw knockback/hitlag, visual effects, and the
full `ftCo_AirCatchHit_Coll` wall-rebound collision behavior still require
their host event/resource contracts before they can claim exact parity.  Older fixtures
without the optional Captain resource tree can prove relation lifecycle but
must not be reported as proving release-hit behavior.
