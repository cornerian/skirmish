//! Optional local-disc smoke check. The ordinary suite never needs a game ISO.
use skirmish::assets::{self, Source};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    sync::atomic::AtomicBool,
};

#[test]
#[ignore = "requires a local USA 1.02 ISO via SKIRMISH_TEST_ISO"]
fn native_import_matches_every_disc_file_byte_for_byte() {
    let iso = std::env::var_os("SKIRMISH_TEST_ISO").expect("set SKIRMISH_TEST_ISO");
    let mut original = File::open(&iso).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let destination = temp.path().join("bundle");
    let cancel = AtomicBool::new(false);
    let bundle = assets::import(Source::File(iso.into()), &destination, &cancel, |_| {}).unwrap();
    assert_eq!(bundle.files().len(), 1209);
    for entry in bundle.files() {
        original.seek(SeekFrom::Start(entry.offset)).unwrap();
        let mut source = vec![0; entry.size as usize];
        original.read_exact(&mut source).unwrap();
        assert_eq!(
            std::fs::read(bundle.resolve(&entry.path).unwrap()).unwrap(),
            source,
            "{}",
            entry.path
        );
    }
    let reused = assets::import(
        Source::File("no-disc-needed.iso".into()),
        &destination,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(reused.files().len(), 1209);
}
