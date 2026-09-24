"""Falco's fighter declaration.

The pinned ``ftFalco.c`` motion table delegates every special callback to the
shared ``ftFox`` implementation.  Falco's observable script-level differences
are fighter data and article kinds, so the policy objects remain shared while
the state and parameters are named explicitly for the Falco source module.
The native blaster callback's transformed ``FtPart_RThumbNb`` launch point is
left to the article-aware host boundary; this definition keeps the shared
callback until that joint mapping is available.
"""

from skirmish import ArticleId, Fighter

if __package__:
    from .fox import Fox, FoxActionState, FoxParameters
else:  # Native source bundles execute each fighter as a top-level module.
    from fox import Fox, FoxActionState, FoxParameters


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
    specials = Fox.specials


__all__ = ["Falco", "FalcoActionState", "FalcoParameters"]
