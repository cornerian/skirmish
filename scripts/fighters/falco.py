from skirmish import register as fighter
from fox import Fox, FoxParameters


class FalcoParameters(FoxParameters):
    projectile_kind: str = "falco_laser"


@fighter
class Falco(Fox):
    name = "falco"
    external_ids = (20,)
    parameters = FalcoParameters
    attributes = FalcoParameters
