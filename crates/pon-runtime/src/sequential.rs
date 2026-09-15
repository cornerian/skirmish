//! Extraction of bounded CFG regions for sequential awaits.

use crate::{Error, continuation::NativeEntry};
use pon_ir::{Block, BlockId, Function, Inst, InstKind, PyConst, Terminator, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentError {
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suspension {
    pub tag: u32,
    pub output_locals: Vec<u32>,
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub function: Function,
    pub input_locals: Vec<u32>,
    pub suspensions: Vec<Suspension>,
    /// Resumed entries receive one explicit completion value after spills.
    pub resume_value: bool,
}

pub struct SequentialNativeImage {
    pub segments: Vec<Segment>,
    pub entries: Vec<NativeEntry>,
}

pub fn compile_sequential(
    module: &pon_ir::Module,
    function: pon_ir::FunctionId,
) -> Result<SequentialNativeImage, Error> {
    let original = module
        .functions
        .get(function.0 as usize)
        .ok_or_else(|| Error::Compile("sequential function disappeared".into()))?;
    let segments = extract_segments(original).map_err(|e| Error::Compile(e.to_string()))?;
    let mut entries = Vec::with_capacity(segments.len());
    for segment in &segments {
        let mut segment_module = module.clone();
        segment_module.functions[function.0 as usize] = segment.function.clone();
        segment_module.main = function;
        entries.push(NativeEntry::compile_module_entry(segment_module, function)?);
    }
    Ok(SequentialNativeImage { segments, entries })
}

impl fmt::Display for SegmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self::Unsupported(message) = self;
        write!(f, "unsupported sequential continuation: {message}")
    }
}
impl std::error::Error for SegmentError {}

#[derive(Clone, Copy)]
struct AwaitSite {
    block: BlockId,
    tag: u32,
    target: BlockId,
}

/// Split at every reachable await. Each region retains its CFG until any
/// path reaches an await or return, so branch and join semantics survive.
pub fn extract_segments(function: &Function) -> Result<Vec<Segment>, SegmentError> {
    if function.blocks.is_empty() {
        return Err(unsupported("function has no blocks"));
    }
    let mut sites = Vec::new();
    for block in &function.blocks {
        for inst in &block.insts {
            if matches!(inst.kind, InstKind::Await { .. }) {
                sites.push(AwaitSite {
                    block: block.id,
                    tag: sites.len() as u32 + 1,
                    target: completion_block(function, block.id)?,
                });
            }
        }
    }
    let mut starts = vec![function.blocks[0].id];
    // Keep one stable entry per source await, even when completion blocks
    // happen to join. The tag is therefore always the direct entry index.
    for site in &sites {
        starts.push(site.target);
    }
    let regions = starts
        .iter()
        .map(|id| walk_region(function, *id, &sites))
        .collect::<Result<Vec<_>, _>>()?;
    // A downstream await is normally outside region zero. Walk the region
    // transition graph to distinguish that case from a truly unreachable site.
    let mut reachable = vec![false; starts.len()];
    reachable[0] = true;
    for index in 0..starts.len() {
        if reachable[index] {
            for site in sites
                .iter()
                .filter(|site| regions[index].contains(&site.block))
            {
                reachable[site.tag as usize] = true;
            }
        }
    }
    for site in &sites {
        if !reachable[site.tag as usize] {
            return Err(unsupported(format!(
                "await block {} is unreachable",
                site.block.0
            )));
        }
    }
    // Compute live-in locals across both ordinary CFG edges and suspension
    // edges. A resumed region may define a local before consuming it, so its
    // downstream live-in set must flow through that region's definitions
    // before it reaches the preceding suspension. Repeatedly solving the
    // monotone equations also handles branches whose await tags are not a
    // simple linear chain.
    let mut schemas = vec![Vec::new(); regions.len()];
    loop {
        let mut changed = false;
        for (index, region) in regions.iter().enumerate() {
            let candidate = live_slots(function, region, function.arity as u32, &sites, &schemas);
            if candidate != schemas[index] {
                schemas[index] = candidate;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // The generator transform materializes an await resume payload and often
    // stores it into a temporary even when the source expression discards the
    // value. Those bookkeeping stores are safe to remove; an authored use is
    // rejected explicitly below.
    for site in &sites {
        let result_slots = post_await_result_slots(function, *site)?;
        let target_reads = &schemas[site.tag as usize];
        if result_slots.iter().any(|slot| target_reads.contains(slot)) {
            return Err(unsupported("await result use is unsupported"));
        }
    }

    let mut result = Vec::with_capacity(regions.len());
    let mut synthetic = max_value(function).saturating_add(1);
    for (index, region) in regions.iter().enumerate() {
        let input = if index == 0 {
            Vec::new()
        } else {
            schemas[index].clone()
        };
        let resume_value = index > 0;
        let shift = input.len() as u32 + u32::from(resume_value);
        let mut blocks = region
            .iter()
            .map(|id| block(function, *id).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        for current in &mut blocks {
            for inst in &mut current.insts {
                shift_compiler_local(&mut inst.kind, function.arity as u32, shift);
            }
            if resume_value {
                replace_stop_values(current, function.arity as u32 + input.len() as u32);
            }
            current
                .insts
                .retain(|inst| !is_generator_payload(&inst.kind));
            if let Some(site) = sites.iter().find(|site| site.block == current.id) {
                let position = current
                    .insts
                    .iter()
                    .position(|inst| matches!(inst.kind, InstKind::Await { .. }))
                    .ok_or_else(|| unsupported("await position disappeared during lowering"))?;
                let (_, await_result, resume_values) = await_resume_values(function, site.block)?;
                let mut generated_values = resume_values;
                generated_values.insert(await_result);
                if current.insts[position + 1..].iter().any(|inst| {
                    !is_generator_payload(&inst.kind)
                        && !matches!(
                            inst.kind,
                            InstKind::GenResumePayload | InstKind::GenLastStopValue
                        )
                        && !matches!(inst.kind, InstKind::StoreLocal(_, value) if generated_values.contains(&value))
                }) { return Err(unsupported("await result use is unsupported")); }
                let awaitable = match current.insts[position].kind {
                    InstKind::Await { awaitable } => awaitable,
                    _ => unreachable!(),
                };
                current.insts.truncate(position);
                let one = Value(synthetic);
                let tag = Value(synthetic + 1);
                synthetic += 2;
                current
                    .insts
                    .push(Inst::new(one, InstKind::Const(PyConst::Int(1))));
                current.insts.push(Inst::new(
                    tag,
                    InstKind::Const(PyConst::Int(site.tag as i64)),
                ));
                let target = site.tag as usize;
                let mut tuple = vec![one, tag, awaitable];
                for slot in &schemas[target] {
                    let value = Value(synthetic);
                    synthetic += 1;
                    current.insts.push(Inst::new(
                        value,
                        InstKind::LoadLocal(pon_ir::LocalId(*slot + shift)),
                    ));
                    tuple.push(value);
                }
                let output = Value(synthetic);
                synthetic += 1;
                current
                    .insts
                    .push(Inst::new(output, InstKind::BuildList { elts: tuple }));
                current.term = Terminator::Return(output);
            } else if let Terminator::Return(value) = current.term {
                let marker = Value(synthetic);
                let output = Value(synthetic + 1);
                synthetic += 2;
                current
                    .insts
                    .push(Inst::new(marker, InstKind::Const(PyConst::Int(0))));
                current.insts.push(Inst::new(
                    output,
                    InstKind::BuildList {
                        elts: vec![marker, value],
                    },
                ));
                current.term = Terminator::Return(output);
            }
        }
        if index > 0 {
            let entry = starts[index];
            let current = blocks
                .iter_mut()
                .find(|block| block.id == entry)
                .ok_or_else(|| missing_block(entry))?;
            let mut prologue = Vec::with_capacity(input.len() * 2 + 1);
            let (_, await_result, _) = await_resume_values(function, sites[index - 1].block)?;
            let completion_slot = function.arity as u32 + input.len() as u32;
            prologue.push(Inst::new(
                await_result,
                InstKind::LoadLocal(pon_ir::LocalId(completion_slot)),
            ));
            for (offset, slot) in input.iter().copied().enumerate() {
                let loaded = Value(synthetic);
                let stored = Value(synthetic + 1);
                synthetic += 2;
                prologue.push(Inst::new(
                    loaded,
                    InstKind::LoadLocal(pon_ir::LocalId(function.arity as u32 + offset as u32)),
                ));
                prologue.push(Inst::new(
                    stored,
                    InstKind::StoreLocal(pon_ir::LocalId(slot + shift), loaded),
                ));
            }
            prologue.append(&mut current.insts);
            current.insts = prologue;
        }
        remap_blocks(&mut blocks)?;
        let mut native = clone_header(function);
        native.arity += input.len() + usize::from(resume_value);
        native.n_locals += input.len() + usize::from(resume_value);
        native.params.positional_count = native.arity;
        native
            .params
            .names
            .extend(input.iter().map(|slot| format!("spill_{slot}")));
        if resume_value {
            native.params.names.push("await_result".into());
        }
        native.blocks = blocks;
        let suspensions = sites
            .iter()
            .filter(|site| region.contains(&site.block))
            .map(|site| Suspension {
                tag: site.tag,
                output_locals: schemas[site.tag as usize].clone(),
            })
            .collect();
        result.push(Segment {
            function: native,
            input_locals: input,
            suspensions,
            resume_value,
        });
    }
    Ok(result)
}

fn walk_region(
    function: &Function,
    entry: BlockId,
    sites: &[AwaitSite],
) -> Result<Vec<BlockId>, SegmentError> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut active = HashSet::new();
    fn visit(
        function: &Function,
        id: BlockId,
        sites: &[AwaitSite],
        out: &mut Vec<BlockId>,
        seen: &mut HashSet<BlockId>,
        active: &mut HashSet<BlockId>,
    ) -> Result<(), SegmentError> {
        if active.contains(&id) {
            return Err(unsupported("CFG loop crosses a sequential region"));
        }
        if !seen.insert(id) {
            return Ok(());
        }
        let current = block(function, id)?;
        out.push(id);
        if sites.iter().any(|site| site.block == id)
            || matches!(current.term, Terminator::Return(_))
        {
            return Ok(());
        }
        active.insert(id);
        for next in successors(&current.term) {
            visit(function, next, sites, out, seen, active)?;
        }
        active.remove(&id);
        Ok(())
    }
    visit(function, entry, sites, &mut out, &mut seen, &mut active)?;
    Ok(out)
}

fn live_slots(
    function: &Function,
    region: &[BlockId],
    parameter_count: u32,
    sites: &[AwaitSite],
    schemas: &[Vec<u32>],
) -> Vec<u32> {
    let ids = region.iter().copied().collect::<HashSet<_>>();
    let mut live = region
        .iter()
        .map(|id| (*id, BTreeSet::new()))
        .collect::<HashMap<_, _>>();
    loop {
        let mut changed = false;
        for id in region.iter().rev() {
            let current = block(function, *id).expect("region block exists");
            let mut defined = BTreeSet::new();
            let mut use_before_def = BTreeSet::new();
            for inst in &current.insts {
                match inst.kind {
                    InstKind::LoadLocal(slot) if !defined.contains(&slot.0) => {
                        use_before_def.insert(slot.0);
                    }
                    InstKind::StoreLocal(slot, _) | InstKind::DeleteLocal(slot) => {
                        defined.insert(slot.0);
                    }
                    _ => {}
                }
            }
            let mut next = use_before_def;
            if let Some(site) = sites.iter().find(|site| site.block == current.id) {
                next.extend(
                    schemas[site.tag as usize]
                        .iter()
                        .copied()
                        .filter(|slot| !defined.contains(slot)),
                );
            }
            for successor in successors(&current.term) {
                if ids.contains(&successor) {
                    next.extend(
                        live[&successor]
                            .iter()
                            .copied()
                            .filter(|slot| !defined.contains(slot)),
                    );
                }
            }
            let entry = live.get_mut(id).unwrap();
            if *entry != next {
                *entry = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    live[&region[0]]
        .iter()
        .copied()
        .filter(|slot| *slot >= parameter_count)
        .collect()
}

fn await_resume_values(
    function: &Function,
    id: BlockId,
) -> Result<(usize, Value, HashSet<Value>), SegmentError> {
    let current = block(function, id)?;
    let position = current
        .insts
        .iter()
        .position(|inst| matches!(inst.kind, InstKind::Await { .. }))
        .ok_or_else(|| unsupported("missing await"))?;
    let await_result = match current.insts[position].kind {
        InstKind::Await { .. } => current.insts[position].result,
        _ => unreachable!(),
    };
    let values = current
        .insts
        .iter()
        .skip(position + 1)
        .filter_map(|inst| matches!(inst.kind, InstKind::GenResumePayload).then_some(inst.result))
        .collect();
    Ok((position, await_result, values))
}

fn post_await_result_slots(function: &Function, site: AwaitSite) -> Result<Vec<u32>, SegmentError> {
    let (position, await_result, resume_values) = await_resume_values(function, site.block)?;
    Ok(block(function, site.block)?
        .insts
        .iter()
        .skip(position + 1)
        .filter_map(|inst| match inst.kind {
            InstKind::StoreLocal(slot, value)
                if value == await_result || resume_values.contains(&value) =>
            {
                Some(slot.0)
            }
            _ => None,
        })
        .collect())
}

fn successors(term: &Terminator) -> Vec<BlockId> {
    match *term {
        Terminator::Jump(id) => vec![id],
        Terminator::Branch {
            then_blk, else_blk, ..
        } => vec![then_blk, else_blk],
        Terminator::CondBranch { then_, else_, .. } => vec![then_, else_],
        Terminator::ForLoop { body, done, .. } => vec![body, done],
        Terminator::Suspend { resume, .. } => vec![resume],
        _ => Vec::new(),
    }
}

fn remap_blocks(blocks: &mut [Block]) -> Result<(), SegmentError> {
    let map = blocks
        .iter()
        .enumerate()
        .map(|(n, b)| (b.id, BlockId(n as u32)))
        .collect::<HashMap<_, _>>();
    for (index, block) in blocks.iter_mut().enumerate() {
        block.id = BlockId(index as u32);
        for target in terminator_targets(&mut block.term) {
            *target = *map
                .get(target)
                .ok_or_else(|| unsupported("CFG target escapes native region"))?;
        }
    }
    Ok(())
}
fn terminator_targets(term: &mut Terminator) -> Vec<&mut BlockId> {
    match term {
        Terminator::Jump(id) => vec![id],
        Terminator::Branch {
            then_blk, else_blk, ..
        } => vec![then_blk, else_blk],
        Terminator::CondBranch { then_, else_, .. } => vec![then_, else_],
        Terminator::ForLoop { body, done, .. } => vec![body, done],
        Terminator::Suspend { resume, .. } => vec![resume],
        _ => Vec::new(),
    }
}
fn completion_block(function: &Function, await_block: BlockId) -> Result<BlockId, SegmentError> {
    let entry = block(function, await_block)?;
    let delegate = match entry.term {
        Terminator::Jump(target) => target,
        _ => return Err(unsupported("await entry is not a generator delegate")),
    };
    let delegate_block = block(function, delegate)?;
    match delegate_block.term {
        Terminator::ForLoop { body, done, .. }
            if matches!(block(function, body)?.term, Terminator::Suspend { .. }) =>
        {
            Ok(done)
        }
        _ => Err(unsupported("await has no completion block")),
    }
}
fn shift_compiler_local(kind: &mut InstKind, parameter_count: u32, shift: u32) {
    let slot = match kind {
        InstKind::LoadLocal(slot)
        | InstKind::StoreLocal(slot, _)
        | InstKind::DeleteLocal(slot)
        | InstKind::MakeCell(slot) => slot,
        _ => return,
    };
    if slot.0 >= parameter_count {
        slot.0 += shift;
    }
}
fn is_generator_payload(kind: &InstKind) -> bool {
    matches!(
        kind,
        InstKind::GenDelegateStep { .. } | InstKind::GenResumePayload | InstKind::GenLastStopValue
    )
}

fn replace_stop_values(block: &mut Block, completion_slot: u32) {
    for inst in &mut block.insts {
        if matches!(inst.kind, InstKind::GenLastStopValue) {
            inst.kind = InstKind::LoadLocal(pon_ir::LocalId(completion_slot));
        }
    }
}

fn max_value(function: &Function) -> u32 {
    function
        .blocks
        .iter()
        .flat_map(|b| b.insts.iter())
        .map(|i| i.result.0)
        .max()
        .unwrap_or(0)
}
fn clone_header(function: &Function) -> Function {
    Function {
        name: function.name.clone(),
        arity: function.arity,
        is_coroutine: false,
        is_generator: false,
        is_async_generator: false,
        params: function.params.clone(),
        blocks: Vec::new(),
        n_locals: function.n_locals,
    }
}
fn block(function: &Function, id: BlockId) -> Result<&Block, SegmentError> {
    function
        .blocks
        .iter()
        .find(|block| block.id == id)
        .ok_or_else(|| missing_block(id))
}
fn missing_block(id: BlockId) -> SegmentError {
    unsupported(format!("missing block {}", id.0))
}
fn unsupported(message: impl Into<String>) -> SegmentError {
    SegmentError::Unsupported(message.into())
}
