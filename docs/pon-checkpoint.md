# Pon fighter scripts checkpoint

This is a work in progress checkpoint for the Pon fighter scripts and
gameplay ownership refactor. The installable SDK lives in `scripts/api`.

The current boundary includes the 21-module consolidation from the old game
and fighter ownership split, the command tuple, and the recovered exact
aerial, dash, and taunt callbacks. Smash and tilt excerpts are preserved and
jab has been reconstructed. Validation remains pending.

Previously passing checkpoints include SDK-28, native Fox load-2, and
neutral-repeat-1. The latest root test run is unresolved because the
environment is missing dataclasses; this is an environment diagnosis, not a
full-goal or parity claim. Fixture resource type fixes are complete.

Remaining work includes merging the 30 commits currently ahead on
`origin/main` and running the repository's full required verification gates.
