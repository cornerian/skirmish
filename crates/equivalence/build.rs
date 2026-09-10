fn main() {
    #[cfg(feature = "c-oracle")]
    prepare_rng_snapshots();
}

#[cfg(feature = "c-oracle")]
fn prepare_rng_snapshots() {
    use std::{env, fs, path::PathBuf};

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let originals = root.join("tests/oracle/original");
    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!(
        "cargo:rerun-if-changed={}",
        originals.join("random_hsd.c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        originals.join("random_msl.c").display()
    );
    for name in ["random_hsd", "random_msl"] {
        let source = fs::read_to_string(originals.join(format!("{name}.c"))).unwrap();
        let adapted = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("#include"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(output.join(format!("{name}_original.inc")), adapted).unwrap();
    }
}
