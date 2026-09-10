fn main() {
    #[cfg(feature = "c-oracle")]
    build_oracle();
    #[cfg(feature = "renderer")]
    build_renderer();
}

#[cfg(feature = "renderer")]
fn build_renderer() {
    let shaders = "src/renderer/shaders";
    println!("cargo:rerun-if-changed={shaders}");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for name in ["mesh", "ui"] {
        let shader = wesl::Wesl::new(shaders)
            .compile(
                &format!("package::{name}")
                    .parse()
                    .expect("valid shader module path"),
            )
            .unwrap_or_else(|error| panic!("WESL compilation failed: {error}"));
        std::fs::write(output.join(format!("{name}.wgsl")), shader.to_string())
            .expect("write compiled shader into Cargo build output");
    }
}

#[cfg(feature = "c-oracle")]
fn build_oracle() {
    use std::{collections::BTreeMap, env, fs, path::PathBuf};
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=tests/oracle");
    let originals: BTreeMap<String, PathBuf> = fs::read_dir("tests/oracle/original")
        .expect("C reference snapshots")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "c"))
        .map(|path| (path.file_stem().unwrap().to_str().unwrap().to_owned(), path))
        .collect();
    let aliases: BTreeMap<String, String> =
        serde_json::from_str(&fs::read_to_string("tests/oracle/adapters.json").unwrap()).unwrap();
    let mut adapters = originals.clone();
    for (alias, source) in aliases {
        assert!(
            alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
            "invalid adapter name"
        );
        let path = originals
            .get(&source)
            .expect("adapter must reference an original snapshot");
        assert!(
            adapters.insert(alias, path.clone()).is_none(),
            "duplicate adapter name"
        );
    }
    for (adapter, path) in adapters {
        let source = fs::read_to_string(&path).unwrap();
        let selection = if adapter == "ftcommon" {
            PathBuf::from("tests/oracle/physics.functions.json")
        } else {
            PathBuf::from("tests/oracle").join(format!("{adapter}.functions.json"))
        };
        let source = if selection.exists() {
            let names: Vec<String> =
                serde_json::from_str(&fs::read_to_string(selection).unwrap()).unwrap();
            names
                .iter()
                .map(|name| extract_function(&source, name))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            source
        };
        let mut adapted = source
            .lines()
            .filter(|line| !line.trim_start().starts_with("#include"))
            .collect::<Vec<_>>()
            .join("\n");
        // Replace the GameCube pointer-sized stack storage with explicit
        // 32-bit union members on the 64-bit host. Algorithms are unchanged.
        if path.file_stem().unwrap() == "bytecode" {
            adapted = adapted
                .replace("stack->data", "stack->data.u")
                .replace("list->data", "list->data.u")
                .replace("((ByteCodeVal*) &stack->data.u)->i", "stack->data.i")
                .replace("((ByteCodeVal*) &stack->data.u)->f", "stack->data.f")
                .replace(
                    "((ByteCodeVal*) &args[operand])->i",
                    "float_bits(args[operand])",
                )
                .replace("*(void**) &fv", "float_bits(fv)")
                .replace("*(void**) &f0", "float_bits(f0)")
                .replace("(void*)", "(uint32_t)");
        }
        if path.file_stem().unwrap() == "mplib" {
            // Original CollLine is 8 bytes; native pointers make the host
            // adapter's struct larger. Retain logical indices without truncating pointers.
            adapted = adapted.replace(
                "(s32) line_r26 - (s32) groundCollLine",
                "(ptrdiff_t) (line_r26 - groundCollLine) * 8",
            );
        }
        fs::write(out.join(format!("{adapter}_original.inc")), adapted).unwrap();
    }
    let mut build = cc::Build::new();
    build
        .std("gnu11")
        .include(out)
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-ffp-contract=off")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-fgnu89-inline")
        // Preserved upstream source has harmless warnings on the host ABI.
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter");
    for entry in fs::read_dir("tests/oracle").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "c") {
            build.file(path);
        }
    }
    build.compile("skirmish_oracle");
    println!("cargo:rustc-link-lib=m");
}

/// Select complete, verbatim definitions from a pinned, trusted snapshot. Only
/// column-zero function headers match; calls and declarations cannot match.
#[cfg(feature = "c-oracle")]
fn extract_function<'a>(source: &'a str, name: &str) -> &'a str {
    let needle = format!(" {name}(");
    let start = source
        .match_indices(&needle)
        .find_map(|(pos, _)| {
            let line = source[..pos].rfind('\n').map_or(0, |i| i + 1);
            let prefix = &source[line..pos];
            let body = source[pos..].find('{')? + pos;
            (!prefix.starts_with(char::is_whitespace)
                && !prefix.contains([';', '=', '('])
                && !source[pos..body].contains(';'))
            .then_some(line)
        })
        .unwrap_or_else(|| panic!("missing original function {name}"));
    let body = source[start..].find('{').unwrap() + start;
    assert!(
        !source[start..body].contains(';'),
        "declaration instead of definition: {name}"
    );
    let mut depth = 0;
    for (offset, byte) in source[body..].bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[start..=body + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated original function {name}");
}
