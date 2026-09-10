# In-game asset import

Open the graphical game and select **Import Game Assets** from the main menu.
Use arrows/D-pad and Enter/A to select **Find ISO automatically** or
**Choose ISO file...**. You can also drop an ISO onto the game window.
The file picker uses the operating system's normal dialog; automatic search and
drag-and-drop remain available if the desktop does not provide a file picker.

The native Rust importer supports the unmodified Melee USA 1.02 image only.
It rejects incorrect file sizes and headers first, then identifies the complete
image with streaming **XXH3-128** before extracting all 1,209 game files. Other games,
regions, revisions, modified images, and truncated images are rejected. It does
not download a disc, mount it, launch an emulator, or invoke Python/Bun/Dolphin.
The same game executable contains the importer and the progress screen.

Search visits Downloads, Games, and Desktop under the player's home directory.
On this development machine it also checks `/mnt/archive/datasets/melee` and
`/mnt/shared/Games` when those directories exist. Directory symlinks and
inaccessible paths are skipped; uppercase `.ISO` filenames are accepted.
Choosing a file avoids searching these folders.
If automatic search finds no valid image, **Choose ISO file...** becomes the
selected action. Press Enter/A to open manual browsing; the error remains visible.

The reference XXH3-128 is `a661231bb4bee1822b3451322e008583`, measured from the
independently SHA-256-verified source image. XXH3 is a fast noncryptographic
identity/integrity check, not an authenticity signature. The manifest records
the XXH3 fingerprint and the known reference SHA-256; per-file SHA-256 hashes
remain compatible with existing installed bundles. The developer helper
`cargo run --locked --example iso_fingerprint -- /path/to/game.iso` measures
full-file XXH3 time. A warm local read of the 1,459,978,240-byte reference image
took 0.199 seconds here; cold storage and other machines will differ. The hash
dependency is optimized in development builds as well as release builds.

See the [canonical asset tree](asset-tree.md) for the categorized decoded-output
contract and the [complete source tree](asset-source-tree.md) for every input file.

Search, hashing, extraction, and installed-file verification run on a worker
thread while the SDL window continues processing events. **Cancel import** or
Esc/B cancels that work; closing the game requests cancellation and waits for
the worker to clean up. Cancelling the OS file picker returns to the screen.
Only one import can run per screen, and an OS-managed installation lock prevents
two game processes from publishing the same destination concurrently.

Allow about 1.5 GB of free space. By default, SDL chooses the operating system's
per-user application data folder for organization/application `Skirmish/Skirmish`.
The importer installs `assets/melee-usa-1.02` beneath it. Success shows the actual
location. Developer/test launches can override the entire bundle directory with
`--asset-dir DIRECTORY`; `--import-assets` opens this screen directly.
Players do not need either option.

```text
melee-usa-1.02/
  manifest.json               # version, ISO hash, file ranges/sizes/hashes
  disc/files/                 # original game filesystem paths
    PlFx.dat
    PlFxNr.dat
    GrNBa.dat
    ...
```

Extraction occurs in a private temporary sibling folder. The final directory
appears only after files and the manifest are written and checked. Failed or
cancelled imports remove staging files; existing destinations are never merged
or overwritten. A small persistent sibling lock file is normal: its OS lock is
released even if the process crashes. A forced process kill can leave a hidden
`.skirmish-import-*` staging folder, but never a completed bundle marker.
Existing installations are verified against every recorded file hash without
requiring the ISO. A damaged installation reports an error instead of silently
overwriting it.

`extraction::AssetBundle::open` opens an installed bundle independently of
the disc. `resolve("PlFx.dat")` returns a checked path within `disc/files`;
unknown IDs, traversal paths, escaping symlinks, missing files, and size
mismatches are rejected. `verify` additionally checks every SHA-256.
The manifest uses the existing `skirmish-iso-import-v1` original-file layout,
so the extraction output remains usable with `skirmish-assets` offline tools.

## Current scope

This is the **original-file import stage**, now integrated into the game. It
does not yet port the Python/Bun HSD image/mesh conversion pipeline to Rust or
connect imported files to a faithful playable match. It does not create the
`scenes/*/scene.json` exports produced by the earlier offline importer. The UI
reports this limitation before and after import. Visual scenes still use the
existing `--scene` loader; native gameplay continues using its existing resources.
No system executable/DOL is extracted. Builds, ordinary tests, and the game demo
remain independent of a disc.

## Verification

The ordinary workspace tests exercise synthetic disc extraction through the
bundle resolver, corrupt input rejection, cancellation, preservation of existing
files, controller/menu navigation, dialog callbacks, worker error recovery, and
screen geometry. They contain no original game bytes.

The optional local-disc test additionally compares every extracted file
byte-for-byte with its ISO range, then verifies reuse without the ISO:

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
$SKIRMISH_TEST_ISO = '/mnt/archive/datasets/melee/melee-usa-v1.02.iso'
cargo test --locked -p skirmish --test asset_disc -- --ignored
```

The optional test writes to a temporary directory and removes it afterward.
