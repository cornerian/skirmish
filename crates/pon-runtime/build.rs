use sha2::{Digest, Sha256};
use std::{
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

fn collect(root: &Path, base: &Path, files: &mut Vec<(String, Vec<u8>)>) {
    let entries =
        fs::read_dir(root).unwrap_or_else(|error| panic!("read {}: {error}", root.display()));
    let mut entries = entries
        .map(|entry| entry.unwrap_or_else(|error| panic!("read {}: {error}", root.display())))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            collect(&path, base, files);
        } else {
            let relative = path.strip_prefix(base).expect("source under base");
            let relative = relative
                .to_string_lossy()
                .replace(std::path::MAIN_SEPARATOR, "/");
            files.push((relative, fs::read(&path).expect("read vendored source")));
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let vendor = manifest.join("../../vendor/pon");
    let mut files = Vec::new();
    for directory in ["pon-runtime", "pon-jit"] {
        let root = vendor.join(directory);
        println!("cargo:rerun-if-changed={}", root.display());
        if root.exists() {
            collect(&root, &vendor, &mut files);
        }
    }
    let manifest = "Cargo.toml";
    let path = vendor.join(manifest);
    println!("cargo:rerun-if-changed={}", path.display());
    files.push((
        manifest.to_owned(),
        fs::read(path).expect("read vendored manifest"),
    ));
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut serialized = b"skirmish-pon-vendored-sources-v1\0".to_vec();
    for (path, bytes) in files {
        serialized.extend_from_slice(&(path.len() as u64).to_le_bytes());
        serialized.extend_from_slice(path.as_bytes());
        serialized.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        serialized.extend_from_slice(&bytes);
    }
    let digest: [u8; 32] = Sha256::digest(&serialized).into();
    let destination = PathBuf::from(env::var_os("OUT_DIR").expect("out dir"));
    let mut file = fs::File::create(destination.join("compiler_sources.bin"))
        .expect("create fingerprint output");
    file.write_all(&digest).expect("write fingerprint output");
}
