#![cfg(feature = "experimental-continuations")]

use pon_ir::{InstKind, Terminator, lower_source};
use skirmish_pon_runtime::sequential::extract_segments;

#[test]
fn dumps_two_await_move_cfg_and_spills() {
    let source = r#"
class Move:
    async def run(self, action):
        counter = 1
        await action.first
        counter = counter + 1
        await action.second
        return counter + 1
"#;
    let module = lower_source(source).unwrap();
    let run = module
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    println!(
        "run function: arity={} locals={} blocks={}",
        run.arity,
        run.n_locals,
        run.blocks.len()
    );
    for block in &run.blocks {
        println!("block {}:", block.id.0);
        for inst in &block.insts {
            let detail = match &inst.kind {
                InstKind::Await { awaitable } => format!(" AWAIT awaitable={}", awaitable.0),
                InstKind::StoreLocal(slot, value) => {
                    format!(" STORE_LOCAL slot={} value={}", slot.0, value.0)
                }
                InstKind::LoadLocal(slot) => format!(" LOAD_LOCAL slot={}", slot.0),
                InstKind::GenDelegateStep { delegate } => {
                    format!(" DELEGATE delegate={}", delegate.0)
                }
                InstKind::GenResumePayload => " RESUME_PAYLOAD".into(),
                _ => String::new(),
            };
            println!("  v{} {:?}{}", inst.result.0, inst.kind, detail);
        }
        println!("  TERM {:?}", block.term);
    }
    let suspends = run
        .blocks
        .iter()
        .filter_map(|block| match block.term {
            Terminator::Suspend { state, resume, .. } => Some((state, block.id, resume)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        suspends.len(),
        2,
        "two awaits should lower to two suspend states"
    );
    assert_eq!(suspends[0].0, 1);
    assert_eq!(suspends[1].0, 2);

    let segments = extract_segments(run).unwrap();
    assert_eq!(segments.len(), 3);
    for segment in &segments {
        assert!(segment.function.blocks.iter().all(|block| {
            !matches!(
                block.term,
                Terminator::ForLoop { .. } | Terminator::Suspend { .. }
            ) && block.insts.iter().all(|inst| {
                !matches!(
                    inst.kind,
                    InstKind::Await { .. }
                        | InstKind::GenDelegateStep { .. }
                        | InstKind::GenResumePayload
                )
            })
        }));
    }
    let stores = segments
        .iter()
        .flat_map(|segment| segment.function.blocks.iter())
        .flat_map(|block| block.insts.iter())
        .filter(|inst| matches!(inst.kind, InstKind::StoreLocal(_, _)))
        .count();
    assert_eq!(
        stores, 4,
        "entry initialization, spill restoration, and between-await effect stores"
    );
    assert_eq!(segments[0].suspensions[0].output_locals, vec![2]);
    assert_eq!(segments[1].input_locals, vec![2]);
    assert_eq!(segments[1].suspensions[0].output_locals, vec![2]);
    assert_eq!(segments[2].input_locals, vec![2]);
}
