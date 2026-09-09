# arena

Experimental native match orchestration, independent of rendering. Provides
explicit controller stepping, immutable native resources, complete mutable
state, reset, checkpoints and replay-validator integration. Bone poses and
collision arithmetic belong to `physics`.

The included two-player fixture exercises a match from countdown to completion;
its data is synthetic and does not represent Melee characters. See
[scope, commands and missing rules](../../docs/match.md).

```xonsh
$CARGO_TARGET_DIR = '/mnt/shared/tmp/skirmish-target'
cargo test --locked -p arena
```
