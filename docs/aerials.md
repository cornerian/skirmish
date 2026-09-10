# Ordinary aerial actions

`fighters[].aerials` supplies five ordinary moves in neutral, forward, back,
up and down order. Each move supplies its attack poses/hitboxes, command-state
flags for every sampled frame, landing lag, landing animation end frame and
integer-frame landing physics poses. These are required numeric resources;
omitting a profile does not supply character defaults.

Fresh A chooses using the main stick. A fresh C-stick excursion can initiate an
attack and overrides the main stick for selection. Excursion means either axis
crosses its absolute threshold from below; holding it or reversing its sign
while still outside the threshold does not retrigger. Up/down selection uses
strict angle comparisons; horizontal selection uses fighter facing. Attacks
retain fast fall, ordinary gravity and drift. Their sampled skeleton places
hitboxes, hurtboxes and bone ECB points through the same physics path as jab.

Command-state samples carry the landing-lag and interrupt flags plus an optional
one-shot facing reversal. Hitlag cannot replay the reversal. Interruptible
frames and released wall-tech actions permit another ordinary aerial or an
available ordinary double jump; aerial selection has priority over that jump.
Exhausting attack samples enters Fall. Item throws, character-specific
overrides, aerial dodges and the remaining interrupt branches are unported.

Floor contact with the landing-lag flag enters the corresponding LandingAir
action. Otherwise it uses ordinary Landing (autocancel). Logical shoulder input
age resets on the aggregate digital/analog rising edge and advances during
hitlag. Pressing R while L or analog pressure remains held does not refresh it.
An age strictly below the configured L-cancel window divides lag, truncates
toward zero and replaces zero with one. Landing animation advances using
`(animation_end + 0.1) / lag`; recovery has no action interrupts and ends at the
declared animation end. Landing poses use the supplied integer animation-frame
samples, without interpolating missing resource data.

Animation completion runs before input dispatch, so holding shield can enter
GuardOn on the exact landing-recovery frame. Jumping out of shield can likewise
select an aerial on the launch frame. Cross-subsystem tests cover those
boundaries and aerial shield contacts with checkpoint restoration.

The integration fixtures invent all coefficients and poses. They test action,
combat, landing and checkpoint behavior; they do not establish authentic
character animation, full original callback ordering or whole-replay fidelity.
The original-C arithmetic comparisons are documented separately in
`tests/oracle/README.md`.
