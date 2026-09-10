fn main() {
    println!("cargo:rustc-check-cfg=cfg(skirmish_melee_source)");
    #[cfg(feature = "source")]
    build_source();
}

#[cfg(feature = "source")]
fn build_source() {
    use std::{env, fs, path::PathBuf, process::Command};

    const REVISION: &str = "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9";

    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"));
    let workspace = manifest
        .parent()
        .and_then(|path| path.parent())
        .expect("melee-ui-sys must be in the workspace crates directory");
    let upstream = env::var_os("SKIRMISH_MELEE_SOURCE")
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace.join("../../External/melee"));
    let source = upstream.join("src/melee/mn/mnmain.c");
    println!("cargo:rerun-if-env-changed=SKIRMISH_MELEE_SOURCE");
    println!("cargo:rerun-if-changed={}", source.display());
    if !source.is_file() {
        println!(
            "cargo:warning=pinned Melee source not found at {}; direct UI source is unavailable",
            upstream.display()
        );
        return;
    }
    println!("cargo:rerun-if-changed={}", upstream.join("src").display());
    println!(
        "cargo:rerun-if-changed={}",
        upstream.join("extern/dolphin/include").display()
    );
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .arg("-C")
            .arg(&upstream)
            .args(args)
            .output()
            .unwrap_or_else(|error| panic!("run git in {}: {error}", upstream.display()));
        assert!(
            output.status.success(),
            "git {} failed in {}: {}",
            args.join(" "),
            upstream.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("Git output must be UTF-8")
    };
    let head = git(&["rev-parse", "--verify", "HEAD"]);
    assert_eq!(
        head.trim(),
        REVISION,
        "Melee checkout must be at the pinned revision"
    );
    let status = git(&["status", "--porcelain=v1", "--untracked-files=all"]);
    assert!(
        status.is_empty(),
        "Melee checkout must be clean before native compilation:\n{status}"
    );

    let snapshot = workspace.join("tests/oracle/original/mnmain.c");
    println!("cargo:rerun-if-changed={}", snapshot.display());
    let source_bytes = fs::read(&source).expect("read upstream mnmain.c");
    let snapshot_bytes = fs::read(&snapshot).expect("read pinned mnmain.c snapshot");
    assert_eq!(
        source_bytes, snapshot_bytes,
        "upstream mnmain.c does not match the pinned {REVISION} source"
    );

    let host = manifest.join("c/host_platform.h");
    let bridge = manifest.join("c/bridge.c");
    cc::Build::new()
        .std("gnu11")
        .include(upstream.join("src"))
        .include(upstream.join("extern/dolphin/include"))
        .flag("-include")
        .flag(host.to_str().expect("UTF-8 host header path"))
        .flag_if_supported("-ffunction-sections")
        .flag_if_supported("-fdata-sections")
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter")
        .warnings(false)
        .file(source)
        .file(bridge)
        .compile("skirmish_melee_ui_source");
    println!("cargo:rustc-cfg=skirmish_melee_source");
}
