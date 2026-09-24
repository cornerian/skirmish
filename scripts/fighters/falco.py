"""Falco's fighter declaration.

The pinned ``ftFalco.c`` motion table delegates every special callback to the
shared ``ftFox`` implementation.  Falco's observable script-level difference
is the fighter data and article kind, so the policy objects remain shared while
the state and parameters are named explicitly for the Falco source module.
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


class Falco(Fighter):
    parameters = FalcoParameters
    attributes = FalcoParameters
    action_state = FalcoActionState
    specials = Fox.specials


__all__ = ["Falco", "FalcoActionState", "FalcoParameters"]
