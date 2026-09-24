# Ice Climbers article and partner specification

This specification covers both sides of the pinned `ftPopo`/`ftNana` source.
Popo owns special rows 341–358; Nana's paired rows are 359–366 and are driven
by Popo's state. The Python script exports Popo's rows and exposes only
decisions that can be made from callback context.

## Source anchors and IDs

| Feature | Popo source rows | Pinned source anchors | Ownership |
| --- | ---: | --- | --- |
| Ice Shot | 341 ground, 342 air | `ftPp_SpecialN_Anim`, `ftPp_SpecialN_Phys`, `ftPp_SpecialN_Coll` | Popo phase selection; ice article native |
| Squall Hammer S1/S2 | 343–346 | `ftPp_SpecialS1_Anim/Phys/Coll/IASA`, `ftPp_SpecialS2_Anim/Phys/Coll/IASA` | Popo phase and wall branch; collision and Nana sync native |
| Belay with Nana | 347–356 | `ftPp_SpecialHiStart_0_Anim`, `ftPp_SpecialHiThrow_0_Anim`, `ftPp_SpecialHi_8012280C` | Popo command branches; rope and Nana movement native |
| Blizzard | 357 ground, 358 air | `ftPp_SpecialLw_Anim`, `ftPp_SpecialLw_Phys`, `ftPp_SpecialLw_Coll` | Popo phase selection; blizzard article native |

Nana's source rows are paired implementation rows rather than additional Popo
special roots:

| Nana behavior | Nana source rows | Pinned source anchors | Ownership |
| --- | ---: | --- | --- |
| Squall Hammer follow | 359 ground, 360 air | `ftNn_Init_80123B3C`, `ftPp_SpecialS_0_Anim/Phys/Coll` | Nana follows Popo motion and frame; native paired state |
| Belay follow and launch | 361–366 | `ftNn_Init_801230D0`, `ftPp_SpecialHi_0..4_Anim/Phys/Coll` | Nana rope attachment, throw, launch, and terrain response native |

The stable fighter identity is external ID `14` (`ice-climbers`).  The
current article catalog does not publish numeric Ice Shot, Blizzard, or Belay
rope article IDs.  They must be registered from the pinned native article
table before scripts can use `fighter.spawn_article`; inventing numeric IDs
would permit an article to be staged without its source callbacks.

## Article attributes and ownership

Ice Shot and Blizzard are native articles.  Their source-owned attributes are
the article position and facing at creation, animation and physics state,
hitboxes, lifetime, owner, hitlag, and terrain/contact behavior.  The article
must remain associated with its Popo owner for damage attribution and stale
move identity.

Belay's rope is a native paired object rather than a generic projectile.  It
owns rope attachment, length, collision, Nana's teleport/throw position,
launch velocity, and wall/ceiling resolution.  The rope must be removed or
invalidated transactionally when the partner becomes unavailable.

Nana owns a separate paired fighter state: existence/alive status, source
action and frame, world position, attachment/follow mode, hitlag, and launch
state.  Popo callbacks need two read-only facts at command trace delivery:

* command slot 2: `partner_available`; false selects rows 350/355 from start
  rows 347/352;
* command slot 1: `partner_launching`; true selects row 354 from throw rows
  348/353.

These facts must be computed from Nana state, not inferred from command slot
values.  The current callback context has no paired-fighter lookup; the
minimal native bridge is to add these booleans to the command event context.

## Popo transitions

The Python phase graph has the following source transitions:

* 343–346 retain the active S1/S2 source row on wall contact. Native
  collision code reverses velocity and synchronizes Nana; it does not select a
  different Popo motion row.
* 347/352 with command slot 2 and unavailable Nana advances to 350/355,
  preserving state and frame.
* 348/353 with command slot 1 and Nana launching advances to row 354, as in
  `ftPp_SpecialHi_8012280C`.
* 353, 354, and 356 end in source special fall; rows 354 and 356 landing on
  ground enter the shared special landing action instead of rows 349/351.
* Ordinary ground/air changes preserve the matching row and frame.

## Deterministic test vector

Start Popo in `Source.14:347` with command state `(0, 0, 1, 0)` and deliver a
command-slot-2 event with value `1` plus `partner_available = false`.  The
expected action is `Source.14:350`, with the same action frame and preserved
action state.  Replaying the vector with `partner_available = true` leaves
Popo in `Source.14:347`.

Start Popo in `Source.14:348` with command state `(0, 1, 0, 0)` and deliver a
command-slot-1 event with value `1` plus `partner_launching = true`.  The
expected action is `Source.14:354`.  With `partner_launching = false`, Popo
remains in `Source.14:348`.

## Current host gaps

1. The article catalog has no Ice Climbers article descriptors or numeric
   IDs, so generic article staging cannot simulate Ice Shot or Blizzard.
2. There is no paired Nana entity state exposed to lifecycle callbacks.
3. Belay rope creation, Nana teleport/throw, launch, hitlag, and wall/ceiling
   collision are native-only.
4. Squall Hammer Nana synchronization and collision rebound physics remain
   native-only even though Popo's active-row retention is expressible.
