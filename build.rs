fn main() {
    #[cfg(feature = "c-oracle")]
    build_oracle();
}

#[cfg(feature = "c-oracle")]
fn build_oracle() {
    use std::{env, fs, path::PathBuf};
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=tests/oracle");
    for entry in fs::read_dir("tests/oracle/original").expect("C reference snapshots") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "c") {
            let source = fs::read_to_string(&path).unwrap();
            let source = if path.file_stem().unwrap() == "ftcommon" {
                let names: Vec<String> = serde_json::from_str(
                    &fs::read_to_string("tests/oracle/physics.functions.json").unwrap(),
                )
                .unwrap();
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
            fs::write(
                out.join(format!(
                    "{}_original.inc",
                    path.file_stem().unwrap().to_str().unwrap()
                )),
                adapted,
            )
            .unwrap();
        }
    }
    let mut build = cc::Build::new();
    build
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
            (!prefix.starts_with(char::is_whitespace) && !prefix.contains([';', '=', '(']))
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
