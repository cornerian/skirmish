"""Minimal Fox authoring example.

The canonical declaration lives in ``scripts.fighters.fox``. Import it when
you need the complete fighter; subclass it only to demonstrate a small,
intentional override. Ordinary groups supply fresh ``ActionMove`` defaults,
while ``SpecialMoves`` requires all four special entries in a new declaration.
"""

from scripts.fighters.fox import Fox
from skirmish import Action, ActionMove, AerialMoves, register


defaults = AerialMoves()
neutral_override = ActionMove(
    Action.ATTACK_AIR_N, resource="fox.aerial.neutral"
)
custom_aerials = AerialMoves(neutral=neutral_override)
assert defaults.neutral is not neutral_override


@register
class TrainingFox(Fox):
    name = "training_fox"
    aerials = custom_aerials
