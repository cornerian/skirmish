//! Verified release reader for Pon's pinned pure-Python standard library.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Cursor, Read},
    path::Path,
    sync::Arc,
};

use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tar::Archive;

use crate::{Error, MaterializedBundle, SourceBundle};

const PON_REVISION: &str = "ab9067dbd2899c64c4d67a4bc27b8ad49472b126";
const CPYTHON_REVISION: &str = "v3.14.0";
const FORMAT: &str = "skirmish-pon-stdlib-v1";
const MAX_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FILES: usize = 100_000;

#[derive(Clone, Debug)]
pub struct StandardLibrary {
    bundle: SourceBundle,
    identity: [u8; 32],
}

#[derive(Clone, Debug)]
pub struct MaterializedStandardLibrary {
    inner: MaterializedBundle,
    identity: [u8; 32],
}

impl StandardLibrary {
    /// Read, authenticate, and validate a deterministic `.tar.gz` release.
    /// No archive entry is written to disk until the entire release is valid.
    pub fn from_archive(
        mut reader: impl Read,
        expected_archive_sha256: [u8; 32],
    ) -> Result<Self, Error> {
        let mut archive_bytes = Vec::new();
        let mut limited = (&mut reader).take((MAX_ARCHIVE_BYTES + 1) as u64);
        limited.read_to_end(&mut archive_bytes).map_err(io_error)?;
        if archive_bytes.len() > MAX_ARCHIVE_BYTES {
            return Err(invalid("archive exceeds size limit"));
        }
        let archive_digest: [u8; 32] = Sha256::digest(&archive_bytes).into();
        if archive_digest != expected_archive_sha256 {
            return Err(invalid("archive SHA-256 does not match expected release"));
        }

        let decoder = GzDecoder::new(Cursor::new(archive_bytes));
        let mut archive = Archive::new(decoder);
        let mut manifest_bytes = None;
        let mut license_bytes = None;
        let mut sources = BTreeMap::<String, Vec<u8>>::new();
        let mut seen = BTreeSet::new();
        let mut total_source_bytes = 0u64;

        let entries = archive.entries().map_err(io_error)?;
        for entry_result in entries {
            let mut entry = entry_result.map_err(io_error)?;
            let path = std::str::from_utf8(entry.path_bytes().as_ref())
                .map_err(|_| invalid("archive entry path is not UTF-8"))?
                .to_owned();
            validate_archive_path(&path)?;
            if !seen.insert(path.clone()) {
                return Err(invalid(format!("duplicate archive entry `{path}`")));
            }
            if seen.len() > MAX_FILES {
                return Err(invalid("archive contains too many entries"));
            }
            if !entry.header().entry_type().is_file() {
                return Err(invalid(format!(
                    "archive entry `{path}` is not a regular file"
                )));
            }
            let size = entry.size();
            let limit = if path == "manifest.json" {
                MAX_MANIFEST_BYTES
            } else {
                MAX_FILE_BYTES
            };
            if size > limit {
                return Err(invalid(format!(
                    "archive entry `{path}` exceeds size limit"
                )));
            }
            let mut bytes = Vec::with_capacity(size as usize);
            entry.read_to_end(&mut bytes).map_err(io_error)?;
            if bytes.len() as u64 != size {
                return Err(invalid(format!(
                    "archive entry `{path}` has truncated contents"
                )));
            }
            match path.as_str() {
                "manifest.json" => manifest_bytes = Some(bytes),
                "LICENSE" => license_bytes = Some(bytes),
                path if path.strip_prefix("stdlib/").is_some() => {
                    let relative = path.strip_prefix("stdlib/").unwrap();
                    validate_source_path(relative)?;
                    total_source_bytes = total_source_bytes
                        .checked_add(size)
                        .ok_or_else(|| invalid("source size overflow"))?;
                    if total_source_bytes > MAX_TOTAL_SOURCE_BYTES {
                        return Err(invalid("source files exceed total size limit"));
                    }
                    sources.insert(relative.to_owned(), bytes);
                }
                _ => return Err(invalid(format!("unlisted archive entry `{path}`"))),
            }
        }

        let manifest_bytes =
            manifest_bytes.ok_or_else(|| invalid("archive is missing manifest.json"))?;
        let license_bytes = license_bytes.ok_or_else(|| invalid("archive is missing LICENSE"))?;
        let manifest: Manifest = serde_json::from_slice(&manifest_bytes)
            .map_err(|error| invalid(format!("invalid manifest.json: {error}")))?;
        validate_manifest(&manifest, &sources, &license_bytes)?;

        let identity_payload = IdentityPayload {
            pon_revision: &manifest.pon_revision,
            cpython_revision: &manifest.cpython_revision,
            license: &manifest.license,
            files: &manifest.files,
        };
        let mut identity_json = serde_json::to_vec(&identity_payload)
            .map_err(|error| invalid(format!("cannot encode manifest identity: {error}")))?;
        identity_json.push(b'\n');
        let identity: [u8; 32] = Sha256::digest(identity_json).into();
        let declared_identity = decode_hex_digest(&manifest.identity)?;
        if identity != declared_identity {
            return Err(invalid("manifest identity does not match its records"));
        }

        let version = format!(
            "pon-stdlib-{}-{}",
            manifest.cpython_revision, manifest.identity
        );
        let mut bundle = SourceBundle::new(Arc::<str>::from(version));
        for (path, bytes) in sources {
            let source = String::from_utf8(bytes)
                .map_err(|_| invalid(format!("source file `{path}` is not UTF-8")))?;
            bundle = bundle.with_file(path, Arc::<str>::from(source))?;
        }
        Ok(Self { bundle, identity })
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        self.identity
    }

    pub fn materialize(
        &self,
        cache_root: impl AsRef<Path>,
    ) -> Result<MaterializedStandardLibrary, Error> {
        Ok(MaterializedStandardLibrary {
            inner: self.bundle.materialize(cache_root)?,
            identity: self.identity,
        })
    }
}

impl MaterializedStandardLibrary {
    pub fn root(&self) -> &Path {
        self.inner.root()
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        self.identity
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    pon_revision: String,
    cpython_revision: String,
    root: String,
    file_count: usize,
    license: Record,
    files: Vec<Record>,
    identity: String,
    excluded: Excluded,
}

#[derive(Debug, Deserialize, Serialize)]
struct Record {
    bytes: u64,
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct Excluded {
    directories: Vec<String>,
    suffixes: Vec<String>,
    files: Vec<String>,
}

#[derive(Serialize)]
struct IdentityPayload<'a> {
    cpython_revision: &'a str,
    files: &'a [Record],
    license: &'a Record,
    pon_revision: &'a str,
}

fn validate_manifest(
    manifest: &Manifest,
    sources: &BTreeMap<String, Vec<u8>>,
    license_bytes: &[u8],
) -> Result<(), Error> {
    if manifest.format != FORMAT
        || manifest.pon_revision != PON_REVISION
        || manifest.cpython_revision != CPYTHON_REVISION
        || manifest.root != "stdlib"
    {
        return Err(invalid(
            "manifest is for an unsupported Pon or CPython release",
        ));
    }
    if manifest.file_count != manifest.files.len() || manifest.files.len() != sources.len() {
        return Err(invalid(
            "manifest file_count does not match archive contents",
        ));
    }
    if manifest.license.path != "LICENSE" {
        return Err(invalid("manifest license path must be LICENSE"));
    }
    validate_record(&manifest.license, license_bytes, "LICENSE")?;
    let mut listed = BTreeSet::new();
    for record in &manifest.files {
        validate_source_path(&record.path)?;
        if !listed.insert(record.path.clone()) {
            return Err(invalid(format!(
                "duplicate manifest file `{}`",
                record.path
            )));
        }
        let bytes = sources.get(&record.path).ok_or_else(|| {
            invalid(format!(
                "manifest file missing from archive `{}`",
                record.path
            ))
        })?;
        validate_record(record, bytes, &record.path)?;
    }
    if listed != sources.keys().cloned().collect() {
        return Err(invalid("archive contains an unlisted source file"));
    }
    let _ = (
        &manifest.excluded.directories,
        &manifest.excluded.suffixes,
        &manifest.excluded.files,
    );
    Ok(())
}

fn validate_record(record: &Record, bytes: &[u8], label: &str) -> Result<(), Error> {
    let actual_digest: [u8; 32] = Sha256::digest(bytes).into();
    if record.bytes != bytes.len() as u64 || decode_hex_digest(&record.sha256)? != actual_digest {
        return Err(invalid(format!("hash or size mismatch for `{label}`")));
    }
    Ok(())
}

fn validate_archive_path(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.contains('\\')
        || path.starts_with('/')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(invalid(format!("unsafe archive path `{path}`")));
    }
    Ok(())
}

fn validate_source_path(path: &str) -> Result<(), Error> {
    validate_archive_path(path)?;
    if !path.ends_with(".py") {
        return Err(invalid(format!("source path is not Python: `{path}`")));
    }
    Ok(())
}

fn decode_hex_digest(value: &str) -> Result<[u8; 32], Error> {
    if value.len() != 64 {
        return Err(invalid("digest must contain 64 hexadecimal characters"));
    }
    let mut digest = [0u8; 32];
    for (index, chunk) in value.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        digest[index] = (hex(chunk[0])? << 4) | hex(chunk[1])?;
    }
    Ok(digest)
}

fn hex(value: u8) -> Result<u8, Error> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(invalid("digest contains non-hexadecimal characters")),
    }
}

fn invalid(message: impl Into<String>) -> Error {
    Error::Value(message.into())
}

fn io_error(error: std::io::Error) -> Error {
    Error::Io(error.to_string())
}
