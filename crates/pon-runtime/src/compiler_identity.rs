//! Identity of the vendored Pon compiler/runtime boundary.
//!
//! This is deliberately separate from the pure-Python standard-library
//! identity.  A stdlib release can remain valid while a patched importer or
//! bridge ABI makes compiled code and continuation checkpoints incompatible.

use sha2::{Digest, Sha256};
use std::sync::OnceLock;

const UPSTREAM_PON_REVISION: &str = "ab9067dbd2899c64c4d67a4bc27b8ad49472b126";
const IDENTITY_FORMAT: &str = "skirmish-pon-compiler-v1";
/// Version 2 includes the explicit MoveLifetime/owner selection and the
/// sequential driver callback contract.
pub const BRIDGE_ABI_VERSION: u32 = 2;
pub const CONTINUATION_ABI_VERSION: u32 = 2;

/// Build-time deterministic serialization of vendored runtime/compiler source
/// trees and manifests. The build script picks up `pon-jit` when relocation
/// supplies it, without changing this identity algorithm.
const VENDORED_SOURCES: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/compiler_sources.bin"));

/// Return the identity used by compiled-program caches and checkpoints.
pub fn compiler_identity() -> [u8; 32] {
    static IDENTITY: OnceLock<[u8; 32]> = OnceLock::new();
    *IDENTITY.get_or_init(|| {
        let mut hash = Sha256::new();
        hash.update(IDENTITY_FORMAT.as_bytes());
        hash.update([0]);
        hash.update((UPSTREAM_PON_REVISION.len() as u64).to_le_bytes());
        hash.update(UPSTREAM_PON_REVISION.as_bytes());
        hash.update(BRIDGE_ABI_VERSION.to_le_bytes());
        hash.update(CONTINUATION_ABI_VERSION.to_le_bytes());
        hash.update((VENDORED_SOURCES.len() as u64).to_le_bytes());
        hash.update(VENDORED_SOURCES);
        hash.finalize().into()
    })
}

#[cfg(test)]
mod tests {
    use super::compiler_identity;

    #[test]
    fn compiler_identity_is_deterministic_and_nonzero() {
        let first = compiler_identity();
        assert_eq!(first, compiler_identity());
        assert_ne!(first, [0; 32]);
    }

    #[test]
    fn program_cache_identity_consumes_compiler_identity() {
        let first = crate::Program::new("x = 1", "a.py", ["x"]);
        let second = crate::Program::new("x = 2", "a.py", ["x"]);
        assert_ne!(first.identity_digest(), second.identity_digest());
        assert_eq!(first.compiler_identity(), compiler_identity());
    }
}
