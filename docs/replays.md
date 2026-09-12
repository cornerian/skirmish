# Slippi replay import and validation

`peppi-adapter` imports completed `.slp` files through **Peppi 2.1.2**. It keeps
Peppi's match settings, metadata, Gecko bytes and Arrow frame columns, then builds
an index selecting the surviving timeline. The parser, native physics libraries
and experimental headless match run on modern machines without an ISO, DOL,
emulator or GameCube runtime.

Inspect a replay from the repository root:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo run --locked --bin skirmish -- inspect-replay /path/to/replay.slp
cargo run --locked --bin skirmish -- inspect-replay /path/to/replay.slp --finalized-only
```

The JSON summary records the exact file's SHA-256 and byte count, parser/format
versions, stage, physical player ports, recorded/surviving/discarded frame counts,
selected frame range and finalization watermark. Ports retain their original
identities: a game on P1/P3 has zero-based ports 0/2, not an invented contiguous
player mapping. Followers remain distinct actors on their leader's port.
Import errors exit unsuccessfully without a success summary.

## Accepted files and memory use

The importer accepts completed Slippi **2.0.0 through 3.18.0** files up to
**512 MiB**, with a nonzero declared raw-event length and a complete `GameEnd`.
It checks the wrapper, event boundaries, actor routing, paired pre/post events,
frame continuity and version-dependent columns. Future formats, live files,
truncated events and unsupported event forms are errors. Recorded absent actors
remain absent; nullable columns do not become zero-valued fighters.

Legacy 2.0/2.1 files have no FrameStart records: a new pre-frame ID opens each
frame, and IDs must advance contiguously from -123 without rollback. Intermittent
actor absence in these legacy files is rejected because Peppi would shift later
rows; trailing absence is supported. FrameEnd and item records are absent before
3.0, and post-frame hurtbox state is absent before 2.1. Version-dependent fields
remain optional rather than being synthesized.

Peppi parses a whole file into Arrow columns. The input bytes and parsed arrays
can coexist during import, so the byte limit is not a total-memory limit.
Transitions are materialized one selected frame at a time from those columns;
this is not a constant-memory streaming file parser. Process a large corpus one
file at a time and release each `Replay` before loading the next.

## Timeline and input timing

`Timeline::LastRecorded` selects the last surviving recording of each frame.
A rollback to an earlier frame removes the old tail before replacement frames
are appended. Original recorded rows and indices remain accessible through
`Replay::game()` and `frame_indices()`. Negative frame IDs are preserved.

`Timeline::FinalizedOnly` requires Slippi 3.7 or later and selects only the prefix
covered by explicit frame-bookend watermarks. `GameEnd` does not finalize the
remaining tail. A completed replay can therefore have an empty finalized prefix;
inspection reports zero selected frames, while replay validation rejects an empty
transition stream. Rewriting already-finalized history is an import error.

Pre/post events with the **same frame ID** produce one
`replay_validation::Transition<Inputs, Frame>`: apply that frame's inputs, then
compare its post-frame observation. Actors are identified by `(port, follower)`.
Peppi's field types and float values are retained, including available raw analog
samples, processed sticks/triggers, logical buttons and physical buttons.
Version-dependent fields remain optional. A simulation input adapter must choose
and document which representation it consumes; import does not silently recalibrate
already processed sticks or replace missing raw samples with zero.

`Inputs` carries the original pre-frame records for that choice. Those records
also contain observed state and RNG fields: an adapter must select controller
inputs explicitly, rather than overwrite simulated position, action or RNG state
with the reference values. Frame start/end data, items and stage-event observations
remain available when recorded.

## Compare a file against Skirmish

```xonsh
cargo run --locked --bin skirmish -- validate-replay /path/to/game.slp --initialization /path/to/init.json
```

Add `--finalized-only` to use the explicit finalized prefix. The harness restores
the supplied complete native checkpoint, converts selected replay inputs, calls
the real `Match::step`, and compares the resulting observations. It starts at the
declared `next_frame` and continues through the selected timeline's last frame.
An absent start frame, empty suffix, unsupported input, simulation error or
observation difference fails; a matching prefix is not silently accepted.

Add `--report /path/to/report.json` to also write the exact JSON object printed
to stdout (the [report fields](#reports-and-regression-cases) plus
`initialization_sha256`) to that path, so a machine-readable outcome survives
independently of captured process output. It is written before the command's
exit status is decided, so it is produced for `matched`, `mismatch` and `error`
outcomes alike; only a failure before a `Report` exists (bad file, bad JSON,
preflight checks in `match_validation::validate`) leaves it unwritten.

The required initialization JSON has these fields, with no defaults:

| Field | Meaning |
| --- | --- |
| `data` | Embedded complete native `MatchData`, in the [match resource format](match.md); not a filename or replay-derived fighter snapshot |
| `seed` | Explicit unsigned 32-bit seed passed to `Match::new` |
| `ports` | Two distinct Peppi port names, such as `["P1", "P3"]`, mapping native fighters 0/1 to physical replay ports |
| `next_frame` | Signed Slippi frame ID whose input is applied by the next native step |
| `warmup` | Array of native controller pairs applied after initialization and before checkpointing; `[]` means no warmup |

For example, construct the file from an existing native resource bundle in xonsh:

```xonsh
import json
from pathlib import Path
initialization = {
    'data': json.loads(Path('/path/to/native-match.json').read_text()),
    'seed': 0,
    'ports': ['P1', 'P3'],
    'next_frame': -123,
    'warmup': [],
}
Path('/mnt/shared/tmp/skirmish-replay-init.json').write_text(json.dumps(initialization))
```

These seed/port/frame choices are explicit example inputs, not values recovered
from the replay. Resources and warmup must actually establish the native state
corresponding to the declared frame. Warmup uses `run-match`'s controller-pair
format; it does not infer earlier actions or hidden state.

Library callers can pass an existing complete in-memory checkpoint to
`skirmish_replay::match_validation::validate`, together with the `Match`, replay, port mapping
and timeline policy. Its `next_frame` labels the first post-step observation to
compare. Persistent checkpoint encoding is not provided.

## Building an initialization from an external match-data export

`make-initialization` derives most of that JSON automatically from a native
`MatchData` export and the replay itself, instead of requiring hand-picked
seed/port/frame values:

```xonsh
cargo run --locked --bin skirmish -- make-initialization \
    --match-data /path/to/match-data.json \
    --replay /path/to/game.slp \
    --output /path/to/initialization.json
```

It reads the replay's `GameStart` once:

- **Ports**: the two occupied ports, sorted ascending, map to native fighters
  0/1 in that order (e.g. `["P1", "P3"]`). Anything other than exactly two
  human-controlled, non-team, non-follower ports is rejected before any other
  check runs.
- **Characters and stage**: `match-data.json`'s `fighters[].name` and
  `stage.name` are matched against small public Slippi/CSS external-ID tables
  (`crates/cli/src/initialization.rs`'s `CHARACTER_EXTERNAL_IDS` and
  `STAGE_EXTERNAL_IDS`, using the same kebab-case convention as
  `tests/fixtures/slippi/manifest.json`'s `character`/`stage` fields) and
  compared against the replay's recorded external character/stage IDs. Any
  disagreement, including an unrecognized name, is a clear `character
  mismatch` or `stage mismatch` error; characters are checked before the
  stage. This is a metadata cross-check, not a claim that the match data's
  physics are authentic.
- **Stocks**: each occupied port's recorded starting stock count must equal
  `match-data.json`'s `rules.stocks`, or make-initialization refuses; `stocks`
  itself is not a separate `Initialization` field; it is `rules.stocks`
  applied uniformly by `Match::new`.
- **Seed**: `GameStart.random_seed`, unless `--seed` overrides it. Peppi
  decodes this field unconditionally for every Slippi version the importer
  currently accepts (2.0.0 through 3.18.0), so there is no in-range version
  for which it is genuinely absent; `--seed` exists for a hypothetical future
  format without the field, and for deliberately reproducing a match under a
  different seed.
- **Handicap**: each occupied port's `GameStart.players[_].handicap` fills
  `match-data.json`'s `players[_].handicap` (`docs/grab-escape-timer.md`).
  Peppi 2.1.2 decodes this field unconditionally, so it is always a real
  recorded value (9 whenever the handicap rule itself is off, which is the
  case for every replay this importer currently exercises), not a
  placeholder needing a presence check.
- **`next_frame` and `warmup`**: `next_frame` is the timeline's first selected
  frame ID. `warmup` is always `[]`: the native `Match::new` checkpoint
  already corresponds to Melee's own pre-game state, which is also where
  Slippi's own frame numbering starts (`peppi::frame::FIRST_INDEX`, `-123`;
  legacy 2.0/2.1 files must advance contiguously from there, so every
  currently-accepted replay's first frame already is `-123`; see "Accepted
  files and memory use" above). make-initialization does not implement any
  warmup-stepping to reach a later start, so it refuses outright if the
  replay's first selected frame is not `-123`, rather than emitting a
  misaligned checkpoint. (This is the same convention `crates/cli/tests/
  replay_match.rs`'s `Recording` helper already follows for its own
  synthetic replays: `next_frame: FIRST` where `FIRST = -123`, `warmup:
  Vec::new()`.)

## Input and observation policy

Native comparison requires exactly two human-controlled leaders in a non-team
match. Peppi preserves
Nana/follower data, but this adapter rejects followers. Processed joystick X/Y
supplies normalized main-stick input; processed C-stick and analog trigger
values pass through without recalibration. Physical button bits supply
A/B/X/Y/Z/L/R and the four D-pad directions (left/right/down/up, the same
low nibble Slippi's physical and processed button words share with native
`HSD_Pad`); only D-pad up has an observable effect (the [taunt
profile](taunt.md)), so a replay containing a left/right/down D-pad press
now imports instead of being rejected.
Derived main-stick, C-stick and logical-trigger flags are accepted; a logical
trigger flag without analog pressure or digital L/R is rejected. Other buttons
remain unsupported. Raw stick and physical analog samples remain preserved by
the importer, but do not replace processed input. This channel support enables
C-stick ASDI and, with explicit resources, aerial selection and direct
main-stick shield drops through one-way platforms. C-stick shield jumps use the
processed C-stick channel and preserve release-based short hops. Grounded rolls
and spot dodges accept fresh main-stick or held C-stick samples and map to
Slippi states 233..235 with animation indices 42, 43 and 41. Shield grabs accept
physical A with a held shoulder or physical Z, and the late-dash buffer yields
CatchDash. Air dodges map to state 236 (animation 44) with FallSpecial 35 (26)
and LandingFallSpecial 43 (36). Tilts map to states 51..57 with animation
indices 53..59, and a catch now follows physical Z or A with a held shoulder.
With the optional `jump_backward_threshold`, a backward ground or aerial jump
maps to state 26 (animation 17) or 28 (animation 19) instead of the ordinary
25/16 and 27/18, and an aerial jump's own animation end maps Fall to state 32
(animation 23) instead of the ordinary 29/20; see [state parity](state-parity.md).
Smashes map to states 58..64 with animation indices 60..66; a held A freezes
the reported action frame while charging. With the optional jab-combo
profile the jab family maps to states 44..49 (animation indices 46..51):
Jab, Attack12 and Attack13 are 44..46, and the rapid jab's Start, Loop and
End are 47..49. With the optional dash-attack profile the dash attack maps
to state 50 (animation 52), entered from a fresh A press during Dash's
middle/late phases or from Run. With the optional edge/teeter profile,
Ottotto and OttottoWait map to states 245/246 (animation indices 210/211),
entered when a facing/stick-admissible walk crosses a floor end. With the
optional [walk-speed profile](walk.md), Walk maps to state 15/16/17 by its
current Slow/Middle/Fast kind (animation indices 7/8/9), and `state_age`
reports the tracked float animation frame instead of the integer action
frame; without the profile, Walk stays state 15 with the integer frame, as
before this batch. Run already maps to state 21 (animation 13); with the
optional [run-animation profile](run.md), its `state_age` likewise reports
the tracked float Run animation frame (wrapping at the Run figatree's
length) instead of the integer action frame; without the profile, Run keeps
the integer frame, as before this batch. Wait already maps to state 14 with
`state_age` as the integer action frame (unchanged by this batch); with the
optional [idle-animation profile](idle.md), the reported animation index
now follows the tracked idle sub-motion (2 while in Wait1_0, otherwise the
last picked entry) instead of the fixed constant 2; without the profile,
the index stays 2 as before this batch. Landing already maps to state 42; with the
optional landing profile it additionally accepts the complete Wait chain
once `movement.normal_landing_lag` elapses, so a C-stick sample on the first
interruptible frame can now select a smash straight out of that state. With
the optional [taunt profile](taunt.md), a fresh physical D-pad-up press maps
to state 264 (animation 239, AppealSR) or 265 (animation 240, AppealSL);
without the profile the press is accepted as input but has no effect. With
the optional [Fox side-special profile](fox-side-special.md), a fresh
physical B press past the side threshold maps to states 347..349 on the
ground or 350..352 in the air (animation indices 301..306 are an
unverified extrapolation, see that profile's own doc); the ground End
returns to Wait's own state 14, and the air End's own natural completion
exits into `FallSpecial`'s existing state 35, landing into the existing
`LandingFallSpecial` state 43. With the optional [Fox down-special
profile](fox-down-special.md), a fresh physical B press past the down
threshold maps to states 360..369 (Start/Loop/Hit/End/Turn on the ground,
then the same five in the air; animation indices are the same kind of
unverified extrapolation, see that profile's own doc); both the ground
and air End's own natural completion return to Wait's state 14 or
`Fall`'s existing state 29, unlike the side special's own air End.
File-backed regressions require changed down-stick, up-C-stick, roll-stick,
horizontal/downward C-stick, shield-grab button, air-dodge trigger, tilt
attack, smash attack or charge, jab press, dash-attack/re-dash press,
landing interrupt-window, D-pad-up taunt and Fox side-special B-press
samples to diverge at their first affected frames.
Pre-frame position, action and RNG never overwrite the simulation.

The named **`fighter-post-v11`** policy compares these post-frame fields for each
mapped actor:

| Replay field | Native observation | Comparison |
| --- | --- | --- |
| `state` | Common action-state mapping | Exact `u16` |
| `state_age` | Action frame, or Walk's/Run's tracked float animation frame when the optional walk-speed/run-animation profile is present | Exact `f32` bits |
| `position.x`, `position.y` | Fighter position | Exact `f32` bits |
| `direction` | Fighter facing | Exact `f32` bits |
| `percent` | Damage percent | Exact `f32` bits |
| `shield` | Shield health | Exact `f32` bits |
| `stocks` | Remaining stocks | Exact integer |
| `airborne` | Inverse of native grounded state | Exact boolean |
| `jumps` | Configured maximum minus jumps used | Exact integer |
| `ground` | Last contacted collision-line ID, retaining `0xffff` before first contact | Exact `u16` |
| `l_cancel` | Per-frame aerial-landing result | Exact integer |
| `character` | GameStart external character mapped through Melee's internal-fighter table | Exact integer |
| `last_attack_landed` | Attacker's retained move-table ID | Exact low byte |
| `combo_count` | Attacker's retained `ftColl_800763C0` counter | Exact low byte |
| `last_hit_by` | Recorded source physical port, or Melee's initial sentinel 6 | Exact integer |
| `last_hit_by_instance` (Slippi 3.16+) | Victim's retained source action-instance ID | Exact `u16` |
| `instance_id` (Slippi 3.16+) | Fighter's current motion-family instance ID | Exact `u16` |
| selected `state_flags` | Reflector, protection, fast-fall, hitlag, active shield, hitstun, inert shield-touch, powershield, dead and sleep/inactive bits | Exact bits |
| `misc_as` while in hitstun | Remaining hitstun | Exact `f32` bits |
| `hurtbox_state` (Slippi 2.1+) | Scripted `x1988` body state, else the timed vulnerable, invulnerable or intangible state | Exact integer |
| `velocities.*` (Slippi 3.5+) | Air X/Y, knockback X/Y and ground X velocity | Exact `f32` bits |
| `hitlag` (Slippi 3.8+) | Remaining hitlag | Exact `f32` bits |
| `animation_index` (Slippi 3.11+) | Common motion table's current `anim_id`, including `-1` as `0xffffffff` | Exact `u32` |

The report's `fields` array follows the replay version, so fields absent from an
older Slippi schema are visible rather than silently claimed. The refactored
action enum collapses some original motion states. Those actions map to one
documented common-state ID. Fox's own Blaster and side-
special profiles both map using GameStart character metadata (gated on
character 2 only -- Falco 22 shares both moves' own code but is not mapped
here); every other
character-specific special and internal elimination remain unmapped and
therefore produce an action-state mismatch. The inactive respawn interval maps
to Melee's common `Sleep` state 11 and its absent animation. Zelda/Sheik
transformations are also unimplemented, so a post-frame internal character
change produces a character mismatch. Action-instance retention covers the
implemented common motion families and Fox's Blaster (one shared identity
across its whole Start->Loop->...->End sequence, `fighter::action_instance`).
Luigi and held-item metadata branches in the original callback remain
outside the native fighter model. Animation-index mapping covers the same
resolved common actions and Fox's own Blaster/side-special states. Refactored directional jump actions use their
canonical first animation, and the original randomized Wait animation changes
are not scheduled yet; Walk does too unless the optional walk-speed profile
is present, in which case its animation follows the tracked Slow/Middle/Fast
kind instead. RNG, collision-line
geometry, the offscreen and follower state-flag bits, items and stage state are
not compared. Reflect and powershield bits follow the independent GuardReflect
timers, including their active-at-zero boundary. Shield-touch follows the
per-frame inert-hitbox overlap signal. The selected dead bit
follows `Fighter::x221F_b1`: it begins immediately for ordinary blast deaths,
after disappearance for star/screen deaths, persists through the inactive
respawn delay, and clears on rebirth. The selected sleep bit follows
`Fighter::x221F_b3` only during that inactive respawn delay.
Nonempty item and dynamic stage-event records in a real file are still
rejected as unsupported simulation: the native simulation now models one
item kind (Fox's Blaster laser, `game::projectile`, `docs/
fox-neutral-special.md`), but the file-backed replay comparison harness
does not yet diff its own `State.projectiles` against a real file's own
item-frame block (`type`/`state`/`position`/`velocity`/`owner`) -- this
batch's own self-recorded regression (`crates/cli/tests/replay_match.rs`)
instead asserts the native `Event`/`Projectile` state directly, which this
engine fully controls. Extending the file-backed comparison itself to
diff real item fields is the concrete next step, not yet done. Equality of
the selected fields does not
establish equality of hidden state, complete frame behavior or Melee gameplay.
The match remains an experimental ruleset with incomplete character resources;
the current Fox subset cannot validate arbitrary real Fox matches.

## Reports and regression cases

Reports identify the observation policy and fields, input policy, replay summary
and hash, `resources_sha256`, explicit `ports` and `checkpoint_next_frame`. The CLI
adds `initialization_sha256`, computed over the exact initialization file bytes.
Its `outcome.status` is:

- `matched`: includes `first_frame`, `last_frame` and `checked_frames`; every frame
  from the declared start through the selected timeline end matched.
- `mismatch`: includes `frame`, matched-prefix `checked_frames`, and `difference`
  containing `port`, `field`, `expected` and `actual` as hexadecimal bit strings.
- `error`: includes a frame when available, matched-prefix count and the
  unsupported condition or failed step's explanation.

Mismatch and error reports exit unsuccessfully. File/JSON parsing, initialization
and preflight checks can fail before a report exists. Keep the report, replay,
initialization and inputs
through the failing frame as a regression case. Complete checkpoints also support
native counterfactual branches without depending on rendering.

The file-backed harness tests write synthetic `.slp` files, run the actual native
simulator and verify successful comparisons and deliberate corruptions. They test
the comparison path and failure reporting, not fidelity to recorded Melee gameplay.
