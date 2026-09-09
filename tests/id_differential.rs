#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]

use proptest::prelude::*;
use skirmish::id::{IdRegistry, IdTable};
use std::sync::Mutex;

static ORACLE: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn oracle_id_trace(ops: *const u32, count: u32, output: *mut u32);
}

fn compare(ops: &[[u32; 4]]) {
    let _guard = ORACLE.lock().unwrap();
    let mut expected = vec![[0u32; 2]; ops.len()];
    // SAFETY: arrays are contiguous u32s with exactly the wrapper's input/output
    // widths; all operation kinds, table selectors and trace counts are bounded.
    unsafe {
        oracle_id_trace(
            ops.as_ptr().cast(),
            ops.len() as u32,
            expected.as_mut_ptr().cast(),
        );
    }
    let mut registry = IdRegistry::default();
    let mut explicit = IdTable::new();
    for (index, [kind, table, id, data]) in ops.iter().copied().enumerate() {
        match kind {
            0 => {
                registry.insert((table != 0).then_some(&mut explicit), id, data);
            }
            1 => {
                registry.remove((table != 0).then_some(&mut explicit), id);
            }
            2 => {}
            3 => registry.setup(),
            4 => registry.forget_memory(id as usize, data as usize),
            _ => unreachable!(),
        }
        let found = registry.get((table != 0).then_some(&explicit), id);
        assert_eq!(
            [found.copied().unwrap_or(0), u32::from(found.is_some())],
            expected[index],
            "operation {index}: {:?}",
            ops[index],
        );
    }
}

#[test]
fn collisions_replacement_null_and_removal_match_upstream() {
    let mut ops = Vec::new();
    for table in 0..2 {
        for id in 0..30 {
            ops.push([0, table, id * 101, id]);
        }
        for id in [0, 29, 15, 14, 28, 1] {
            ops.push([1, table, id * 101, 0]);
            ops.push([1, table, id * 101, 0]);
        }
        for id in 0..30 {
            ops.push([2, table, id * 101, 0]);
            ops.push([0, table, id * 101, 0]);
            ops.push([2, table, id * 101, 0]);
        }
    }
    ops.extend([[4, 0, 5, 0], [2, 0, 202, 0], [2, 1, 202, 0]]);
    compare(&ops);
}

proptest! {
    #[test]
    fn operation_sequences_match_original_c(
        ops in prop::collection::vec(
            (0u32..5, 0u32..2, prop_oneof![0u32..64, any::<u32>()], any::<u32>())
                .prop_map(|(kind, table, id, data)| [kind, table, id, data]),
            0..512,
        ),
    ) {
        compare(&ops);
    }
}
