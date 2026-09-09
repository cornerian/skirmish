# Archived Slippi fixtures

Ten unmodified `.slp` files (32,840,742 bytes, 31.32 MiB) copied from the verified
3,996-file local archive subset. These small corpus fixtures are intentionally
tracked in Git; the remaining archive stays outside the repository. No dataset
download, archive mount, ISO, or emulator is required to run their tests.

Source: [erickfm/slippi-public-dataset-v3.7](https://huggingface.co/datasets/erickfm/slippi-public-dataset-v3.7/tree/c82be5f6e43f3388555cfe0cf8652580601f396d),
revision `c82be5f6e43f3388555cfe0cf8652580601f396d`, declared **CC0-1.0**.
Originally compiled by altf4, with contributions from nikki and yashichi, and
mirrored by erickfm. [manifest.json](manifest.json) records original paths,
SHA-256 hashes, sizes, header metadata, selection rules and expected imports.
Every copied file was checked against its archive manifest hash.

Selection greedily favors additional character IDs, stages, recording versions,
occupied-port layouts and duration buckets, with smaller files breaking ties.
This is a coverage-oriented sample, not a representative tournament benchmark
or a proof of globally optimal coverage. Character labels use GameStart's
external character IDs; transformations during play can add other forms.

The selection covers 24 of 26 observed character IDs (missing Kirby and Luigi),
all six observed stages, seven of eight format versions (missing 3.6.0), seven
of eight port layouts (missing P1/P3/P4), and all three duration buckets.
It includes Ice Climbers followers and a recording with 2,102 discarded rollback
frames. Despite the upstream card's filtering description, the actual subset
also contains four-player and sub-30-second recordings, both represented here.

| File prefix | Match | Stage | Format | Last frame |
| --- | --- | --- | --- | --- |
| 01 | Marth / Dr. Mario / Yoshi / Captain Falcon | Battlefield | 3.9.0 | 8514 |
| 02 | Jigglypuff / Bowser / Zelda / Falco | Battlefield | 3.9.0 | 12656 |
| 03 | Mario / Donkey Kong | Final Destination | 2.0.1 | 1722 |
| 04 | Samus / Ganondorf | Yoshi's Story | 1.7.1 | 15835 |
| 05 | Fox / Peach | Fountain of Dreams | 2.2.0 | 5264 |
| 06 | Mr. Game & Watch / Ness | Dream Land | 3.7.0 | 8815 |
| 07 | Ice Climbers / Young Link | Pokémon Stadium | 3.3.0 | 16074 |
| 08 | Sheik / Link | Fountain of Dreams | 3.0.0 | 8280 |
| 09 | Mewtwo / Pikachu | Fountain of Dreams | 2.0.1 | 3474 |
| 10 | Pichu / Roy | Battlefield | 2.0.1 | 11719 |

`tests/slippi_corpus.rs` verifies byte identity and the CLI's complete import
summary for nine supported files. The 1.7.1 file must fail explicitly with an
unsupported-format error and no success output. Summary expectations were
captured from the existing Peppi 2.1.2 import path; they are regression snapshots,
not an independent simulator oracle. The system workflow runs this target in
default debug, C-oracle debug and C-oracle release configurations.

These files do not yet supply the native resources and initial checkpoints
needed for whole-match simulation parity. The separate synthetic `replay_match`
tests exercise that comparison harness and first-divergence reporting.
