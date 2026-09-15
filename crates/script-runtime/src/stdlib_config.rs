use skirmish_pon_runtime::{MaterializedStandardLibrary, StandardLibrary};
use std::{
    fs::File,
    sync::{Arc, OnceLock},
};

const ARCHIVE_ENV: &str = "SKIRMISH_PON_STDLIB_ARCHIVE";
const SHA256_ENV: &str = "SKIRMISH_PON_STDLIB_SHA256";

static CONFIGURED: OnceLock<Option<Arc<MaterializedStandardLibrary>>> = OnceLock::new();

pub fn configure_standard_library(library: MaterializedStandardLibrary) -> Result<(), String> {
    let library = Arc::new(library);
    match CONFIGURED.set(Some(Arc::clone(&library))) {
        Ok(()) => Ok(()),
        Err(_) => {
            let current = CONFIGURED.get().and_then(|value| value.as_ref());
            if current.is_some_and(|current| current.identity_digest() == library.identity_digest())
            {
                Ok(())
            } else {
                Err("Pon standard library identity is already configured".into())
            }
        }
    }
}

pub fn configured_standard_library_identity() -> Option<[u8; 32]> {
    configured().map(|library| library.identity_digest())
}

/// Load the explicitly supplied development/test release before the first
/// program is prepared.  Packaged binaries configure the same global through
/// the CLI; once that has happened, environment variables are deliberately
/// ignored so a child cannot replace the release identity.
pub(crate) fn configure_from_environment_if_needed() -> Result<(), String> {
    if configured().is_some() {
        return Ok(());
    }

    let archive = std::env::var_os(ARCHIVE_ENV);
    let sha256 = std::env::var_os(SHA256_ENV);
    match (archive, sha256) {
        (None, None) => Ok(()),
        (Some(_), None) => Err(format!(
            "{ARCHIVE_ENV} requires {SHA256_ENV} (64 hexadecimal characters)"
        )),
        (None, Some(_)) => Err(format!(
            "{SHA256_ENV} requires {ARCHIVE_ENV} (verified archive path)"
        )),
        (Some(archive), Some(sha256)) => {
            let archive = archive
                .into_string()
                .map_err(|_| format!("{ARCHIVE_ENV} must be valid UTF-8"))?;
            let sha256 = sha256
                .into_string()
                .map_err(|_| format!("{SHA256_ENV} must be valid UTF-8"))?;
            let expected = parse_sha256(&sha256)?;
            let reader = File::open(&archive)
                .map_err(|error| format!("cannot read Pon stdlib archive {archive}: {error}"))?;
            let library = StandardLibrary::from_archive(reader, expected)
                .map_err(|error| format!("invalid Pon stdlib archive {archive}: {error}"))?;
            let cache_root = std::env::temp_dir().join("skirmish-pon-stdlib");
            let materialized = library.materialize(&cache_root).map_err(|error| {
                format!(
                    "cannot materialize Pon stdlib archive {archive} in {}: {error}",
                    cache_root.display()
                )
            })?;
            configure_standard_library(materialized)
        }
    }
}

fn parse_sha256(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "{SHA256_ENV} must contain exactly 64 hexadecimal characters"
        ));
    }
    let mut digest = [0u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .expect("validated hexadecimal digest");
    }
    Ok(digest)
}

pub(crate) fn configured() -> Option<Arc<MaterializedStandardLibrary>> {
    CONFIGURED.get().and_then(|value| value.as_ref().cloned())
}

pub(crate) fn freeze_unconfigured() -> Result<(), String> {
    if CONFIGURED.set(None).is_ok() || CONFIGURED.get().is_some_and(Option::is_none) {
        Ok(())
    } else {
        Err("Pon standard library identity is already configured".into())
    }
}
