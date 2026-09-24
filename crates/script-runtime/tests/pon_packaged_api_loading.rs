//! Regression for the embedded authoring package manifest.
//!
//! Captain-family fighters import the shared `fighter.captain_family` API
//! module. The test goes through `CompiledProgram`, whose preparation path
//! merges the generated embedded SDK bundle. It is ignored by default because
//! the class-based authoring API requires the verified Pon standard-library
//! archive.

use skirmish_script_runtime::{CompiledProgram, NativeValue};

#[test]
#[ignore = "requires the verified Pon stdlib artifact"]
fn packaged_sdk_loads_shared_captain_family_module() {
    // Keep the fixture outside a canonical roster filename: this makes the
    // assertion isolate package importing rather than relying on roster
    // identity inference for a root module.
    let source = r#"
from skirmish import Fighter
from fighter.captain_family import CaptainNeutralSpecial

class PackageProbe(Fighter):
    name = "package-probe"
    external_ids = (99,)
"#;
    let program = CompiledProgram::new(source, "package_probe.py", [])
        .expect("construct package probe program");
    program
        .prepare_for_current_thread()
        .expect("embedded authoring package must include fighter.captain_family");

    let NativeValue::Dict(metadata) = program.export_metadata().expect("export package probe")
    else {
        panic!("package probe export must be a definition dictionary");
    };
    assert_eq!(
        metadata["name"],
        NativeValue::String("package-probe".into())
    );
}
