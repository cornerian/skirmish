# Vendored Pon runtime

This directory contains the `pon-runtime` crate from `can1357/pon` at commit
`ab9067dbd2899c64c4d67a4bc27b8ad49472b126`. It is vendored so Skirmish can
apply its import-policy boundary before cached-module acceptance and source
discovery. The upstream workspace and unrelated Pon crates remain external;
the runtime's `pon-gc` dependency stays pinned to the same upstream commit.

The upstream checkout does not carry a standalone license file; source headers
and the upstream repository at the pinned commit are the provenance record.

Local patch: `pon-runtime/src/import.rs` adds `ImportPolicyHook`, its setter,
and one invocation immediately after reading the visible `sys.modules` entry
and before the cached-binding branch or source lookup. The runtime also carries
the local lifetime and lock-order fixes needed by Skirmish's native embedding;
these changes are part of the vendored integration boundary.

The importer also keeps a process-global registry of curated native module
descriptors. Pon module boxes are immortal but do not trace their mutable
namespace values; `gc_held_roots` therefore enumerates each registered
descriptor's current attributes on every collection, including while bundle
module guards temporarily replace `sys.modules` bindings.

The vendored `pon-jit` crate is also from the same upstream commit. Its local
integration owns a process-wide bounded tier-up compiler service and explicit
driver lifetime coordination, including the runtime lock-order/deadlock fix.
Generated external references are marked non-colocated before tier-0, tier-1,
and OSR definitions to request long-range lowering when needed. Set
`PON_TRACE_RELOCS=1` to opt into a diagnostic compilation that prints backend
relocation kinds and targets; it is disabled by default. The local
`pon-jit/tests/far_reloc.rs` regression proves that a deliberately distant
non-colocated external resolves successfully and that forcing the same target
colocated emits the `X86CallPCRel4` bug class. This does not establish a final
fix for every relocation path or full Fox parity; the Fox up/down acceptance
suites remain the integration gate.
