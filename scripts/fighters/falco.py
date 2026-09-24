"""Falco's fighter declaration.

The pinned ``ftFalco.c`` motion table delegates every special callback to the
shared ``ftFox`` implementation.  Falco's observable script-level difference
is the fighter data and article kind, so the policy objects remain shared while
the state and parameters are named explicitly for the Falco source module.
"""

from skirmish import Action, ArticleId, Fighter, hook

if __package__:
    from .fox import Fox, FoxActionState, FoxParameters, Illusion
else:  # Native source bundles execute each fighter as a top-level module.
    from fox import Fox, FoxActionState, FoxParameters, Illusion


class FalcoIllusion(Illusion):
    """Falco's shared Phantasm callback with the native end transition."""

    @hook.animation_end(*Illusion._DASH_PHASES, Illusion.air_end, Illusion.ground_end)
    def animation_end(self, fighter, ctx) -> None:
        # ftFx_SpecialSEnd_Anim calls the common grounded wait transition when
        # the Phantasm end animation finishes.  Keep it explicit here because
        # the shared policy's callback only covered the aerial fall-special
        # path, leaving Falco's grounded end state without a transition.
        if fighter.action == self.ground_end:
            fighter.change_action(Action.WAIT)
            return
        super().animation_end(fighter, ctx)


class FalcoActionState(FoxActionState):
    """State layout used by ftFalco's shared ftFox special callbacks."""


class FalcoParameters(FoxParameters):
    article_id: ArticleId = ArticleId.FALCO_LASER
    # ftFc_Init_OnLoad (ftfalco.c:468-484) explicitly enables wall jumps
    # before delegating the remaining setup to ftFx_Init_OnLoadForFalco.
    can_walljump: bool = True
    # ftFc_Init_OnLoad installs It_Kind_Falco_Phantasm in the fighter's
    # side-special item slot.  Keep the source identity typed so article
    # bridges cannot accidentally pass Fox's adjacent item kind (56).
    phantasm_article_id: ArticleId = ArticleId.FALCO_PHANTASM


class Falco(Fighter):
    parameters = FalcoParameters
    attributes = FalcoParameters
    action_state = FalcoActionState
    # ftFalco's ftfalco.c motion table points at the shared ftFox callbacks.
    # The host projectile boundary models the laser's actual ECB midpoint
    # spawn; the native hand joint is only the item's previous ray anchor and
    # requires a semantic joint map that this script layer does not expose.
    specials = Fox.specials.replace(side=FalcoIllusion())


__all__ = ["Falco", "FalcoActionState", "FalcoParameters", "FalcoIllusion"]
