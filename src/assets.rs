//! Optional offline disc import. Gameplay never opens an ISO or runs a decoder.
//! The installed bundle retains original files, with a checked path resolver;
//! it is not a converted visual scene or a complete native MatchData resource.

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

pub const ISO_SHA256: &str = "0de05981a34156b9cedcef73c73d4244ac05cf6149ab3c9cfed917698819e464";
pub const BUNDLE_NAME: &str = "melee-usa-1.02";
const SCHEMA: &str = "skirmish-iso-import-v1";
const CHUNK: usize = 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    Searching,
    Validating { percent: u8 },
    Extracting { completed: usize, total: usize },
    CheckingInstalled { completed: usize, total: usize },
}

#[derive(Clone, Debug)]
pub enum Source {
    File(PathBuf),
    Search(Vec<PathBuf>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetFile {
    pub path: String,
    pub offset: u64,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    schema: String,
    iso_sha256: String,
    files: Vec<AssetFile>,
    limitations: Vec<String>,
}

#[derive(Debug)]
pub struct AssetBundle {
    root: PathBuf,
    manifest: Manifest,
}

impl AssetBundle {
    /// Inspect a previously installed bundle without an ISO. Manifest paths are
    /// validated before they can be used, including bundles imported externally.
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("Opening installed asset folder")?;
        let file = File::open(root.join("manifest.json")).context("Opening asset manifest")?;
        ensure!(
            file.metadata()?.len() <= 8 * 1024 * 1024,
            "Asset manifest is too large"
        );
        let manifest: Manifest = serde_json::from_reader(file).context("Reading asset manifest")?;
        ensure!(
            manifest.schema == SCHEMA && manifest.iso_sha256 == ISO_SHA256,
            "Unsupported asset bundle; import a Melee USA 1.02 ISO"
        );
        ensure!(!manifest.files.is_empty(), "Asset bundle has no files");
        let mut names = HashSet::new();
        for entry in &manifest.files {
            validate_path(&entry.path)?;
            ensure!(
                names.insert(entry.path.to_ascii_lowercase()),
                "Duplicate asset path"
            );
            ensure!(
                entry.sha256.len() == 64 && entry.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid asset digest"
            );
        }
        let bundle = Self { root, manifest };
        for entry in bundle.files() {
            let path = bundle.resolve(&entry.path)?;
            ensure!(
                path.metadata()?.len() == entry.size,
                "Asset size mismatch: {}",
                entry.path
            );
        }
        Ok(bundle)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn files(&self) -> &[AssetFile] {
        &self.manifest.files
    }

    /// Resolve an exact original disc identifier, e.g. `PlFx.dat`. The canonical
    /// containment check also rejects symlinks escaping the installed bundle.
    pub fn resolve(&self, id: &str) -> Result<PathBuf> {
        validate_path(id)?;
        ensure!(
            self.files().iter().any(|entry| entry.path == id),
            "Unknown asset: {id}"
        );
        let path = self
            .root
            .join("disc/files")
            .join(id)
            .canonicalize()
            .with_context(|| format!("Missing installed asset: {id}"))?;
        ensure!(
            path.starts_with(self.root.join("disc/files")) && path.is_file(),
            "Asset is outside the installed files: {id}"
        );
        Ok(path)
    }

    pub fn verify(&self, cancel: &AtomicBool, progress: &mut impl FnMut(Progress)) -> Result<()> {
        let total = self.files().len();
        for (index, entry) in self.files().iter().enumerate() {
            check_cancel(cancel)?;
            let mut file = File::open(self.resolve(&entry.path)?)?;
            let digest = hash_stream(&mut file, cancel, |_| {})?;
            ensure!(
                digest == entry.sha256,
                "Installed asset is damaged: {}",
                entry.path
            );
            progress(Progress::CheckingInstalled {
                completed: index + 1,
                total,
            });
        }
        Ok(())
    }
}

fn validate_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty() && !path.contains(['\\', ':', '\0']),
        "Unsafe asset path"
    );
    ensure!(
        path.split('/')
            .all(|name| !name.is_empty() && name != "." && name != ".."),
        "Unsafe asset path"
    );
    ensure!(
        Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "Unsafe asset path"
    );
    Ok(())
}

fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Relaxed), "Import cancelled");
    Ok(())
}

fn entry_exists(path: &Path) -> Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn word(bytes: &[u8], offset: usize) -> Result<u32> {
    let value = bytes
        .get(offset..offset + 4)
        .context("Truncated disc filesystem")?;
    Ok(u32::from_be_bytes(value.try_into()?))
}

/// Parse and check the entire filesystem before creating any output files.
fn read_entries(stream: &mut (impl Read + Seek)) -> Result<Vec<AssetFile>> {
    let size = stream.seek(SeekFrom::End(0))?;
    stream.rewind()?;
    let mut header = [0; 0x440];
    stream
        .read_exact(&mut header)
        .context("Not a GameCube ISO")?;
    ensure!(
        &header[..6] == b"GALE01" && header[7] == 2 && word(&header, 0x1c)? == 0xc2339f3d,
        "Choose an unmodified Melee USA 1.02 ISO"
    );
    let offset = u64::from(word(&header, 0x424)?);
    let length = u64::from(word(&header, 0x428)?);
    ensure!(
        offset >= 0x440 && (12..=32 * 1024 * 1024).contains(&length) && offset + length <= size,
        "Invalid disc filesystem range"
    );
    stream.seek(SeekFrom::Start(offset))?;
    let mut fst = vec![0; length as usize];
    stream.read_exact(&mut fst)?;
    let count = word(&fst, 8)? as usize;
    ensure!(
        word(&fst, 0)? == 0x01000000
            && word(&fst, 4)? == 0
            && count > 0
            && count <= fst.len() / 12
            && count <= 100_000,
        "Invalid disc filesystem root"
    );
    let mut stack = vec![(0, count, String::new())];
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for index in 1..count {
        while index >= stack.last().context("Invalid directory end")?.1 {
            stack.pop();
        }
        let (parent, parent_end, folder) = stack.last().context("Missing parent directory")?;
        let flags = word(&fst, index * 12)?;
        let start = word(&fst, index * 12 + 4)? as usize;
        let end = word(&fst, index * 12 + 8)? as usize;
        let name_offset = count * 12 + (flags & 0xffffff) as usize;
        let name_bytes = fst.get(name_offset..).context("Invalid filename offset")?;
        let terminator = name_bytes
            .iter()
            .position(|b| *b == 0)
            .context("Unterminated filename")?;
        let name = std::str::from_utf8(&name_bytes[..terminator]).context("Invalid filename")?;
        ensure!(
            name.is_ascii() && !name.contains('/'),
            "Invalid disc filename"
        );
        validate_path(name)?;
        let path = if folder.is_empty() {
            name.to_owned()
        } else {
            format!("{folder}/{name}")
        };
        ensure!(
            path.len() <= 4096 && stack.len() <= 64,
            "Disc filesystem nesting is too deep"
        );
        ensure!(
            seen.insert(path.to_ascii_lowercase()),
            "Duplicate disc path"
        );
        match flags >> 24 {
            1 => {
                ensure!(
                    start == *parent && end > index && end <= *parent_end,
                    "Invalid directory bounds"
                );
                stack.push((index, end, path));
            }
            0 => {
                ensure!(
                    start as u64 >= offset + length && start as u64 + end as u64 <= size,
                    "Disc file outside image"
                );
                entries.push(AssetFile {
                    path,
                    offset: start as u64,
                    size: end as u64,
                    sha256: String::new(),
                });
            }
            _ => bail!("Invalid disc entry type"),
        }
    }
    for required in ["PlFx.dat", "PlFxNr.dat", "GrNBa.dat"] {
        ensure!(
            entries.iter().any(|entry| entry.path == required),
            "Missing Melee file: {required}"
        );
    }
    Ok(entries)
}

fn hash_stream(
    stream: &mut impl Read,
    cancel: &AtomicBool,
    mut read: impl FnMut(u64),
) -> Result<String> {
    let mut digest = Sha256::new();
    let mut bytes = vec![0; CHUNK];
    let mut total = 0;
    loop {
        check_cancel(cancel)?;
        let length = stream.read(&mut bytes)?;
        if length == 0 {
            break;
        }
        digest.update(&bytes[..length]);
        total += length as u64;
        read(total);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn validate_iso(
    path: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<(File, Vec<AssetFile>)> {
    let mut stream = File::open(path).context("Opening ISO")?;
    let entries = read_entries(&mut stream)?;
    let size = stream.metadata()?.len();
    stream.rewind()?;
    let mut previous = None;
    let digest = hash_stream(&mut stream, cancel, |bytes| {
        let percent = (bytes.saturating_mul(100) / size.max(1)).min(100) as u8;
        if previous != Some(percent) {
            progress(Progress::Validating { percent });
            previous = Some(percent);
        }
    })?;
    ensure!(
        digest == ISO_SHA256,
        "ISO checksum does not match the unmodified Melee USA 1.02 image"
    );
    Ok((stream, entries))
}

fn find_iso(
    source: Source,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<(File, Vec<AssetFile>)> {
    if let Source::File(path) = source {
        return validate_iso(&path, cancel, progress);
    }
    let Source::Search(roots) = source else {
        unreachable!()
    };
    progress(Progress::Searching);
    let mut pending: Vec<_> = roots.into_iter().rev().collect();
    let mut seen = HashSet::new();
    let mut rejected = 0;
    while let Some(path) = pending.pop() {
        check_cancel(cancel)?;
        let Ok(meta) = path.symlink_metadata() else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        let Ok(canonical) = path.canonicalize() else {
            continue;
        };
        if !seen.insert(canonical) {
            continue;
        }
        if meta.is_dir() {
            if let Ok(dir) = fs::read_dir(path) {
                let mut children: Vec<_> = dir.filter_map(|e| e.ok().map(|e| e.path())).collect();
                children.sort();
                pending.extend(children.into_iter().rev());
            }
        } else if meta.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("iso"))
        {
            match validate_iso(&path, cancel, progress) {
                Ok(result) => return Ok(result),
                Err(_) => {
                    rejected += 1;
                    progress(Progress::Searching);
                }
            }
        }
    }
    check_cancel(cancel)?;
    bail!(
        "No valid Melee USA 1.02 ISO found ({rejected} rejected). Use Choose ISO file to select one."
    )
}

/// Import on a worker thread. A private staging folder and an exclusive install
/// lock prevent partial bundles and concurrent overwrite. Existing bundles are
/// verified and reused; repairing them requires choosing a fresh destination.
pub fn import(
    source: Source,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(Progress),
) -> Result<AssetBundle> {
    if entry_exists(destination)? {
        let bundle = AssetBundle::open(destination)?;
        bundle.verify(cancel, &mut progress)?;
        return Ok(bundle);
    }
    let (mut stream, entries) = find_iso(source, cancel, &mut progress)?;
    install(&mut stream, entries, destination, cancel, &mut progress)
}

fn install(
    stream: &mut (impl Read + Seek),
    mut entries: Vec<AssetFile>,
    destination: &Path,
    cancel: &AtomicBool,
    progress: &mut impl FnMut(Progress),
) -> Result<AssetBundle> {
    let parent = destination
        .parent()
        .context("Asset folder needs a parent")?;
    fs::create_dir_all(parent).context("Creating asset storage folder")?;
    let name = destination
        .file_name()
        .context("Asset folder needs a name")?
        .to_string_lossy();
    let lock_path = parent.join(format!(".{name}.import-lock"));
    let lock_file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .context("Opening asset installation lock")?;
    // OS-managed locks release on process exit, including a crash. Keep the
    // small lock file in place so another process cannot lock a different inode.
    lock_file
        .try_lock()
        .context("Another game window is importing assets. Wait for it to finish and retry")?;
    ensure!(
        !entry_exists(destination)?,
        "Assets were installed by another import; retry to verify them"
    );
    let staging = tempfile::Builder::new()
        .prefix(".skirmish-import-")
        .tempdir_in(parent)?;
    let total = entries.len();
    progress(Progress::Extracting {
        completed: 0,
        total,
    });
    let mut buffer = vec![0; CHUNK];
    for (index, entry) in entries.iter_mut().enumerate() {
        check_cancel(cancel)?;
        let target = staging.path().join("disc/files").join(&entry.path);
        fs::create_dir_all(target.parent().context("Missing file parent")?)?;
        let mut output =
            File::create_new(&target).context("Writing extracted asset (check free disk space)")?;
        stream.seek(SeekFrom::Start(entry.offset))?;
        let mut remaining = entry.size;
        let mut digest = Sha256::new();
        while remaining != 0 {
            check_cancel(cancel)?;
            let length = remaining.min(CHUNK as u64) as usize;
            stream
                .read_exact(&mut buffer[..length])
                .context("ISO changed or was truncated during extraction")?;
            output
                .write_all(&buffer[..length])
                .context("Writing assets failed; check free disk space")?;
            digest.update(&buffer[..length]);
            remaining -= length as u64;
        }
        output.sync_all()?;
        entry.sha256 = format!("{:x}", digest.finalize());
        progress(Progress::Extracting {
            completed: index + 1,
            total,
        });
    }
    check_cancel(cancel)?;
    let manifest = Manifest {
        schema: SCHEMA.into(), iso_sha256: ISO_SHA256.into(), files: entries,
        limitations: vec!["Original game files only; visual conversion, animation and native gameplay resource conversion are not implemented by this importer.".into()],
    };
    let mut file = File::create_new(staging.path().join("manifest.json"))?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    // Check the full resolver before publishing. The ISO has already been hashed;
    // each output file's digest was computed while copying its exact byte range.
    AssetBundle::open(staging.path())?;
    check_cancel(cancel)?;
    ensure!(
        !entry_exists(destination)?,
        "Destination appeared during import; existing assets were preserved"
    );
    fs::rename(staging.path(), destination).context("Publishing imported assets")?;
    AssetBundle::open(destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn fixture(names: &[&str]) -> Vec<u8> {
        let mut raw = vec![0; 0x900];
        raw[..6].copy_from_slice(b"GALE01");
        raw[7] = 2;
        raw[0x1c..0x20].copy_from_slice(&0xc2339f3du32.to_be_bytes());
        let mut fst = Vec::new();
        for n in [0x01000000, 0, names.len() as u32 + 1] {
            fst.extend(n.to_be_bytes());
        }
        let mut strings = Vec::new();
        for (i, name) in names.iter().enumerate() {
            for n in [strings.len() as u32, 0x800 + i as u32 * 4, 4] {
                fst.extend(n.to_be_bytes());
            }
            strings.extend(name.as_bytes());
            strings.push(0);
        }
        fst.extend(strings);
        raw[0x424..0x428].copy_from_slice(&0x440u32.to_be_bytes());
        raw[0x428..0x42c].copy_from_slice(&(fst.len() as u32).to_be_bytes());
        raw[0x440..0x440 + fst.len()].copy_from_slice(&fst);
        raw[0x800..0x80c].copy_from_slice(b"abcdefghijkl");
        raw
    }

    fn disc() -> Cursor<Vec<u8>> {
        Cursor::new(fixture(&["PlFx.dat", "PlFxNr.dat", "GrNBa.dat"]))
    }

    #[test]
    fn synthetic_disc_installs_and_resolves_without_disc_at_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("bundle");
        let mut source = disc();
        let entries = read_entries(&mut source).unwrap();
        let bundle = install(
            &mut source,
            entries,
            &dest,
            &AtomicBool::new(false),
            &mut |_| {},
        )
        .unwrap();
        drop(source);
        assert_eq!(
            fs::read(bundle.resolve("PlFxNr.dat").unwrap()).unwrap(),
            b"efgh"
        );
        assert!(bundle.resolve("../manifest.json").is_err());
        bundle.verify(&AtomicBool::new(false), &mut |_| {}).unwrap();
        fs::write(bundle.resolve("PlFx.dat").unwrap(), b"oops").unwrap();
        assert!(bundle.verify(&AtomicBool::new(false), &mut |_| {}).is_err());
    }

    #[test]
    fn malformed_tables_and_modified_images_are_rejected() {
        for names in [
            ["../bad", "PlFxNr.dat", "GrNBa.dat"],
            ["PlFx.dat", "PlFx.dat", "GrNBa.dat"],
        ] {
            assert!(read_entries(&mut Cursor::new(fixture(&names))).is_err());
        }
        let mut raw = disc().into_inner();
        raw[0x450..0x454].copy_from_slice(&0x900u32.to_be_bytes());
        assert!(read_entries(&mut Cursor::new(raw)).is_err());
        let temp = tempfile::tempdir().unwrap();
        let iso = temp.path().join("test.ISO");
        fs::write(&iso, disc().into_inner()).unwrap();
        assert!(validate_iso(&iso, &AtomicBool::new(false), &mut |_| {}).is_err());
        let dest = temp.path().join("bundle");
        assert!(
            import(
                Source::Search(vec![temp.path().into()]),
                &dest,
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert!(!dest.exists());
    }

    #[test]
    fn cancellation_and_truncation_clean_staging_and_preserve_existing_files() {
        for truncate in [false, true] {
            let temp = tempfile::tempdir().unwrap();
            let dest = temp.path().join("bundle");
            let mut source = disc();
            let entries = read_entries(&mut source).unwrap();
            if truncate {
                source.get_mut().truncate(0x801);
            }
            let cancel = AtomicBool::new(false);
            let result = install(&mut source, entries, &dest, &cancel, &mut |p| {
                if !truncate && matches!(p, Progress::Extracting { completed: 1, .. }) {
                    cancel.store(true, Ordering::Relaxed);
                }
            });
            assert!(result.is_err());
            assert!(!dest.exists());
            assert!(
                fs::read_dir(temp.path())
                    .unwrap()
                    .all(|entry| entry.unwrap().file_name() == ".bundle.import-lock")
            );
        }
        let temp = tempfile::tempdir().unwrap();
        let dest = temp.path().join("bundle");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"keep").unwrap();
        assert!(
            import(
                Source::File("missing.iso".into()),
                &dest,
                &AtomicBool::new(false),
                |_| {}
            )
            .is_err()
        );
        assert_eq!(fs::read(dest.join("keep")).unwrap(), b"keep");
    }
}
