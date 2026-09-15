//! Stable identity bytes for script resources.

use super::{bundled_source, definition::AssetStore};
use crate::game::data::FighterData;
use std::collections::{BTreeMap, BTreeSet};

/// Stable identity marker for the Pon scripting ABI. Keep this marker
/// separate from the source hash: changing the frontend, engine bindings, or
/// compiler behavior must invalidate replay/checkpoint identities even when a
/// character's source bytes are unchanged.
const ABI_MARKER: &[u8] = b"\0skirmish-pon-fighter-v2\0";

/// Appends the selected fighter programs and their registered dependencies to
/// a match resource stream.  The source selection remains owned by the game
/// resource, while this encoding keeps names and bytes unambiguous and stable.
pub(crate) fn append(resource_bytes: &mut Vec<u8>, fighters: &[FighterData]) {
    append_with_assets(resource_bytes, fighters, &AssetStore::builtins());
}

fn append_with_assets(resource_bytes: &mut Vec<u8>, fighters: &[FighterData], assets: &AssetStore) {
    let mut selected = BTreeMap::<String, String>::new();
    let mut program_records = BTreeSet::<Vec<u8>>::new();
    let mut has_script = false;
    for fighter in fighters {
        let source = fighter
            .script
            .as_ref()
            .map(|program| {
                program_records.insert(encode_program_record(
                    program.source(),
                    &program.dependency_sources,
                ));
                program.source()
            })
            .or_else(|| bundled_source(fighter.specials.as_ref()));
        if let Some(source) = source {
            has_script = true;
            if fighter.script.is_none() {
                selected.insert(source.to_owned(), source.to_owned());
            }
        }
    }
    if !has_script {
        return;
    }

    append_sources_with_dependencies(resource_bytes, selected.values(), assets, &program_records);
}

#[cfg(test)]
fn append_sources<'a, I>(resource_bytes: &mut Vec<u8>, selected: I, assets: &AssetStore)
where
    I: IntoIterator<Item = &'a String>,
{
    append_sources_with_dependencies(resource_bytes, selected, assets, &BTreeSet::new());
}

fn append_sources_with_dependencies<'a, I>(
    resource_bytes: &mut Vec<u8>,
    selected: I,
    assets: &AssetStore,
    program_records: &BTreeSet<Vec<u8>>,
) where
    I: IntoIterator<Item = &'a String>,
{
    append_identity_prefix(
        resource_bytes,
        skirmish_script_runtime::compiler_identity::compiler_identity(),
    );
    if let Some(identity) = skirmish_script_runtime::configured_standard_library_identity() {
        resource_bytes.extend_from_slice(b"stdlib:");
        resource_bytes.extend_from_slice(&identity);
        resource_bytes.push(0);
    }
    resource_bytes.extend_from_slice(b"sdk:");
    resource_bytes.extend_from_slice(
        &crate::game::script::starlark::embedded_sdk_identity().expect("embedded Pon SDK identity"),
    );
    resource_bytes.push(0);
    for source in selected {
        append_field(resource_bytes, b"selected", source.as_bytes());
    }
    let dependencies = assets.dependencies("").expect("built-in assets are valid");
    for (name, source) in dependencies {
        append_field(resource_bytes, name.as_bytes(), source.as_bytes());
    }
    for record in program_records {
        append_field(resource_bytes, b"program", record);
    }
}

fn append_identity_prefix(resource_bytes: &mut Vec<u8>, compiler_identity: [u8; 32]) {
    resource_bytes.extend_from_slice(ABI_MARKER);
    resource_bytes.extend_from_slice(b"compiler:");
    resource_bytes.extend_from_slice(&compiler_identity);
    resource_bytes.push(0);
}

fn encode_program_record(source: &str, dependencies: &BTreeMap<String, String>) -> Vec<u8> {
    let mut record = Vec::new();
    append_field(&mut record, b"source", source.as_bytes());
    for (name, dependency) in dependencies {
        append_field(&mut record, name.as_bytes(), dependency.as_bytes());
    }
    record
}

fn append_field(stream: &mut Vec<u8>, name: &[u8], value: &[u8]) {
    stream.extend_from_slice(&(name.len() as u64).to_le_bytes());
    stream.extend_from_slice(name);
    stream.extend_from_slice(&(value.len() as u64).to_le_bytes());
    stream.extend_from_slice(value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn digest(dependency: &str) -> [u8; 32] {
        let mut assets = AssetStore::default();
        assets.register("helper.py", dependency);
        let source = r#"
from helper import BONUS
fighter = character(
    name = "identity_test", external_ids = [], state = State(),
    action_state = State(), parameters = struct(), behaviors = [],
)
"#;
        let mut bytes = Vec::new();
        let selected = [source.to_owned()];
        append_sources(&mut bytes, selected.iter(), &assets);
        Sha256::digest(bytes).into()
    }

    #[test]
    fn imported_dependency_changes_script_identity() {
        assert_ne!(digest("one"), digest("two"));
    }

    #[test]
    fn dependency_order_is_deterministic() {
        let mut first = AssetStore::default();
        first.register("z.py", "z");
        first.register("a.py", "a");
        let mut second = AssetStore::default();
        second.register("a.py", "a");
        second.register("z.py", "z");
        let source = ["source".to_owned()];
        let mut first_bytes = Vec::new();
        append_sources(&mut first_bytes, source.iter(), &first);
        let mut second_bytes = Vec::new();
        append_sources(&mut second_bytes, source.iter(), &second);
        assert_eq!(first_bytes, second_bytes);
    }

    #[test]
    fn dependency_association_changes_identity() {
        let source_a = "program-a";
        let source_b = "program-b";
        let mut a_then_b = BTreeSet::new();
        a_then_b.insert(encode_program_record(
            source_a,
            &[("helper.py".into(), "one".into())].into_iter().collect(),
        ));
        a_then_b.insert(encode_program_record(
            source_b,
            &[("helper.py".into(), "two".into())].into_iter().collect(),
        ));
        let mut b_then_a = BTreeSet::new();
        b_then_a.insert(encode_program_record(
            source_a,
            &[("helper.py".into(), "two".into())].into_iter().collect(),
        ));
        b_then_a.insert(encode_program_record(
            source_b,
            &[("helper.py".into(), "one".into())].into_iter().collect(),
        ));
        let assets = AssetStore::default();
        let selected = std::iter::empty();
        let mut first = Vec::new();
        append_sources_with_dependencies(&mut first, selected, &assets, &a_then_b);
        let mut second = Vec::new();
        append_sources_with_dependencies(&mut second, std::iter::empty(), &assets, &b_then_a);
        assert_ne!(Sha256::digest(first), Sha256::digest(second));
    }

    #[test]
    fn resource_identity_contains_compiler_fingerprint() {
        let assets = AssetStore::default();
        let source = ["source".to_owned()];
        let mut bytes = Vec::new();
        append_sources(&mut bytes, source.iter(), &assets);
        let marker = skirmish_script_runtime::compiler_identity::compiler_identity();
        assert!(bytes.windows(marker.len()).any(|window| window == marker));
    }

    #[test]
    fn compiler_only_identity_change_rejects_resource_checkpoint_identity() {
        let assets = AssetStore::default();
        let source = ["source".to_owned()];
        let mut current = Vec::new();
        append_sources(&mut current, source.iter(), &assets);
        let mut incompatible = Vec::new();
        append_identity_prefix(&mut incompatible, [0xA5; 32]);
        incompatible.extend_from_slice(&current[ABI_MARKER.len() + 9 + 32 + 1..]);
        assert_ne!(Sha256::digest(current), Sha256::digest(incompatible));
    }
}
