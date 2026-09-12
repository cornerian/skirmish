"""Audit the retail PowerPC binary for double-precision arithmetic hiding
inside nominally-`float` expressions.

`tools/ppc_fma_audit.py` (the `skirmish-fma` batch) already disassembled the
retail binary looking for *fused* multiply-add instructions (`fmadds` and
friends) and found none in the dash/ground-movement/friction/fall/air-dodge
chain. But "no fused op" is not the same claim as "every op is single
precision, separately rounded" -- that batch's own tool only prints
instructions in a window around a fused op; for a function with zero fused
ops it never prints the instruction stream at all, so whether the
individual (non-fused) `fmul`/`fadd`/`fsub`/`fdiv` ops used their
single-precision suffixed forms (`fmuls`/`fadds`/`fsubs`/`fdivs`, one
rounding, each op) or the plain double-precision forms (`fmul`/`fadd`/
`fsub`/`fdiv`, computed and left at double precision until something rounds
it down) was never actually checked instruction by instruction. On PowerPC,
FPRs always hold the IEEE double format; a "single" op is architecturally
defined as "compute at double precision, then round once to single" -- so a
plain `fmul` followed by a plain `fadds` rounds only once (at the add) where
a strict single-precision port that does `(a * b)` then `+ c` as two
separately-rounded f32 operations would round twice, and the two are not
guaranteed to agree bit for bit (classic "double rounding"). This tool finds
every such site: every double-precision arithmetic instruction (`fmul`,
`fadd`, `fsub`, `fdiv`, and the double-precision fused forms `fmadd`/
`fmsub`/`fnmadd`/`fnmsub`), every `lfd` (loads a double -- either a real
double constant/global or a double reached via `frsp`-free promotion), and
every `frsp` (the actual, explicit single-rounding point), so the exact
place double precision starts and ends is visible per function.

Usage mirrors `ppc_fma_audit.py`:

    uv run --with capstone python3 tools/ppc_precision_audit.py \\
        --dol /path/to/main.dol \\
        --symbols /path/to/symbols.txt \\
        ftCo_Dash_Phys ftCommon_8007C98C ...

    uv run --with capstone python3 tools/ppc_precision_audit.py \\
        --dol main.dol --symbols symbols.txt --functions-file names.txt \\
        --summary-only

Only the standard library and `capstone` (via `uv run --with capstone`) are
used, exactly like `ppc_fma_audit.py`; the DOL section-table parsing, symbol
table loading, and the undecodable-opcode resync workaround (Capstone 5's
PPC backend does not decode `fcmpo`) are duplicated from that tool rather
than imported, so this file stands alone.
"""
from __future__ import annotations

import argparse
import re
import struct
import sys
from dataclasses import dataclass, field
from pathlib import Path

try:
    import capstone
except ImportError as exc:  # pragma: no cover - guidance, not a code path
    raise SystemExit(
        "capstone is required; run this via "
        "`uv run --with capstone python3 tools/ppc_precision_audit.py ...`, "
        "not a bare interpreter"
    ) from exc

SYMBOL_LINE_RE = re.compile(
    r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\.text:0x([0-9A-Fa-f]+)\s*;"
    r".*?\bsize:0x([0-9A-Fa-f]+)"
)

# Double-precision arithmetic: computed and rounded to *double* precision.
# On the Gekko/Broadway (PowerPC 750-family) FPU, FPRs always store the IEEE
# double format; these mnemonics (no trailing 's') round their result to the
# nearest representable *double*, not single -- the single-rounding-to-f32
# step only happens at `frsp`, `stfs`, or a suffixed ('s') op.
DOUBLE_ARITH = {"fmul", "fadd", "fsub", "fdiv", "fmadd", "fmsub", "fnmadd", "fnmsub"}
# Single-precision arithmetic: double-precision compute, one rounding to f32.
SINGLE_ARITH = {
    "fmuls", "fadds", "fsubs", "fdivs",
    "fmadds", "fmsubs", "fnmadds", "fnmsubs",
}
# Everything else worth classifying explicitly.
OTHER_CLASSIFIED = {
    "frsp": "round-to-single (explicit single-rounding point)",
    "lfd": "load double (8-byte, from memory)",
    "lfs": "load single (4-byte, promoted losslessly to double in the FPR)",
    "stfd": "store double (8-byte)",
    "stfs": "store single (rounds to single on the way out, if source wasn't already)",
    "fmr": "register move (no rounding)",
    "fneg": "negate (exact, no rounding)",
    "fabs": "absolute value (exact, no rounding)",
    "fnabs": "negated absolute value (exact, no rounding)",
    "fcmpu": "compare (no rounding)",
    "fcmpo": "compare, ordered (no rounding)",
    "fsel": "select (no rounding)",
    "fctiwz": "convert to integer, round toward zero",
}


@dataclass
class Symbol:
    name: str
    addr: int
    size: int


@dataclass
class DolSection:
    offset: int
    addr: int
    size: int

    def contains(self, addr: int) -> bool:
        return self.size > 0 and self.addr <= addr < self.addr + self.size

    def file_offset(self, addr: int) -> int:
        return self.offset + (addr - self.addr)


@dataclass
class Dol:
    text_sections: list[DolSection]
    data_sections: list[DolSection]
    raw: bytes

    @classmethod
    def load(cls, path: Path) -> "Dol":
        raw = path.read_bytes()
        if len(raw) < 0x100:
            raise ValueError(f"{path} is too small to be a DOL")

        def unpack_u32(offset: int, count: int) -> tuple[int, ...]:
            return struct.unpack(">%dI" % count, raw[offset : offset + 4 * count])

        text_off = unpack_u32(0x00, 7)
        data_off = unpack_u32(0x1C, 11)
        text_addr = unpack_u32(0x48, 7)
        data_addr = unpack_u32(0x64, 11)
        text_size = unpack_u32(0x90, 7)
        data_size = unpack_u32(0xAC, 11)

        text_sections = [
            DolSection(o, a, s) for o, a, s in zip(text_off, text_addr, text_size) if s
        ]
        data_sections = [
            DolSection(o, a, s) for o, a, s in zip(data_off, data_addr, data_size) if s
        ]
        return cls(text_sections, data_sections, raw)

    def read(self, addr: int, size: int) -> bytes:
        for section in self.text_sections + self.data_sections:
            if section.contains(addr):
                if addr + size > section.addr + section.size:
                    raise ValueError(
                        f"0x{addr:08x}+0x{size:x} runs past the end of the "
                        f"section containing it (addr 0x{section.addr:08x} "
                        f"size 0x{section.size:x})"
                    )
                start = section.file_offset(addr)
                return self.raw[start : start + size]
        raise ValueError(f"0x{addr:08x} is not inside any DOL text/data section")


def load_symbols(path: Path) -> dict[str, Symbol]:
    symbols: dict[str, Symbol] = {}
    for line in path.read_text().splitlines():
        m = SYMBOL_LINE_RE.match(line)
        if not m:
            continue
        name, addr_hex, size_hex = m.groups()
        symbols[name] = Symbol(name, int(addr_hex, 16), int(size_hex, 16))
    return symbols


def _disasm_resync(md: "capstone.Cs", code: bytes, base_addr: int):
    """Same resync workaround as ppc_fma_audit.py: Capstone 5's PPC backend
    does not decode every legacy FPU opcode (`fcmpo` in particular), and
    treats an undecodable word as end-of-stream rather than skipping it.
    PowerPC instructions are fixed 4 bytes, so record undecoded words as
    `.long` and keep advancing instead of losing the rest of the function.
    """
    offset = 0
    n = len(code)
    while offset + 4 <= n:
        addr = base_addr + offset
        chunk = code[offset : offset + 4]
        insn = next(md.disasm(chunk, addr), None)
        if insn is not None and insn.size > 0:
            yield (insn.address, insn.mnemonic, insn.op_str)
            offset += insn.size
        else:
            word = struct.unpack(">I", chunk)[0]
            yield (addr, ".long", f"0x{word:08x}  ; undecoded by capstone's PPC backend")
            offset += 4


@dataclass
class Instr:
    addr: int
    mnemonic: str
    op_str: str
    kind: str  # "double-arith" | "single-arith" | "other" | ""


def _classify(mnemonic: str) -> str:
    base = mnemonic.rstrip(".")
    if base in DOUBLE_ARITH:
        return "double-arith"
    if base in SINGLE_ARITH:
        return "single-arith"
    if base in OTHER_CLASSIFIED:
        return "other"
    return ""


@dataclass
class FunctionAudit:
    symbol: Symbol
    instructions: list[Instr]

    @property
    def double_arith(self) -> list[Instr]:
        return [i for i in self.instructions if i.kind == "double-arith"]

    @property
    def single_arith(self) -> list[Instr]:
        return [i for i in self.instructions if i.kind == "single-arith"]

    @property
    def frsp(self) -> list[Instr]:
        return [i for i in self.instructions if i.mnemonic.rstrip(".") == "frsp"]

    @property
    def lfd(self) -> list[Instr]:
        return [i for i in self.instructions if i.mnemonic.rstrip(".") == "lfd"]

    @property
    def lfs(self) -> list[Instr]:
        return [i for i in self.instructions if i.mnemonic.rstrip(".") == "lfs"]

    def summary_line(self) -> str:
        d, s = len(self.double_arith), len(self.single_arith)
        if d == 0:
            verdict = "single-precision only"
        else:
            verdict = f"** {d} DOUBLE-PRECISION arith op(s) **"
        return (
            f"{self.symbol.name}: {len(self.instructions)} instrs, "
            f"{s} single-arith, {d} double-arith, "
            f"{len(self.frsp)} frsp, {len(self.lfd)} lfd, {len(self.lfs)} lfs "
            f"-- {verdict}"
        )


def disassemble_function(dol: Dol, sym: Symbol) -> FunctionAudit:
    code = dol.read(sym.addr, sym.size)
    md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_32 + capstone.CS_MODE_BIG_ENDIAN)
    md.detail = False
    instrs = [
        Instr(addr, mnemonic, op_str, _classify(mnemonic))
        for addr, mnemonic, op_str in _disasm_resync(md, code, sym.addr)
    ]
    return FunctionAudit(sym, instrs)


def format_report(audit: FunctionAudit, context: int) -> str:
    lines = [
        f"== {audit.symbol.name} @ 0x{audit.symbol.addr:08x} "
        f"(size 0x{audit.symbol.size:x}, {len(audit.instructions)} instrs) =="
    ]
    interesting = {"double-arith", "single-arith"} | {
        i for i, ins in enumerate(audit.instructions)
        if ins.mnemonic.rstrip(".") in ("frsp", "lfd")
    }
    any_shown = False
    for i, ins in enumerate(audit.instructions):
        base = ins.mnemonic.rstrip(".")
        show = ins.kind in ("double-arith",) or base in ("frsp", "lfd")
        if not show:
            continue
        any_shown = True
        tag = {
            "double-arith": "[DOUBLE]",
            "single-arith": "[single]",
        }.get(ins.kind, {"frsp": "[frsp]", "lfd": "[lfd]"}.get(base, ""))
        lines.append(f"  0x{ins.addr:08x}  {tag:9s} {ins.mnemonic} {ins.op_str}".rstrip())
        lo = max(0, i - context)
        hi = min(len(audit.instructions), i + context + 1)
        for j in range(lo, hi):
            if j == i:
                continue
            a2, m2, o2 = audit.instructions[j].addr, audit.instructions[j].mnemonic, audit.instructions[j].op_str
            marker = "before" if j < i else "after "
            lines.append(f"      {marker}: 0x{a2:08x}  {m2} {o2}".rstrip())
    if not any_shown:
        lines.append("  (no double-precision arith, frsp, or lfd -- pure single-precision f32 throughout)")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dol", required=True, type=Path)
    parser.add_argument("--symbols", required=True, type=Path)
    parser.add_argument("--context", type=int, default=2)
    parser.add_argument("--functions-file", type=Path)
    parser.add_argument("--summary-only", action="store_true")
    parser.add_argument("functions", nargs="*")
    args = parser.parse_args(argv)

    names = list(args.functions)
    if args.functions_file:
        for line in args.functions_file.read_text().splitlines():
            line = line.split("#", 1)[0].strip()
            if line:
                names.append(line)
    if not names:
        parser.error("no function names given (positionally or via --functions-file)")

    dol = Dol.load(args.dol)
    symbols = load_symbols(args.symbols)

    missing = [n for n in names if n not in symbols]
    if missing:
        print(f"warning: not found in symbols.txt: {', '.join(missing)}", file=sys.stderr)

    audits = []
    for name in names:
        sym = symbols.get(name)
        if sym is None:
            continue
        if sym.size == 0:
            print(f"warning: {name} has size 0 in symbols.txt, skipping", file=sys.stderr)
            continue
        audits.append(disassemble_function(dol, sym))

    if not args.summary_only:
        for audit in audits:
            print(format_report(audit, args.context))
            print()

    print("== summary ==")
    total_double = 0
    for audit in audits:
        print(f"  {audit.summary_line()}")
        total_double += len(audit.double_arith)
    print(f"\n{len(audits)} function(s) audited, {total_double} double-precision arith op(s) total")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
