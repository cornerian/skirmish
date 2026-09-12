"""Audit the retail PowerPC binary for fused multiply-add instructions.

MetroWerks CodeWarrior contracts `a*b+c`-shaped float expressions into single
Gekko instructions (`fmadds`/`fmsubs`/`fnmadds`/`fnmsubs`, plus the double-
precision forms without the trailing `s`) that round once, rather than
computing the multiply and the add/sub as two separately-rounded IEEE-754
f32 operations. A straight port of the decomp's C (`a * b + c`) reproduces
the *value* of the expression but not always the *bit pattern*, because Rust
does not contract float arithmetic by default. This tool disassembles named
functions out of the real GameCube DOL and reports every fused instruction
found, so a port can decide, function by function and expression by
expression, where `f32::mul_add` (or a negated form) is needed to match.

Usage:

    uv run --with capstone python3 tools/ppc_fma_audit.py \\
        --dol /path/to/main.dol \\
        --symbols /path/to/symbols.txt \\
        ftCommon_8007C98C ftCommon_ApplyGroundMovement ...

    # Or read one function name per line from a file (blank lines and
    # `#`-prefixed comments are ignored):
    uv run --with capstone python3 tools/ppc_fma_audit.py \\
        --dol main.dol --symbols symbols.txt --functions-file names.txt

Only the Python standard library and `capstone` (invoked via `uv run --with
capstone`, never `pip install`d) are used. Capstone 5's PowerPC disassembler
is queried for each function's raw bytes at their file offset (resolved
through the DOL's own section table, not a hardcoded load address), and the
per-instruction mnemonics are matched against the fused-op set below.
"""
from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

try:
    import capstone
except ImportError as exc:  # pragma: no cover - guidance, not a code path
    raise SystemExit(
        "capstone is required; run this via "
        "`uv run --with capstone python3 tools/ppc_fma_audit.py ...`, "
        "not a bare interpreter"
    ) from exc

# Single- and double-precision fused multiply-add/subtract mnemonics. Capstone
# renders the record-form ('.') suffix (CR1 update) as part of the mnemonic,
# so match by prefix rather than exact string.
FUSED_PREFIXES = (
    "fmadds",   # frD = (frA * frC) + frB, single, one rounding
    "fmsubs",   # frD = (frA * frC) - frB
    "fnmadds",  # frD = -((frA * frC) + frB)
    "fnmsubs",  # frD = -((frA * frC) - frB)
    "fmadd",    # double-precision forms (no trailing 's')
    "fmsub",
    "fnmadd",
    "fnmsub",
)
# fmadds/fmsubs/... all start with one of the double-precision prefixes too
# ("fmadd" is a prefix of "fmadds"), so de-duplicate by checking the longest
# match; see _classify below.

# Other floating point ops worth showing as context around a fused
# instruction: pure multiply/add/sub/div, register moves, loads/stores,
# rounding to single, negation/absolute value, and compares.
CONTEXT_MNEMONIC_RE = re.compile(
    r"^(fmuls?|fadds?|fsubs?|fdivs?|frsp|fmr|fneg|fabs|fnabs|fcmpu|fcmpo|"
    r"lfs|lfd|stfs|stfd|fsel|fctiwz?)\.?$"
)

SYMBOL_LINE_RE = re.compile(
    r"^\s*([A-Za-z_][A-Za-z0-9_]*)\s*=\s*\.text:0x([0-9A-Fa-f]+)\s*;"
    r".*?\bsize:0x([0-9A-Fa-f]+)"
)


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
            import struct

            return struct.unpack(">%dI" % count, raw[offset : offset + 4 * count])

        text_off = unpack_u32(0x00, 7)
        data_off = unpack_u32(0x1C, 11)
        text_addr = unpack_u32(0x48, 7)
        data_addr = unpack_u32(0x64, 11)
        text_size = unpack_u32(0x90, 7)
        data_size = unpack_u32(0xAC, 11)

        text_sections = [
            DolSection(o, a, s)
            for o, a, s in zip(text_off, text_addr, text_size)
            if s
        ]
        data_sections = [
            DolSection(o, a, s)
            for o, a, s in zip(data_off, data_addr, data_size)
            if s
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


def _classify(mnemonic: str) -> str | None:
    """Return the canonical fused-op name for a capstone mnemonic, or None."""
    base = mnemonic.rstrip(".")
    # Longest-prefix match so "fmadds" isn't mis-tagged as bare "fmadd".
    for name in sorted(FUSED_PREFIXES, key=len, reverse=True):
        if base == name:
            return base
    return None


@dataclass
class FusedInstr:
    addr: int
    mnemonic: str
    op_str: str
    context_before: list[str] = field(default_factory=list)
    context_after: list[str] = field(default_factory=list)


@dataclass
class FunctionAudit:
    symbol: Symbol
    instructions: list[tuple[int, str, str]]
    fused: list[FusedInstr]

    def summary_line(self) -> str:
        if not self.fused:
            return f"{self.symbol.name}: no fused ops ({len(self.instructions)} instrs)"
        counts: dict[str, int] = {}
        for f in self.fused:
            counts[f.mnemonic] = counts.get(f.mnemonic, 0) + 1
        parts = ", ".join(f"{k}x{v}" for k, v in sorted(counts.items()))
        return f"{self.symbol.name}: {len(self.fused)} fused op(s) [{parts}]"


def _disasm_resync(md: "capstone.Cs", code: bytes, base_addr: int):
    """Disassemble PPC code, one instruction at a time, resyncing on any word
    capstone's PowerPC backend does not decode.

    Capstone 5's PPC disassembler does not cover every legacy FPU opcode (for
    example `fcmpo`, which appears in this binary); `Cs.disasm` treats such a
    word as the end of the stream rather than skipping it, which would
    silently truncate a function's instruction list well before its real end
    and hide any fused op that happens to follow. PowerPC instructions are
    always 4 bytes, so an undecodable word can simply be recorded as
    `.long` (its raw hex) and skipped, without losing sync with the words
    after it.
    """
    import struct

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


def disassemble_function(dol: Dol, sym: Symbol, context: int) -> FunctionAudit:
    code = dol.read(sym.addr, sym.size)
    md = capstone.Cs(capstone.CS_ARCH_PPC, capstone.CS_MODE_32 + capstone.CS_MODE_BIG_ENDIAN)
    md.detail = False
    instructions = list(_disasm_resync(md, code, sym.addr))
    fused: list[FusedInstr] = []
    for i, (addr, mnemonic, op_str) in enumerate(instructions):
        if _classify(mnemonic) is None:
            continue
        before = [
            f"{a:#010x}  {m} {o}".strip()
            for a, m, o in instructions[max(0, i - context) : i]
        ]
        after = [
            f"{a:#010x}  {m} {o}".strip()
            for a, m, o in instructions[i + 1 : i + 1 + context]
        ]
        fused.append(FusedInstr(addr, mnemonic, op_str, before, after))
    return FunctionAudit(sym, instructions, fused)


def format_report(audit: FunctionAudit) -> str:
    lines = [f"== {audit.symbol.name} @ 0x{audit.symbol.addr:08x} "
             f"(size 0x{audit.symbol.size:x}, {len(audit.instructions)} instrs) =="]
    if not audit.fused:
        lines.append("  (no fused multiply-add/subtract instructions)")
        return "\n".join(lines)
    for f in audit.fused:
        lines.append(f"  0x{f.addr:08x}  {f.mnemonic} {f.op_str}")
        for b in f.context_before:
            lines.append(f"      before: {b}")
        for a in f.context_after:
            lines.append(f"      after:  {a}")
    return "\n".join(lines)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dol", required=True, type=Path, help="path to main.dol")
    parser.add_argument("--symbols", required=True, type=Path,
                         help="path to the decomp's config/GALE01/symbols.txt")
    parser.add_argument("--context", type=int, default=2,
                         help="number of surrounding instructions to print (default 2)")
    parser.add_argument("--functions-file", type=Path,
                         help="file with one function name per line "
                              "(# comments and blank lines ignored)")
    parser.add_argument("--summary-only", action="store_true",
                         help="skip the per-instruction report; print only "
                              "the one-line-per-function summary (useful "
                              "for a first pass over many functions)")
    parser.add_argument("functions", nargs="*", help="function names to audit")
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
        audits.append(disassemble_function(dol, sym, args.context))

    if not args.summary_only:
        for audit in audits:
            print(format_report(audit))
            print()

    print("== summary ==")
    total_fused = 0
    for audit in audits:
        print(f"  {audit.summary_line()}")
        total_fused += len(audit.fused)
    print(f"\n{len(audits)} function(s) audited, {total_fused} fused op(s) total")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
