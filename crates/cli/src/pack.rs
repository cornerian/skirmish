//! Compact binary sibling of the gameplay export's `match-data.json`.
//!
//! `MatchData` (`skirmish::game::data::MatchData`) is plain serde JSON; the
//! exporter's ~380 MB `match-data.json` remains the source of truth (see
//! `docs/gameplay-export.md`). This module adds a lossless binary sibling,
//! `match-data.bin`, produced by `skirmish pack convert` and consumed by
//! every loader that accepts a `--match-data` path
//! (`crates/cli/src/initialization.rs`'s `make-initialization`, and this
//! crate's own `real_parity*` test data discovery): both encode/decode the
//! exact same `MatchData` value, so using a `.bin` file changes load time
//! and size only, never behavior or the JSON schema.
//!
//! ## Format choice: CBOR (`ciborium`), not `bincode`/`postcard`
//!
//! `MatchData`'s tree contains internally tagged enums
//! (`#[serde(tag = "kind", ...)]`, e.g. `CollisionBox` here, and similar
//! enums in `fighter::damage` and `game` itself) and a `#[serde(flatten)]`
//! field (`game::jab`). Both require a `serde::Deserializer` that supports
//! `deserialize_any`, to buffer a value's fields generically and sniff the
//! tag/flattened keys before picking a concrete variant/shape -- something
//! JSON provides but `bincode` and `postcard`, by design, explicitly
//! refuse (`bincode::error::DecodeError::AnyNotSupported`), because
//! neither retains field names or any other self-describing structure on
//! the wire. Worse, many fields use `#[serde(default,
//! skip_serializing_if = "Option::is_none")]`: for JSON this only omits an
//! object key, harmless because JSON fields are looked up by name; a
//! positional format encodes struct fields by call order, so skipping a
//! `None` field during encoding desyncs the byte stream at the very next
//! field, corrupting everything after it (confirmed directly: an earlier
//! version of this module used `bincode`'s own serde bridge directly on
//! `MatchData` and its decode crashed trying to allocate gigabytes for a
//! garbage length-prefixed `String`, the first field after a skipped
//! `Option`). Neither restriction is a Skirmish bug; both are inherent to
//! *any* non-self-describing binary format applied via serde derive, so no
//! choice of `bincode`/`postcard` configuration fixes it -- only a
//! self-describing wire format (one that keeps field names/tags, like JSON
//! does) can round-trip this particular struct graph via its existing
//! derive.
//!
//! CBOR (RFC 8949, via the `ciborium` crate) is exactly that: a
//! self-describing binary format -- maps keep their keys, `deserialize_any`
//! is fully supported -- so `MatchData`'s derived `Serialize`/`Deserialize`
//! work completely unmodified, with no shadow type and no hand-rolled tree
//! walk. It is also a genuine improvement over JSON on both axes this
//! module cares about: numbers are fixed-width binary (an `f32` is written
//! as an IEEE-754 single-precision value, 4 bytes, not re-parsed from
//! decimal text) and there is no text-escaping/whitespace/punctuation
//! overhead, which is why it loads faster *and* takes less space (measured
//! results are in `docs/gameplay-export.md`). A first attempt at this
//! module built a custom fixed-width tree encoding on top of `bincode`
//! (round-tripping through `serde_json::Value` to dodge the `deserialize_
//! any` restriction above); it was smaller than JSON but decoded *slower*
//! than plain JSON parsing, because it forced every value through two
//! generic tree materializations (a hand-rolled value tree, then `serde_
//! json::Value`) before finally reaching `MatchData`'s typed fields.
//! `ciborium` decodes directly into `MatchData` in one pass, like `serde_
//! json::from_slice` does for JSON, with no such intermediate tree.
//!
//! Header (18 bytes, all multi-byte fields little-endian):
//! - `magic`: 4 bytes, `b"SKPK"`.
//! - `format_version`: `u16`, this module's own header/encoding version.
//!   Bump when the header layout or the CBOR encoding changes in a way
//!   that would make an old `.bin` misdecode.
//! - `data_schema`: `u32`, a copy of the encoded `MatchData.schema` field,
//!   for a cheap sanity check without a full decode.
//! - `payload_len`: `u64`, the length in bytes of the encoded CBOR payload
//!   that follows the header.
//!
//! Unlike a positional format, a genuine `MatchData` shape change (a field
//! added/removed/renamed) fails safely and legibly through the normal
//! `#[serde(deny_unknown_fields)]`/missing-field errors CBOR decoding
//! already raises for such a change, the same way JSON decoding would, so
//! this header does not need (and does not carry) a separate schema-hash
//! escape hatch the way a positional format's header would.
use anyhow::{Context, Result, ensure};
use skirmish::game::data::MatchData;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub const MAGIC: &[u8; 4] = b"SKPK";
pub const FORMAT_VERSION: u16 = 1;

const HEADER_LEN: usize = 4 + 2 + 4 + 8;

/// Encode `data` into the `.bin` pack format described in the module doc.
pub fn encode(data: &MatchData) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    ciborium::into_writer(data, &mut payload).context("CBOR encoding of MatchData failed")?;
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&data.schema.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Decode a `.bin` pack produced by [`encode`], validating the header.
pub fn decode(bytes: &[u8]) -> Result<MatchData> {
    ensure!(
        bytes.len() >= HEADER_LEN,
        "pack file is only {} bytes, shorter than the {HEADER_LEN}-byte header",
        bytes.len()
    );
    let (header, rest) = bytes.split_at(HEADER_LEN);
    ensure!(
        &header[0..4] == MAGIC,
        "not a skirmish gameplay pack: missing SKPK magic"
    );
    let format_version = u16::from_le_bytes(header[4..6].try_into().unwrap());
    ensure!(
        format_version == FORMAT_VERSION,
        "unsupported pack format version {format_version}; this build supports {FORMAT_VERSION}"
    );
    let data_schema = u32::from_le_bytes(header[6..10].try_into().unwrap());
    let payload_len = u64::from_le_bytes(header[10..18].try_into().unwrap()) as usize;
    ensure!(
        rest.len() == payload_len,
        "pack payload length mismatch: header says {payload_len} bytes, file has {}",
        rest.len()
    );
    let data: MatchData =
        ciborium::from_reader(rest).context("CBOR decoding of MatchData failed")?;
    ensure!(
        data.schema == data_schema,
        "pack header's data_schema ({data_schema}) does not match the decoded MatchData.schema ({})",
        data.schema
    );
    Ok(data)
}

/// Read and decode a `.bin` pack file at `path`.
pub fn read_bin(path: &Path) -> Result<MatchData> {
    let bytes = fs::read(path).with_context(|| format!("reading pack file {}", path.display()))?;
    decode(&bytes).with_context(|| format!("decoding pack file {}", path.display()))
}

/// Encode `data` and write it to `path`.
pub fn write_bin(path: &Path, data: &MatchData) -> Result<()> {
    let bytes = encode(data)?;
    fs::write(path, bytes).with_context(|| format!("writing pack file {}", path.display()))
}

/// Load a `MatchData` from `path`, dispatching on its extension: `.bin`
/// decodes the compact pack format above, anything else (in practice
/// always `.json`) parses as plain JSON. This is the one place every
/// loader (`make-initialization --match-data`, the `real_parity*` tests'
/// data discovery) goes through, so both extensions behave identically.
pub fn load_match_data(path: &Path) -> Result<MatchData> {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("bin") => read_bin(path),
        _ => {
            let bytes =
                fs::read(path).with_context(|| format!("reading match data {}", path.display()))?;
            serde_json::from_slice(&bytes)
                .with_context(|| format!("parsing match data JSON {}", path.display()))
        }
    }
}

/// Given a pairing directory (e.g. `<SKIRMISH_GAMEPLAY_DATA>/fox-fd`),
/// return the preferred match-data file: `match-data.bin` if present,
/// otherwise `match-data.json`, otherwise `None`. Every gameplay-export
/// consumer that used to hardcode `match-data.json` should discover its
/// path through this function instead.
pub fn discover_match_data(pairing_dir: &Path) -> Option<PathBuf> {
    let bin = pairing_dir.join("match-data.bin");
    if bin.is_file() {
        return Some(bin);
    }
    let json = pairing_dir.join("match-data.json");
    if json.is_file() { Some(json) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> MatchData {
        serde_json::from_str(include_str!(
            "../../../tests/fixtures/game/integration-match.json"
        ))
        .unwrap()
    }

    #[test]
    fn round_trips_the_integration_fixture_bit_exactly() {
        let data = fixture();
        let bytes = encode(&data).unwrap();
        assert_eq!(&bytes[0..4], MAGIC);
        let decoded = decode(&bytes).unwrap();
        assert_eq!(data, decoded);
    }

    #[test]
    fn load_match_data_agrees_between_json_and_bin() {
        let data = fixture();
        let directory = tempfile::tempdir().unwrap();
        let json_path = directory.path().join("match-data.json");
        let bin_path = directory.path().join("match-data.bin");
        fs::write(&json_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
        write_bin(&bin_path, &data).unwrap();

        let from_json = load_match_data(&json_path).unwrap();
        let from_bin = load_match_data(&bin_path).unwrap();
        assert_eq!(data, from_json);
        assert_eq!(data, from_bin);
    }

    #[test]
    fn discover_prefers_bin_over_json() {
        let data = fixture();
        let directory = tempfile::tempdir().unwrap();
        let json_path = directory.path().join("match-data.json");
        fs::write(&json_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();
        assert_eq!(
            discover_match_data(directory.path()),
            Some(json_path.clone())
        );

        let bin_path = directory.path().join("match-data.bin");
        write_bin(&bin_path, &data).unwrap();
        assert_eq!(discover_match_data(directory.path()), Some(bin_path));
    }

    #[test]
    fn rejects_bad_magic() {
        let error = decode(&[0u8; 30]).unwrap_err();
        assert!(error.to_string().contains("SKPK"), "{error}");
    }

    #[test]
    fn rejects_truncated_header() {
        let error = decode(&[0u8; 4]).unwrap_err();
        assert!(error.to_string().contains("header"), "{error}");
    }

    #[test]
    fn rejects_a_corrupted_data_schema_field() {
        let data = fixture();
        let mut bytes = encode(&data).unwrap();
        // Flip the data_schema header field (bytes 6..10) so it no longer
        // matches the encoded MatchData.schema.
        bytes[6] ^= 0xFF;
        let error = decode(&bytes).unwrap_err();
        assert!(error.to_string().contains("data_schema"), "{error}");
    }

    #[test]
    fn preserves_extreme_f32_bit_patterns() {
        let mut data = fixture();
        data.rules.friction_above_walk = f32::MIN_POSITIVE;
        data.rules.walk_accel_taper_gain = -0.0_f32;
        data.rules.fast_fall_threshold = f32::EPSILON;
        let bytes = encode(&data).unwrap();
        let decoded = decode(&bytes).unwrap();
        assert_eq!(
            data.rules.friction_above_walk.to_bits(),
            decoded.rules.friction_above_walk.to_bits()
        );
        assert!(decoded.rules.walk_accel_taper_gain.is_sign_negative());
        assert_eq!(
            data.rules.fast_fall_threshold.to_bits(),
            decoded.rules.fast_fall_threshold.to_bits()
        );
    }
}
