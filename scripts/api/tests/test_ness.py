"""Source-contract tests for Ness's fighter-side special states."""

from types import SimpleNamespace
import sys
import unittest
from pathlib import Path


ROOT = Path(__file__).parents[3]
sys.path.insert(0, str(ROOT / "scripts" / "api"))
sys.path.insert(0, str(ROOT / "scripts"))

from fighter import Action, Button, export_definition
from fighters.ness import Ness, PKFire, PKFlash, PKThunder, PSIMagnet


class _Input:
    def __init__(self, stick=(0.0, 0.0), pressed=True):
        self.stick = stick
        self.pressed = pressed

    def just_pressed(self, button):
        return self.pressed and button is Button.B


class _Fighter:
    def __init__(self, action=Action.WAIT):
        self.action = action
        self.action_frame = 0
        self.changes = []

    def change_action(self, action, **kwargs):
        self.action = action
        self.changes.append((action, kwargs))


class _AnimationAwareFighter(_Fighter):
    def __init__(self, available=True):
        super().__init__()
        self.available = available
        self.queried_states = []

    def has_complete_animation(self, state):
        self.queried_states.append(state)
        return self.available


class _LandingFighter(_Fighter):
    def __init__(self, action):
        super().__init__(action)
        self.fall_special = None

    def enter_fall_special(self, *, mobility, landing_lag):
        self.fall_special = {"mobility": mobility, "landing_lag": landing_lag}


def _context(*, resource=None, stick=(0.0, 0.0), grounded=True, pressed=True):
    available = {resource} if resource is not None else set()
    return SimpleNamespace(
        input=_Input(stick, pressed),
        ground_open=grounded,
        air_open=not grounded,
        resource=lambda path: object() if path in available else None,
        rules=SimpleNamespace(
            specials=SimpleNamespace(side_stick_threshold=0.5, vertical_threshold=0.5)
        ),
    )


class NessTests(unittest.TestCase):
    def test_native_special_states_and_article_boundary(self):
        exported = export_definition(Ness).as_dict()
        expected = {
            "neutral": {
                "ground_start": 348, "ground_hold": 349,
                "ground_release": 350, "ground_end": 351,
                "air_start": 352, "air_hold": 353,
                "air_release": 354, "air_end": 355,
            },
            "side": {"ground": 356, "air": 357},
            "up": {
                "ground_start": 358, "ground_hold": 359, "ground_end": 360,
                "ground_launch": 361, "air_start": 362, "air_hold": 363,
                "air_end": 364, "air_launch": 365, "air_rebound": 366,
            },
            "down": {
                "ground_start": 367, "ground_hold": 368, "ground_hit": 369,
                "ground_end": 370, "ground_turn": 371, "air_start": 372,
                "air_hold": 373, "air_hit": 374, "air_end": 375,
                "air_turn": 376,
            },
        }
        for root, phases in expected.items():
            for phase, state in phases.items():
                action = exported["actions"][f"special.{root}.{phase}"]
                self.assertEqual(action["slippi_state"], state)
        self.assertTrue(
            exported["actions"]["special.neutral.ground_hold"]["animation_loop"]
        )
        self.assertFalse(any(binding.callback == "spawn" for move in (
            PKFlash(), PKFire(), PKThunder(), PSIMagnet()
        ) for binding in move.events()))

    def test_entry_is_resource_gated_and_routes_directional_b(self):
        for move, resource, stick in (
            (PKFlash(), "neutral", (0.0, 0.0)),
            (PKFire(), "side", (1.0, 0.0)),
            (PKThunder(), "up", (0.0, 1.0)),
            (PSIMagnet(), "down", (0.0, -1.0)),
        ):
            blocked = _Fighter()
            self.assertFalse(move.input_pressed(blocked, _context(stick=stick)))
            self.assertFalse(blocked.changes)

            fighter = _Fighter()
            self.assertTrue(
                move.input_pressed(
                    fighter, _context(resource=resource, stick=stick)
                )
            )
            self.assertEqual(fighter.action, move.ground)
            self.assertEqual(fighter.action_frame, 1)

        wrong = _Fighter()
        self.assertFalse(
            PKFire().input_pressed(
                wrong, _context(resource="side", stick=(0.0, 1.0))
            )
        )

    def test_entry_rejects_missing_native_animation(self):
        fighter = _AnimationAwareFighter(available=False)
        self.assertFalse(
            PKThunder().input_pressed(
                fighter,
                _context(resource="up", stick=(0.0, 1.0), grounded=False),
            )
        )
        self.assertEqual(fighter.queried_states, [362])
        self.assertEqual(fighter.changes, [])

    def test_source_lifecycle_preserves_surface_and_terminal_destinations(self):
        for move, ground, air in (
            (PKFlash(), PKFlash.ground_end, PKFlash.air_end),
            (PKFire(), PKFire.ground, PKFire.air),
            (PKThunder(), PKThunder.ground_end, None),
            (PSIMagnet(), PSIMagnet.ground_end, PSIMagnet.air_end),
        ):
            grounded = _Fighter(ground)
            move._transition_animation_end(grounded, SimpleNamespace(grounded=True))
            self.assertEqual(grounded.action, Action.WAIT)

            if air is not None:
                airborne = _Fighter(air)
                move._transition_animation_end(airborne, SimpleNamespace(grounded=False))
                self.assertEqual(airborne.action, Action.FALL)

        fighter = _Fighter(PKFlash.air_hold)
        PKFlash()._transition_ground_air(fighter, SimpleNamespace(grounded=True))
        self.assertEqual(fighter.action, PKFlash.ground_hold)

        fighter = _Fighter(PKFire.ground)
        PKFire().ground_to_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, Action.FALL)
        self.assertEqual(PKFire().on_ground, {})
        self.assertEqual(PKFire().on_air, {})

        fighter = _Fighter(PSIMagnet.ground_hold)
        self.assertTrue(PSIMagnet().release(fighter, SimpleNamespace()))
        self.assertEqual(fighter.action, PSIMagnet.ground_end)

    def test_pk_fire_aerial_landing_uses_source_landing_lag(self):
        move = PKFire()
        fighter = _LandingFighter(move.air)
        resource = SimpleNamespace(attributes=SimpleNamespace(x38=12.0))
        ctx = _context(resource="side", grounded=False)
        ctx.resource = lambda path: resource if path == "side" else None

        self.assertTrue(move.landing(fighter, ctx))
        self.assertEqual(fighter.action, move.air)
        self.assertEqual(
            fighter.fall_special,
            {"mobility": 0, "landing_lag": 12.0},
        )

    def test_psi_magnet_release_exits_absorb_and_turn_phases(self):
        move = PSIMagnet()
        for phase, end in (
            (move.ground_hit, move.ground_end),
            (move.ground_turn, move.ground_end),
            (move.air_hit, move.air_end),
            (move.air_turn, move.air_end),
        ):
            fighter = _Fighter(phase)
            self.assertTrue(move.release(fighter, SimpleNamespace()))
            self.assertEqual(fighter.action, end)

    def test_source_animation_callbacks_cover_each_nonterminal_branch(self):
        # These are the branches performed by the source animation callbacks
        # after an animation completes.  Hold phases are native looping
        # motions and therefore intentionally have no terminal transition.
        branches = (
            (PKFlash(), {
                PKFlash.ground_start: PKFlash.ground_hold,
                PKFlash.air_start: PKFlash.air_hold,
                PKFlash.ground_release: PKFlash.ground_end,
                PKFlash.air_release: PKFlash.air_end,
            }),
            (PKThunder(), {
                PKThunder.ground_start: PKThunder.ground_hold,
                PKThunder.air_start: PKThunder.air_hold,
                PKThunder.ground_launch: PKThunder.ground_end,
            }),
            (PSIMagnet(), {
                PSIMagnet.ground_start: PSIMagnet.ground_hold,
                PSIMagnet.air_start: PSIMagnet.air_hold,
                PSIMagnet.ground_hit: PSIMagnet.ground_hold,
                PSIMagnet.air_hit: PSIMagnet.air_hold,
                PSIMagnet.ground_turn: PSIMagnet.ground_hold,
                PSIMagnet.air_turn: PSIMagnet.air_hold,
            }),
        )
        for move, expected in branches:
            for source, target in expected.items():
                fighter = _Fighter(source)
                move._transition_animation_end(
                    fighter, SimpleNamespace(grounded=source in move.on_ground)
                )
                self.assertEqual(fighter.action, target)

    def test_pk_thunder_aerial_completion_uses_source_fall_special(self):
        move = PKThunder()
        resource = SimpleNamespace(attributes=SimpleNamespace(x70=18.0))
        ctx = _context(resource="up", grounded=False)
        ctx.resource = lambda path: resource if path == "up" else None
        for phase in (move.air_end, move.air_launch, move.air_rebound):
            fighter = _LandingFighter(phase)
            self.assertTrue(move.enter_fall_special(fighter, ctx))
            self.assertEqual(
                fighter.fall_special,
                {"mobility": 1, "landing_lag": 18.0},
            )

        resource.attributes.x70 = 0.0
        fighter = _LandingFighter(move.air_launch)
        self.assertTrue(move.enter_fall_special(fighter, ctx))
        self.assertEqual(fighter.action, Action.FALL)

    def test_pk_thunder_surface_changes_only_control_states(self):
        # ftNs_SpecialHi*_Coll has explicit ground/air pairs for startup,
        # control, and end. PK Thunder 2 launch/rebound collision is handled
        # by its article/collision callback and must not be guessed here.
        move = PKThunder()
        for airborne, grounded in (
            (move.air_start, move.ground_start),
            (move.air_hold, move.ground_hold),
            (move.air_end, move.ground_end),
            (move.air_launch, move.ground_launch),
        ):
            fighter = _Fighter(airborne)
            move._transition_ground_air(fighter, SimpleNamespace(grounded=True))
            self.assertEqual(fighter.action, grounded)

        fighter = _Fighter(move.ground_launch)
        move._transition_ground_air(fighter, SimpleNamespace(grounded=False))
        self.assertEqual(fighter.action, move.air_launch)

    def test_pk_flash_end_consumes_special_input_until_terminal_animation_finishes(self):
        # ftNs_SpecialNEnd_IASA is an empty callback.  The terminal release
        # motion therefore remains a Ness special action for its full
        # animation; a fresh B press must not re-enter PK Flash during it.
        move = PKFlash()
        for phase in (move.ground_end, move.air_end):
            fighter = _Fighter(phase)
            self.assertTrue(
                move.input_pressed(
                    fighter,
                    _context(resource="neutral", grounded=phase is move.ground_end),
                )
            )
            self.assertEqual(fighter.action, phase)


if __name__ == "__main__":
    unittest.main()
