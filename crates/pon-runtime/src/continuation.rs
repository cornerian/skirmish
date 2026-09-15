//! Rollback safe continuation metadata for the restricted async move ABI.
//!
//! This module deliberately does not keep a Pon generator frame.  The frame
//! is an execution detail of a prepared image; a checkpoint contains only the
//! source identity, state number, scalar locals, and copied event token.

use pon_ir::{InstKind, Terminator, Type, lower_source};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

use crate::Error;
/// Stable external continuation ABI version.  It is included in the source
/// identity so a checkpoint cannot be resumed by an adapter with a different
/// native step calling convention.
pub const CONTINUATION_ABI_VERSION: u32 = crate::compiler_identity::CONTINUATION_ABI_VERSION;
pub const BRIDGE_ABI_VERSION: u32 = crate::compiler_identity::BRIDGE_ABI_VERSION;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepPhase {
    Pre,
    Post,
}

/// A value which is safe to put in a rollback record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum Scalar {
    Int(i64),
    Float(f64),
    Bool(bool),
    None,
}

/// One named local carried across an await boundary.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScalarLocal {
    pub slot: u32,
    pub value: Scalar,
}

/// Immutable event ownership copied into a continuation record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitToken {
    pub owner: u64,
    pub generation: u64,
    pub event_kind: String,
    pub deadline_frame: u64,
}

impl AwaitToken {
    /// Match an event delivered by the native registry.  Generation and owner
    /// are checked before event kind, so a late event from an interrupted move
    /// cannot resume a replacement action.
    pub fn matches(&self, owner: u64, generation: u64, event_kind: &str, frame: u64) -> bool {
        self.owner == owner
            && self.generation == generation
            && self.event_kind == event_kind
            && (event_kind != "scheduled_deadline" || frame >= self.deadline_frame)
    }
}

/// The only execution state required to resume a supported move.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Continuation {
    pub source_identity: [u8; 32],
    pub move_identity: String,
    pub resume_tag: u32,
    pub locals: Vec<ScalarLocal>,
    pub await_token: AwaitToken,
}

/// A statically validated await point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AwaitPoint {
    pub resume_tag: u32,
    pub block: u32,
    pub awaitable: u32,
    pub next_block: u32,
}

/// Validated lowering result.  `NativeImage` is optional and owns only JIT
/// code; it is intentionally not part of [`Continuation`].
#[derive(Clone, Debug, PartialEq)]
pub struct ContinuationPlan {
    pub source_identity: [u8; 32],
    pub move_identity: String,
    pub function_name: String,
    pub function_index: u32,
    pub locals: Vec<(u32, Type)>,
    pub await_point: AwaitPoint,
}

#[derive(Debug)]
pub enum ContinuationError {
    Lower(String),
    Unsupported(String),
    InvalidState(String),
}

impl fmt::Display for ContinuationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lower(e) => write!(f, "continuation lowering failed: {e}"),
            Self::Unsupported(e) => write!(f, "unsupported async continuation: {e}"),
            Self::InvalidState(e) => write!(f, "invalid continuation state: {e}"),
        }
    }
}

impl std::error::Error for ContinuationError {}

/// Lower and validate one async function using Pon's existing frontend/IR.
pub fn lower_continuation(
    source: &str,
    function_name: &str,
    move_identity: &str,
) -> Result<ContinuationPlan, ContinuationError> {
    let module = lower_source(source).map_err(|e| ContinuationError::Lower(e.to_string()))?;
    let index = select_function(&module, function_name)? as usize;
    let function = &module.functions[index];
    if !function.is_coroutine || function.is_async_generator {
        return Err(ContinuationError::Unsupported(format!(
            "`{function_name}` must be a plain async def coroutine"
        )));
    }

    let mut awaits = Vec::new();
    let mut scalar_slots = std::collections::BTreeMap::<u32, Type>::new();
    let mut value_types = std::collections::HashMap::<u32, Type>::new();
    for block in &function.blocks {
        for inst in &block.insts {
            let inferred = match inst.kind {
                InstKind::Const(pon_ir::PyConst::Int(_)) => Type::IntI64,
                InstKind::Const(pon_ir::PyConst::Float(_)) => Type::Float,
                InstKind::Const(pon_ir::PyConst::Bool(_)) => Type::Bool,
                _ => inst.static_type,
            };
            value_types.insert(inst.result.0, inferred);
            if let InstKind::Await { awaitable } = &inst.kind {
                awaits.push((block.id.0, inst.result.0, awaitable.0));
            }
            if let InstKind::StoreLocal(slot, value) = &inst.kind {
                let ty = value_types.get(&value.0).copied().unwrap_or(Type::Object);
                // Generator lowering adds boxed spill slots for SSA values.
                // They are deliberately excluded from the external record;
                // only statically scalar slots can cross the boundary.
                if matches!(ty, Type::IntI64 | Type::Float | Type::Bool | Type::Bottom) {
                    scalar_slots.insert(slot.0, ty);
                }
            }
        }
    }
    if awaits.len() != 1 {
        return Err(ContinuationError::Unsupported(format!(
            "expected exactly one await boundary, found {}",
            awaits.len()
        )));
    }
    if scalar_slots.len() > 1 {
        return Err(ContinuationError::Unsupported(format!(
            "continuation ABI supports zero or one scalar live local, found {}",
            scalar_slots.len()
        )));
    }
    let (block, _await_result, awaitable) = awaits[0];
    let suspends = function
        .blocks
        .iter()
        .filter_map(|b| match &b.term {
            Terminator::Suspend { state, resume, .. } => Some((*state, resume.0)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let suspend = suspends.first().copied().ok_or_else(|| {
        ContinuationError::Unsupported("await has no lowered suspend point".into())
    })?;
    let suspend_block = function
        .blocks
        .iter()
        .find(|b| matches!(b.term, Terminator::Suspend { .. }))
        .map(|b| b.id.0)
        .unwrap();
    let post_block = function
        .blocks
        .iter()
        .find_map(|b| match b.term {
            Terminator::ForLoop { body, done, .. } if body.0 == suspend_block => Some(done.0),
            _ => None,
        })
        .ok_or_else(|| {
            ContinuationError::Unsupported("await delegation has no post-await exit block".into())
        })?;
    let source_identity = source_identity(source, move_identity);
    Ok(ContinuationPlan {
        source_identity,
        move_identity: move_identity.to_owned(),
        function_name: function_name.to_owned(),
        function_index: index as u32,
        locals: scalar_slots.into_iter().collect(),
        await_point: AwaitPoint {
            resume_tag: suspend.0,
            block,
            awaitable,
            next_block: post_block,
        },
    })
}

pub(crate) fn select_function(
    module: &pon_ir::Module,
    requested: &str,
) -> Result<u32, ContinuationError> {
    if let Some((class_name, method_name)) = requested.rsplit_once('.') {
        let mut matches = Vec::new();
        for function in &module.functions {
            for block in &function.blocks {
                for inst in &block.insts {
                    let InstKind::BuildClass { body, name, .. } = inst.kind else {
                        continue;
                    };
                    if module.names.get(name.0 as usize).map(String::as_str) != Some(class_name) {
                        continue;
                    }
                    let Some(class_body) = module.functions.get(body.0 as usize) else {
                        continue;
                    };
                    for class_block in &class_body.blocks {
                        for method in &class_block.insts {
                            let func_index = match method.kind {
                                InstKind::MakeFunction {
                                    func_index,
                                    name_interned,
                                    ..
                                } if module
                                    .names
                                    .get(name_interned.0 as usize)
                                    .map(String::as_str)
                                    == Some(method_name) =>
                                {
                                    Some(func_index)
                                }
                                InstKind::MakeFunctionFull { code, .. }
                                    if module
                                        .functions
                                        .get(code.0 as usize)
                                        .is_some_and(|function| function.name == method_name) =>
                                {
                                    Some(code.0)
                                }
                                _ => None,
                            };
                            if let Some(func_index) = func_index {
                                matches.push(func_index);
                            }
                        }
                    }
                }
            }
        }
        matches.sort_unstable();
        matches.dedup();
        return match matches.as_slice() {
            [index] => Ok(*index),
            [] => Err(ContinuationError::Unsupported(format!(
                "method `{requested}` was not lowered"
            ))),
            _ => Err(ContinuationError::Unsupported(format!(
                "method `{requested}` is ambiguous"
            ))),
        };
    }
    let matches = module
        .functions
        .iter()
        .enumerate()
        .filter_map(|(index, function)| (function.name == requested).then_some(index as u32))
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(ContinuationError::Unsupported(format!(
            "function `{requested}` was not lowered"
        ))),
        _ => Err(ContinuationError::Unsupported(format!(
            "function `{requested}` is ambiguous; use DeclaringClass.{requested}"
        ))),
    }
}

impl ContinuationPlan {
    pub fn initial(
        &self,
        move_identity: &str,
        token: AwaitToken,
    ) -> Result<Continuation, ContinuationError> {
        if move_identity.is_empty() {
            return Err(ContinuationError::InvalidState(
                "move identity is empty".into(),
            ));
        }
        if move_identity != self.move_identity {
            return Err(ContinuationError::InvalidState(
                "move identity mismatch".into(),
            ));
        }
        if token.event_kind.is_empty() {
            return Err(ContinuationError::InvalidState(
                "event kind is empty".into(),
            ));
        }
        Ok(Continuation {
            source_identity: self.source_identity,
            move_identity: move_identity.to_owned(),
            resume_tag: self.await_point.resume_tag,
            locals: Vec::new(),
            await_token: token,
        })
    }

    pub fn validate(&self, state: &Continuation) -> Result<(), ContinuationError> {
        if state.source_identity != self.source_identity {
            return Err(ContinuationError::InvalidState(
                "source identity mismatch".into(),
            ));
        }
        if state.resume_tag != self.await_point.resume_tag {
            return Err(ContinuationError::InvalidState(
                "resume tag does not belong to this plan".into(),
            ));
        }
        if state.move_identity.is_empty() {
            return Err(ContinuationError::InvalidState(
                "move identity is empty".into(),
            ));
        }
        if state.move_identity != self.move_identity {
            return Err(ContinuationError::InvalidState(
                "move identity mismatch".into(),
            ));
        }
        if state.locals.len() > self.locals.len() {
            return Err(ContinuationError::InvalidState(format!(
                "expected at most {} scalar live locals, found {}",
                self.locals.len(),
                state.locals.len()
            )));
        }
        if state.await_token.event_kind.is_empty() {
            return Err(ContinuationError::InvalidState(
                "event kind is empty".into(),
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for local in &state.locals {
            if !seen.insert(local.slot) {
                return Err(ContinuationError::InvalidState(format!(
                    "duplicate scalar local slot {}",
                    local.slot
                )));
            }
            let Some((_, ty)) = self.locals.iter().find(|(slot, _)| *slot == local.slot) else {
                return Err(ContinuationError::InvalidState(
                    "unknown scalar local slot".into(),
                ));
            };
            let valid = matches!(
                (ty, &local.value),
                (Type::IntI64, Scalar::Int(_))
                    | (Type::Float, Scalar::Float(_))
                    | (Type::Bool, Scalar::Bool(_))
                    | (Type::Bottom, Scalar::None)
            );
            if !valid {
                return Err(ContinuationError::InvalidState(format!(
                    "scalar local slot {} has the wrong type",
                    local.slot
                )));
            }
            if let Scalar::Float(value) = local.value
                && !value.is_finite()
            {
                return Err(ContinuationError::InvalidState(
                    "non-finite float local".into(),
                ));
            }
        }
        Ok(())
    }

    /// Return whether a delivered native event may consume this checkpoint.
    /// The check is deliberately side effect free; callers remove the record
    /// only after the post step commits successfully.
    pub fn event_matches(
        &self,
        state: &Continuation,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
    ) -> Result<bool, ContinuationError> {
        self.validate(state)?;
        if state.locals.len() != self.locals.len() {
            return Err(ContinuationError::InvalidState(format!(
                "resume checkpoint requires {} scalar live locals, found {}",
                self.locals.len(),
                state.locals.len()
            )));
        }
        Ok(state
            .await_token
            .matches(owner, generation, event_kind, frame))
    }

    /// Invalidate a checkpoint before dropping its disposable native image.
    /// Cancellation is represented by removal in the host; this helper keeps
    /// the operation explicit for callers that need to distinguish a stale
    /// record from a successfully cancelled one.
    pub fn cancel(&self, state: &Continuation) -> Result<(), ContinuationError> {
        self.validate(state)
    }

    /// Compile a native image with the pinned Pon JIT. The image is a
    /// disposable execution artifact and never enters the checkpoint record.
    ///
    /// The entry is an IR-derived pre-await step, rather than the original
    /// coroutine wrapper.  The current restricted slice requires the await
    /// block to be the entry block and has no branch before it; richer CFGs
    /// are rejected until their state/local ABI is implemented.
    pub fn compile_native(&self, source: &str) -> Result<NativeStep, Error> {
        if source_identity(source, &self.move_identity) != self.source_identity {
            return Err(Error::Compile(
                "continuation source identity mismatch".into(),
            ));
        }
        let _lock = crate::PON_RUNTIME_LOCK
            .lock()
            .expect("Pon runtime lock poisoned");
        let parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = crate::safety::Attachment::acquire().map_err(Error::Runtime)?;
        let result = self.compile_native_active(source);
        drop(_attachment);
        parked.restore();
        result
    }

    fn compile_native_active(&self, source: &str) -> Result<NativeStep, Error> {
        if self.locals.len() > 1 {
            return Err(Error::Compile(format!(
                "continuation ABI supports zero or one scalar live local, found {}",
                self.locals.len()
            )));
        }
        let mut module = lower_source(source).map_err(|e| Error::Compile(e.to_string()))?;
        let function = module
            .functions
            .get_mut(self.function_index as usize)
            .ok_or_else(|| {
                Error::Compile("continuation function disappeared after lowering".into())
            })?;
        let pre_arity = function.arity;
        if self.await_point.block != 0 || function.blocks.len() < 2 {
            return Err(Error::Compile(
                "pre-await native step requires a straight-line entry block".into(),
            ));
        }
        let block = function
            .blocks
            .iter_mut()
            .find(|b| b.id.0 == self.await_point.block)
            .ok_or_else(|| Error::Compile("await block disappeared after lowering".into()))?;
        let await_index = block
            .insts
            .iter()
            .position(|inst| matches!(inst.kind, InstKind::Await { .. }))
            .ok_or_else(|| Error::Compile("await instruction disappeared after lowering".into()))?;
        let awaitable = match block.insts[await_index].kind {
            InstKind::Await { awaitable } => awaitable,
            _ => unreachable!(),
        };
        block.insts.truncate(await_index);
        block
            .insts
            .retain(|inst| !matches!(inst.kind, InstKind::GenResumePayload));
        let next_value = block
            .insts
            .iter()
            .map(|inst| inst.result.0)
            .max()
            .unwrap_or(0)
            + 1;
        let mut tuple_elts = vec![awaitable];
        if let Some((live_slot, _)) = self.locals.first() {
            let local_value = pon_ir::Value(next_value);
            block.insts.push(pon_ir::Inst::new(
                local_value,
                InstKind::LoadLocal(pon_ir::LocalId(*live_slot)),
            ));
            tuple_elts.push(local_value);
        }
        let tuple_value = pon_ir::Value(next_value + 1);
        block.insts.push(pon_ir::Inst::new(
            tuple_value,
            InstKind::BuildTuple { elts: tuple_elts },
        ));
        block.term = Terminator::Return(tuple_value);
        function.blocks.truncate(1);
        function.is_coroutine = false;
        function.is_generator = false;
        function.is_async_generator = false;
        module.main = pon_ir::FunctionId(self.function_index);
        let mut pre_engine = pon_jit::JitEngine::new();
        let pre_entry = pre_engine
            .compile(&module)
            .map_err(|e| Error::Compile(e.to_string()))?;

        // Build the post-await image from the lowered resume block. The
        // external ABI supplies its one scalar live-in as argument zero.
        let mut post_module = lower_source(source).map_err(|e| Error::Compile(e.to_string()))?;
        let post_fn = post_module
            .functions
            .get_mut(self.function_index as usize)
            .ok_or_else(|| {
                Error::Compile("continuation function disappeared after lowering".into())
            })?;
        let post_input_arity = post_fn.arity;
        let mut post_param_names = post_fn.params.names.clone();
        let mut post_block = post_fn
            .blocks
            .iter()
            .find(|b| b.id.0 == self.await_point.next_block)
            .cloned()
            .ok_or_else(|| Error::Compile("lowered await resume block disappeared".into()))?;
        post_block.id = pon_ir::BlockId(0);
        post_block
            .insts
            .retain(|inst| !matches!(inst.kind, InstKind::GenResumePayload));
        let planned_slot = self.locals.first().map(|local| local.0);
        let mut live_slots = std::collections::BTreeSet::new();
        for inst in &mut post_block.insts {
            if let InstKind::LoadLocal(slot) = &mut inst.kind {
                // Positional parameters remain available across the await;
                // only compiler locals need rebinding to the scalar resume
                // slot below.
                if slot.0 < post_input_arity as u32 {
                    continue;
                }
                live_slots.insert(slot.0);
                if Some(slot.0) != planned_slot {
                    return Err(Error::Compile(format!(
                        "resume block loads unsupported scalar spill slot {}, expected {:?}",
                        slot.0, planned_slot
                    )));
                }
                *slot = pon_ir::LocalId(post_input_arity as u32);
            }
        }
        if live_slots.len() > 1 {
            return Err(Error::Compile(format!(
                "resume block requires zero or one scalar live-in, found {}",
                live_slots.len()
            )));
        }
        post_fn.blocks = vec![post_block];
        post_fn.params = if planned_slot.is_some() {
            post_param_names.push("live_scalar".into());
            pon_ir::ir::ParamLayout {
                names: post_param_names,
                positional_count: post_input_arity + 1,
                ..Default::default()
            }
        } else {
            pon_ir::ir::ParamLayout {
                names: post_param_names,
                positional_count: post_input_arity,
                ..Default::default()
            }
        };
        post_fn.arity = post_input_arity + usize::from(planned_slot.is_some());
        post_fn.is_coroutine = false;
        post_fn.is_generator = false;
        post_fn.is_async_generator = false;
        post_module.main = pon_ir::FunctionId(self.function_index);
        let mut post_engine = pon_jit::JitEngine::new();
        let post_entry = post_engine
            .compile(&post_module)
            .map_err(|e| Error::Compile(e.to_string()))?;
        Ok(NativeStep {
            _pre_engine: pre_engine,
            pre_entry,
            pre_arity,
            _post_engine: post_engine,
            post_entry,
            post_input_arity,
            post_has_scalar: planned_slot.is_some(),
        })
    }
}

/// Native machine code for one transformed continuation step.
pub struct NativeStep {
    _pre_engine: pon_jit::JitEngine,
    pre_entry: pon_jit::MainFn,
    pre_arity: usize,
    _post_engine: pon_jit::JitEngine,
    post_entry: pon_jit::MainFn,
    post_input_arity: usize,
    post_has_scalar: bool,
}

/// One owned native entry compiled from an already transformed Pon module.
/// The module is retained by the JIT engine, so callers may compile several
/// sequential segments without relowering the original source.
pub struct NativeEntry {
    _engine: pon_jit::JitEngine,
    #[allow(dead_code)]
    entry: pon_jit::MainFn,
    arity: usize,
}

/// Compile one selected function from an already transformed module.
pub fn compile_module_entry(
    module: pon_ir::Module,
    function: pon_ir::FunctionId,
) -> Result<NativeEntry, Error> {
    NativeEntry::compile_module_entry(module, function)
}

impl NativeEntry {
    /// Compile `function` as the module's native entry point. Compilation is
    /// thread-affine and follows the same runtime/GC guards as native steps.
    pub fn compile_module_entry(
        mut module: pon_ir::Module,
        function: pon_ir::FunctionId,
    ) -> Result<Self, Error> {
        let function_index = function.0 as usize;
        let arity = module
            .functions
            .get(function_index)
            .ok_or_else(|| Error::Compile("native entry function disappeared".into()))?
            .arity;
        module.main = function;
        let _lock = crate::PON_RUNTIME_LOCK
            .lock()
            .expect("Pon runtime lock poisoned");
        let parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = crate::safety::Attachment::acquire().map_err(Error::Runtime)?;
        if unsafe { pon_runtime::abi::pon_runtime_init() } != 0 {
            return Err(Error::Runtime(crate::diagnostic()));
        }
        let mut engine = pon_jit::JitEngine::new();
        let entry = engine
            .compile(&module)
            .map_err(|error| Error::Compile(error.to_string()))?;
        drop(_attachment);
        parked.restore();
        Ok(Self {
            _engine: engine,
            entry,
            arity,
        })
    }

    #[must_use]
    pub fn arity(&self) -> usize {
        self.arity
    }

    /// Invoke inside a caller-owned active runtime scope and rooted argv.
    #[allow(dead_code)]
    pub(crate) unsafe fn call_active(
        &mut self,
        argv: *mut *mut pon_runtime::PyObject,
        argc: usize,
    ) -> *mut pon_runtime::PyObject {
        if argc != self.arity {
            return std::ptr::null_mut();
        }
        // SAFETY: the caller owns the runtime lock, active GC region, and
        // rooted argv for this exact arity.
        unsafe { (self.entry)(argv, argc) }
    }
}

impl NativeStep {
    /// Invoke an already prepared native entry while the caller owns the Pon
    /// runtime lock, GC-safe region, attachment, import policy, and rooted
    /// argument storage. Only the prepared gameplay bridge may call this.
    /// `argv` must point to `argc` valid rooted Pon objects for the selected
    /// phase; the returned object is owned by the caller's rooted scope.
    pub(crate) unsafe fn call_active(
        &mut self,
        phase: StepPhase,
        argv: *mut *mut pon_runtime::PyObject,
        argc: usize,
    ) -> *mut pon_runtime::PyObject {
        let (entry, expected) = match phase {
            StepPhase::Pre => (self.pre_entry, self.pre_arity),
            StepPhase::Post => (
                self.post_entry,
                self.post_input_arity + usize::from(self.post_has_scalar),
            ),
        };
        if argc != expected {
            return std::ptr::null_mut();
        }
        // SAFETY: the bridge contract requires `argv` to reference `argc`
        // rooted objects while the caller-owned runtime guard is active.
        unsafe { entry(argv, argc) }
    }

    fn invoke_values(
        &mut self,
        entry: pon_jit::MainFn,
        args: &[crate::Value],
    ) -> Result<crate::Value, Error> {
        let _lock = crate::PON_RUNTIME_LOCK
            .lock()
            .expect("Pon runtime lock poisoned");
        let parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = crate::safety::Attachment::acquire().map_err(Error::Runtime)?;
        if unsafe { pon_runtime::abi::pon_runtime_init() } != 0 {
            return Err(Error::Runtime(crate::diagnostic()));
        }
        let mut marker = 0usize;
        let _stack =
            unsafe { crate::safety::StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let storage = crate::safety::RootedVec::new();
        let mut boxed = storage.guard();
        for value in args {
            boxed.push(crate::box_value(value)?);
        }
        let result = unsafe { (entry)(boxed.as_mut_ptr(), boxed.len()) };
        let value = if result.is_null() {
            Err(Error::Runtime(crate::diagnostic()))
        } else {
            boxed.push(result);
            crate::unbox_value(result)
        };
        drop(boxed);
        drop(storage);
        drop(_stack);
        drop(_attachment);
        parked.restore();
        value
    }

    pub fn call_values(&mut self, args: &[crate::Value]) -> Result<crate::Value, Error> {
        if args.len() != self.pre_arity {
            return Err(Error::Compile(format!(
                "pre-await step expects {} host arguments, found {}",
                self.pre_arity,
                args.len()
            )));
        }
        self.invoke_values(self.pre_entry, args)
    }

    pub fn call_post_value(&mut self, value: crate::Value) -> Result<crate::Value, Error> {
        self.call_post_with_args(&[], Some(value))
    }

    /// Resume a continuation whose lowered post-await body has no external
    /// scalar locals.  The empty argument list is part of the native ABI.
    pub fn call_post(&mut self) -> Result<crate::Value, Error> {
        self.call_post_with_args(&[], None)
    }

    /// Resume while rebinding fresh host-owned `self`/context arguments.
    /// These values are supplied by the game bridge for every event and are
    /// never retained in [`Continuation`].
    pub fn call_post_with_args(
        &mut self,
        args: &[crate::Value],
        scalar: Option<crate::Value>,
    ) -> Result<crate::Value, Error> {
        if args.len() != self.post_input_arity {
            return Err(Error::Compile(format!(
                "post-await step expects {} host arguments, found {}",
                self.post_input_arity,
                args.len()
            )));
        }
        if scalar.is_some() != self.post_has_scalar {
            return Err(Error::Compile(
                "post-await scalar argument does not match lowered ABI".into(),
            ));
        }
        let mut values = args.to_vec();
        if let Some(scalar) = scalar {
            values.push(scalar);
        }
        self.invoke_values(self.post_entry, &values)
    }
}

/// Keep the source identity ergonomically available to callers using Program.
pub fn source_identity(source: &str, move_identity: &str) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"skirmish.pon.continuation\0");
    hash.update(CONTINUATION_ABI_VERSION.to_le_bytes());
    hash.update(BRIDGE_ABI_VERSION.to_le_bytes());
    hash.update(crate::compiler_identity::compiler_identity());
    hash.update((source.len() as u64).to_le_bytes());
    hash.update(source.as_bytes());
    hash.update((move_identity.len() as u64).to_le_bytes());
    hash.update(move_identity.as_bytes());
    hash.finalize().into()
}
