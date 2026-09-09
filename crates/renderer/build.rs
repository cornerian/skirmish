fn main() {
    println!("cargo:rerun-if-changed=shaders");
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    for name in ["mesh", "ui", "particles"] {
        let shader = wesl::Wesl::new("shaders")
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
