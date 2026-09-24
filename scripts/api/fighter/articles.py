"""Numeric native article identities exposed to fighter scripts."""

from dataclasses import replace
from enum import IntEnum

from .actions import Action
from .api import Fighter, FighterPart
from .compat import ActionDescriptor, Button, source_phase
from .events import AnimationEventId, Hook, _action_name, on
from .helpers import directional_b_reserved, fresh_b_input, start_action
from .standard import NeutralSpecial
from .transitions import Transition


class ArticleId(IntEnum):
    """Melee article kinds understood by the shared authoring API."""

    MARIO_FIRE = 48
    DR_MARIO_VITAMIN = 49
    FOX_LASER = 54
    FALCO_LASER = 55
    LINK_BOMB = 58
    YOUNG_LINK_BOMB = 59
    LINK_BOOMERANG = 60
    YOUNG_LINK_BOOMERANG = 61
    LINK_HOOKSHOT = 62
    YOUNG_LINK_HOOKSHOT = 63
    LINK_ARROW = 64
    YOUNG_LINK_ARROW = 65
    NESS_PK_FIRE = 66
    NESS_PK_FIRE_FLAME = 67
    NESS_PK_FLASH = 68
    NESS_PK_THUNDER = 69
    NESS_PK_THUNDER_TRAIL_1 = 70
    NESS_PK_THUNDER_TRAIL_2 = 71
    NESS_PK_THUNDER_TRAIL_3 = 72
    NESS_PK_THUNDER_TRAIL_4 = 73
    LINK_BOW = 76
    YOUNG_LINK_BOW = 77
    NESS_PK_FLASH_EXPLOSION = 78
    SAMUS_BOMB = 93
    PEACH_BOMBER = 98
    PEACH_TURNIP = 99
    PEACH_PARASOL = 103
    PEACH_TOAD = 104
    LUIGI_FIRE = 105
    PEACH_TOAD_SPORE = 111

    @property
    def is_native_callback_owned(self) -> bool:
        return (
            58 <= self <= 73
            or 76 <= self <= 78
            or self in (93, 98, 99, 111, 124)
            or 103 <= self <= 104
            or 114 <= self <= 122
        )


def b0_source_phases(
    ground_state: int,
    air_state: int,
    *,
    animation: tuple[int, int] = (295, 296),
    attack: tuple[str, str] = ("neutral.start.ground", "neutral.start.air"),
) -> tuple[ActionDescriptor, ActionDescriptor]:
    """Build the paired source phases used by an article-producing B0.

    The state IDs stay explicit at the call site.  The animation and attack
    defaults describe the common source B0 start pair, while either can be
    overridden for a source variant.  This helper only packages the repeated
    ground/air shape; it does not infer a fighter, article, or behavior.
    """
    if len(animation) != 2 or len(attack) != 2:
        raise ValueError("B0 source phases need one ground and one air value")
    return (
        source_phase(ground_state, animation=animation[0], attack=attack[0]),
        source_phase(air_state, animation=animation[1], attack=attack[1]),
    )


def _clear_command_slot(fighter: Fighter) -> None:
    command = getattr(fighter.action_state, "command", (0, 0, 0, 0))
    if isinstance(command, (tuple, list)) and command:
        fighter.action_state.command = (0, *command[1:])


class B0ArticleSpecial(NeutralSpecial):
    """Generic paired source neutral special whose B0 event creates an article.

    Concrete moves provide paired ``ground`` and ``air`` descriptors.  Their
    numeric state metadata supplies the compatibility ``ground_state`` and
    ``air_state`` attributes; the lifecycle and native article boundary are
    shared here without encoding a fighter's action identity.
    """

    article_id: ArticleId

    def __init_subclass__(cls, **kwargs):
        # Source-owner binding creates a thin subclass around the authored
        # move.  Its article identity is inherited from the authored class.
        article_id = getattr(cls, "article_id", None)
        if not isinstance(article_id, ArticleId):
            raise TypeError(
                f"{cls.__name__}.article_id must be a known ArticleId"
            )
        if not hasattr(cls, "ground") or not hasattr(cls, "air"):
            raise TypeError(
                f"{cls.__name__} must define paired ground and air phases"
            )
        states = tuple(
            dict(phase.metadata)["slippi_state"]
            for phase in (cls.ground, cls.air)
        )
        declared = tuple(
            getattr(cls, name, None) for name in ("ground_state", "air_state")
        )
        if all(value is not None for value in declared) and declared != states:
            raise ValueError(
                f"{cls.__name__} ground/air state metadata does not match phases"
            )
        cls.ground_state, cls.air_state = states
        cls._ACTIVE = (cls.ground, cls.air)
        cls.on_ground = {cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)}
        cls.on_air = {cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)}
        cls.on_end = {cls.ground: Transition(Action.WAIT), cls.air: Transition(Action.FALL)}
        super().__init_subclass__(**kwargs)

    @on.action_enter()
    def enter(self, fighter: Fighter, ctx) -> None:
        _clear_command_slot(fighter)

    @on.input_pressed(Button.B)
    def input_pressed(self, fighter: Fighter, ctx) -> bool:
        # The host may route callbacks without pre-filtering the button.  Keep
        # the fresh-press check first so a held B cannot be consumed repeatedly
        # while an article move is active.
        if not fresh_b_input(ctx):
            return False
        if fighter.action in self._ACTIVE:
            return True
        lookup = getattr(ctx, "resource", None)
        if lookup is None or lookup(self.resource) is None:
            return False
        if directional_b_reserved(ctx):
            return False
        if not (ctx.ground_open or ctx.air_open):
            return False
        grounded = bool(ctx.ground_open)
        phase, state = (
            (self.ground, self.ground_state)
            if grounded else (self.air, self.air_state)
        )
        if not fighter.has_complete_animation(state):
            return False
        start_action(fighter, phase)
        return True

    @on.animation_event(AnimationEventId.B0)
    def spawn(self, fighter: Fighter, ctx) -> None:
        fighter.spawn_article(
            self.article_id,
            fighter.part_position(FighterPart.L1ST_NB),
            fighter.facing,
        )

    @classmethod
    def events(cls):
        actions = tuple(_action_name(action) for action in cls._ACTIVE)
        return tuple(
            replace(binding, actions=actions)
            if binding.hook in (Hook.ACTION_ENTERED, Hook.ANIMATION_EVENT)
            else binding
            for binding in super().events()
        )


__all__ = ["ArticleId", "B0ArticleSpecial", "b0_source_phases"]
