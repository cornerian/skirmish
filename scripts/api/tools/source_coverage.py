"""Fast inventory checks for declared source motion states.

This module intentionally checks declaration coverage only.  A state appearing
in a Python script proves that an identity is representable; it does not prove
that callbacks, physics, collisions, or resource data match the decomp.
"""

from __future__ import annotations

import argparse
import ast
from dataclasses import dataclass
from pathlib import Path
import re
from typing import Iterable


_STATE_COMMENT = re.compile(r"(?:/\*|//).*?=\s*(\d+)\s*(?:\*/)?$")


@dataclass(frozen=True, slots=True)
class SourceCoverage:
    script: Path
    decomp: Path
    declared: frozenset[int]
    upstream: frozenset[int]

    @property
    def missing(self) -> frozenset[int]:
        return self.upstream - self.declared

    @property
    def extra(self) -> frozenset[int]:
        return self.declared - self.upstream

    @property
    def complete(self) -> bool:
        return not self.missing


def _integer(value: ast.AST) -> int | None:
    if isinstance(value, ast.Constant) and isinstance(value.value, int) and not isinstance(value.value, bool):
        return value.value
    return None


def declared_states(source: str) -> frozenset[int]:
    """Extract literal ``slippi_state`` declarations from a fighter script."""
    tree = ast.parse(source)
    states: set[int] = set()
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        for keyword in node.keywords:
            if keyword.arg != "slippi_state":
                continue
            value = _integer(keyword.value)
            if value is not None:
                states.add(value)
    # ``source_phase(341)`` retains the source state as its first positional
    # argument.  Restrict this rule to that helper so ordinary action calls do
    # not accidentally treat animation or other positional data as states.
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call) or not isinstance(node.func, ast.Name):
            continue
        if node.func.id != "source_phase" or not node.args:
            continue
        value = _integer(node.args[0])
        if value is not None:
            states.add(value)
    return frozenset(states)


def decomp_states(source: str) -> frozenset[int]:
    """Extract motion-state numbers from pinned decomp table comments."""
    states: set[int] = set()
    for line in source.splitlines():
        match = _STATE_COMMENT.search(line.strip())
        if match:
            states.add(int(match.group(1)))
    return frozenset(states)


def inspect(script: Path, decomp: Path) -> SourceCoverage:
    return SourceCoverage(
        script=script,
        decomp=decomp,
        declared=declared_states(script.read_text(encoding="utf-8")),
        upstream=decomp_states(decomp.read_text(encoding="utf-8")),
    )


def format_report(result: SourceCoverage) -> str:
    status = "complete" if result.complete else "missing declaration states"
    lines = [f"{result.script}: {status}"]
    lines.append(f"  declared: {len(result.declared)}  upstream: {len(result.upstream)}")
    if result.missing:
        lines.append(f"  missing: {', '.join(map(str, sorted(result.missing)))}")
    if result.extra:
        lines.append(f"  extra: {', '.join(map(str, sorted(result.extra)))}")
    return "\n".join(lines)


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("script", type=Path)
    parser.add_argument("decomp", type=Path)
    return parser


def main(argv: Iterable[str] | None = None) -> int:
    args = _parser().parse_args(argv)
    result = inspect(args.script, args.decomp)
    print(format_report(result))
    return 0 if result.complete else 1


if __name__ == "__main__":
    raise SystemExit(main())
