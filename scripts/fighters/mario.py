"""Mario's source-backed special motion states.

Mario's side, up, and down callbacks are fighter-local motion handlers in the
pinned ``ftMario`` source.  Their cape and effect/article ownership remains a
host boundary; this module describes the native motion phases and surface
lifecycle exposed by the authoring API.
"""

from skirmish import (
    Action,
    ArticleId,
    B0ArticleSpecial,
    DirectionalSpecial,
    DownSpecial,
    Fighter,
    HitContext,
    SideSpecial,
    Transition,
    UpSpecial,
    b0_source_phases,
    hook,
    source_phase,
)


class Fireball(B0ArticleSpecial):
    article_id = ArticleId.MARIO_FIRE
    ground, air = b0_source_phases(343, 344)


class _SourcePair(DirectionalSpecial):
    """Shared ground/air lifecycle for Mario's source special pairs."""

    on_end = {"ground": Transition(Action.WAIT), "air": Transition(Action.FALL)}

    def __init_subclass__(cls, **kwargs):
        if "ground" in cls.__dict__ and "air" in cls.__dict__:
            cls._ACTIVE = (cls.ground, cls.air)
            cls.on_end = {
                cls.ground: Transition(Action.WAIT),
                cls.air: Transition(Action.FALL),
            }
            cls.on_ground = {
                cls.air: Transition(cls.ground, preserve_state=True, keep_frame=True)
            }
            cls.on_air = {
                cls.ground: Transition(cls.air, preserve_state=True, keep_frame=True)
            }
        super().__init_subclass__(**kwargs)


class Cape(SideSpecial, _SourcePair):
    ground = source_phase(345)
    air = source_phase(346)

    @hook.projectile_contact
    def projectile_contact(self, fighter: Fighter, hit: HitContext) -> None:
        """Mirror the native cape reflect callback's eligibility gate.

        The host owns the animation command that enables ``reflecting`` and
        the projectile handoff itself.  Mario's fighter callback only accepts
        projectile contacts while that flag is active and while the incoming
        damage is within the cape reflection descriptor's maximum.
        """
        if fighter.flags.reflecting and hit.projectile and hit.damage <= hit.max_damage:
            hit.reflect = True


class SuperJumpPunch(UpSpecial, _SourcePair):
    ground = source_phase(347)
    air = source_phase(348)


class MarioTornado(DownSpecial, _SourcePair):
    ground = source_phase(349)
    air = source_phase(350)


class Mario(Fighter):
    specials = Fighter.specials.replace(
        neutral=Fireball(),
        side=Cape(),
        up=SuperJumpPunch(),
        down=MarioTornado(),
    )


__all__ = ["Mario", "Fireball", "Cape", "SuperJumpPunch", "MarioTornado"]
