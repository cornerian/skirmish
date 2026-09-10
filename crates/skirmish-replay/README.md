# skirmish-replay

Concrete integration between Slippi observations, the native Skirmish match,
and the simulator-independent machinery in `replay-validation`.

This crate owns replay input conversion, observation policy, checkpoint-driven
match validation, and its explicit comparison reports. It never initializes
hidden simulator state from recorded observations.
