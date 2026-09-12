fn main() {
    #[cfg(feature = "c-oracle")]
    build_oracle();
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
    // Adapters compiled with `-ffp-contract=fast -mfma` so their one, sole
    // `a * b + c` compiles to a real host `vfmadd`-family instruction
    // instead of the default separately-rounded multiply then add,
    // matching the real Gekko `fmadds` this batch confirmed with
    // `tools/ppc_fma_audit.py` against the retail DOL (`docs/math.md`):
    // `ftCommon_CalcHitlag` -> `fighter::combat::hitlag`, and
    // `ftCo_Damage_CalcAngle` -> `fighter::damage::launch_angle` (split into
    // its own translation unit, `damage_calc_angle.c`, since its five
    // siblings in `damage_core.c` include unaudited fused ops of their
    // own). Two more audited functions were *tried* here and reverted:
    // `ftColl_80079AB0` (`combat_knockback.c`) and `ftCo_800DA824`
    // (`escape_formula.c`) each have more than one candidate multiply
    // feeding an addition, and disassembling the resulting objects showed
    // GCC's contraction pass choosing a different pairing (or reaching
    // across statement boundaries to a pairing the real PowerPC compiler
    // never fused at all) than the retail binary -- see `docs/math.md` for
    // the concrete before/after evidence. Their Rust ports still use
    // `f32::mul_add` at the exact expressions `tools/ppc_fma_audit.py`
    // identified; they're checked against the oracle with a documented
    // few-ULP tolerance instead of bit-for-bit, and pinned exactly by their
    // own native unit tests (`fighter::combat`, `fighter::grab`).
    //
    // Every other adapter (including `lbVector_AngleXY`'s `up_special_angle`
    // extraction, which also has one `fmadds` -- see `docs/math.md` -- but
    // is compiled inside `fox_specialhi.c` alongside unaudited
    // `ftFox_SpecialHi_*` bodies) stays on the original, uncontracted build:
    // contracting that shared translation unit could change functions this
    // batch never checked against hardware fusing. `up.rs::angle_xy` is
    // instead pinned by a native unit test against a hand-verified fused
    // value (`docs/math.md`, `docs/validation.md`).
    const FMA_CONTRACT_FILES: &[&str] = &["combat_hitlag.c", "damage_calc_angle.c"];

    let mut build = cc::Build::new();
    build
        .std("gnu11")
        .include(&out)
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-ffp-contract=off")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-fgnu89-inline")
        // Preserved upstream source has harmless warnings on the host ABI.
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter");
    let mut fma_build = cc::Build::new();
    fma_build
        .std("gnu11")
        .include(&out)
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-ffp-contract=fast")
        .flag_if_supported("-mfma")
        .flag_if_supported("-fno-fast-math")
        .flag_if_supported("-fgnu89-inline")
        .flag_if_supported("-Wno-sign-compare")
        .flag_if_supported("-Wno-unused-parameter")
        // GCC only performs `-ffp-contract=fast` contraction as part of its
        // optimizer; at `-O0` (Cargo's dev/test profile, which `cc` mirrors
        // by default) the flag is accepted but silently does nothing, so a
        // debug test build would link an *uncontracted* oracle here and
        // every one of this batch's differential tests would fail. Force an
        // optimized build for just this one static library regardless of
        // the enclosing Cargo profile; verified against a runtime (not
        // compile-time-constant-folded) `a * b + c` by disassembling the
        // resulting object for a real `vfmadd`-family instruction.
        .opt_level(2);
    for entry in fs::read_dir("tests/oracle").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|ext| ext == "c") {
            if FMA_CONTRACT_FILES.contains(&path.file_name().unwrap().to_str().unwrap()) {
                fma_build.file(path);
            } else {
                build.file(path);
            }
        }
    }
    build.compile("skirmish_oracle");
    fma_build.compile("skirmish_oracle_fma");
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
