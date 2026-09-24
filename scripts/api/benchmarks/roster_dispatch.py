"""Optional, reproducible timings for roster loading and callback dispatch.

Run from the repository root with ``PYTHONPATH=scripts/api:scripts``.  This is
an executable benchmark, deliberately outside the test suite; it reports
medians and raw samples so results can be copied into performance notes
without implying an unmeasured speed ratio.
"""

from __future__ import annotations

import argparse
import importlib
import statistics
import time
from pathlib import Path
from types import SimpleNamespace
from typing import Any, Callable

from fighter import Button, Fighter, NeutralSpecial, on
from skirmish._loader import dispatch, export


ROOT = Path(__file__).resolve().parents[3]
FIGHTERS = ROOT / "scripts" / "fighters"


class _DispatchProbe(NeutralSpecial):
    """A real exported move callback with no host state dependencies."""

    @on.input_pressed(Button.B)
    def callback(self, fighter: Any, ctx: Any) -> None:
        return None


class _DispatchProbeFighter(Fighter):
    __module__ = "roster_dispatch_probe"
    name = "roster_dispatch_probe"
    external_ids = (0x7FFF,)
    specials = Fighter.specials.replace(neutral=_DispatchProbe())


def _timed(function: Callable[[], Any], repeats: int, warmup: int) -> list[int]:
    for _ in range(warmup):
        function()
    samples: list[int] = []
    for _ in range(repeats):
        start = time.perf_counter_ns()
        function()
        samples.append(time.perf_counter_ns() - start)
    return samples


def _report(label: str, samples: list[int]) -> None:
    ordered = sorted(samples)
    print(f"{label}: median_ns={statistics.median(ordered):.0f} "
          f"min_ns={ordered[0]} max_ns={ordered[-1]}")
    print(f"{label}.raw_ns={','.join(str(value) for value in samples)}")


def _roster_modules() -> list[str]:
    return sorted(path.stem for path in FIGHTERS.glob("*.py") if path.stem != "__init__")


def _load_roster(slug: str) -> dict[str, Any]:
    module = importlib.import_module(f"fighters.{slug}")
    module = importlib.reload(module)
    return export({**vars(module), "__name__": module.__name__})


def _probe_bundle() -> tuple[dict[str, Any], int]:
    namespace = {
        name: value for name, value in vars(importlib.import_module(__name__)).items()
    }
    namespace.update({
        "__name__": _DispatchProbeFighter.__module__,
        "_DispatchProbeFighter": _DispatchProbeFighter,
    })
    bundle = export(namespace)
    index = next(i for i, name in enumerate(bundle["callback_names"]) if name.endswith(".callback"))
    return bundle, index


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeats", type=int, default=25)
    parser.add_argument("--warmup", type=int, default=3)
    args = parser.parse_args()
    if args.repeats <= 0 or args.warmup < 0:
        parser.error("--repeats must be positive and --warmup non-negative")

    modules = _roster_modules()
    _report(
        "roster_load_all",
        _timed(lambda: [_load_roster(slug) for slug in modules], args.repeats, args.warmup),
    )

    bundle, index = _probe_bundle()
    args_for_callback = ({"probe": True}, {"event": SimpleNamespace(value=0)})
    dispatch_samples = _timed(
        lambda: dispatch(bundle, index, args_for_callback),
        args.repeats * 100,
        args.warmup,
    )
    _report("callback_dispatch", dispatch_samples)

    print(f"roster_count={len(modules)} callback_slot={index}")
    print("native_host_comparison=unavailable: no comparable host-C workload is exposed by this command")


if __name__ == "__main__":
    main()
