"""Backward-compatible module path for the class based fighter API.

Historically source bundles imported declarations from ``skirmish.api``.
The implementation moved to the ``fighter`` package, but importing only
``fighter.api`` leaves out the public compatibility descriptors and helpers
which are assembled by ``fighter.__init__``.  Re-export that canonical surface
so both paths have the same objects and behavior.
"""

from fighter import *  # noqa: F401,F403
from fighter import __all__ as _FIGHTER_ALL

__all__ = list(_FIGHTER_ALL)
