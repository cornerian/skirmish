//! Native executor for bounded sequential async moves.
//!
//! Every native entry uses the explicit v2 ABI: `Complete = [0, result]` and
//! `Waiting = [1, resume_tag, awaitable, ...scalar_spills]`.

use crate::continuation::{AwaitToken, Continuation, Scalar, ScalarLocal, source_identity};
use crate::sequential::{SequentialNativeImage, compile_sequential};
use crate::{Error, Value};
use pon_ir::{InstKind, Type, lower_source};
use sha2::{Digest, Sha256};

pub use crate::async_move::{MoveStep, PendingMove};

const SEQUENTIAL_ABI_DOMAIN: &[u8] = b"skirmish.pon.sequential.abi.v3";

pub struct SequentialMoveExecutor {
    image: SequentialNativeImage,
    source_identity: [u8; 32],
    move_identity: String,
    scalar_types: Vec<(u32, Type)>,
    pending: Option<PendingMove>,
}

impl SequentialMoveExecutor {
    pub fn new(source: &str, function_name: &str, move_identity: &str) -> Result<Self, Error> {
        if move_identity.is_empty() {
            return Err(Error::Compile("move identity is empty".into()));
        }
        let module = lower_source(source).map_err(|e| Error::Compile(e.to_string()))?;
        let function = crate::continuation::select_function(&module, function_name)
            .map_err(|e| Error::Compile(e.to_string()))?;
        let original = module
            .functions
            .get(function as usize)
            .ok_or_else(|| Error::Compile("sequential function disappeared".into()))?;
        let scalar_types = scalar_types(original)?;
        let image = compile_sequential(&module, pon_ir::FunctionId(function))?;
        Ok(Self {
            image,
            source_identity: sequential_source_identity(source, move_identity),
            move_identity: move_identity.to_owned(),
            scalar_types,
            pending: None,
        })
    }

    pub fn start_scoped<F>(&mut self, token: AwaitToken, invoke: F) -> Result<MoveStep, Error>
    where
        F: FnOnce(&mut crate::continuation::NativeEntry, &[Value]) -> Result<Value, Error>,
    {
        if self.pending.is_some() {
            return Err(Error::Runtime("sequential move is already pending".into()));
        }
        let value = invoke(&mut self.image.entries[0], &[])?;
        self.decode_result(0, token, value)
    }

    pub fn resume_scoped<F>(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        completion_value: Scalar,
        invoke: F,
    ) -> Result<Option<MoveStep>, Error>
    where
        F: FnOnce(&mut crate::continuation::NativeEntry, &[Value]) -> Result<Value, Error>,
    {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        if !self.validate_event(&pending.continuation, owner, generation, event_kind, frame)? {
            return Ok(None);
        }
        let state = pending.clone();
        let segment_index = state.continuation.resume_tag as usize;
        let args = self.segment_args(segment_index, &state.continuation, &completion_value)?;
        let value = invoke(&mut self.image.entries[segment_index], &args)?;
        self.decode_result(segment_index, state.continuation.await_token, value)
            .map(Some)
    }

    // The public ABI keeps owner/generation/event/deadline/completion separate
    // so callers cannot accidentally conflate token identity with its scalar.
    #[allow(clippy::too_many_arguments)]
    pub fn resume_scoped_with_token<F>(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        next_token: AwaitToken,
        completion_value: Scalar,
        invoke: F,
    ) -> Result<Option<MoveStep>, Error>
    where
        F: FnOnce(&mut crate::continuation::NativeEntry, &[Value]) -> Result<Value, Error>,
    {
        if let Some(pending) = self.pending.as_ref() {
            let mut candidate = pending.continuation.clone();
            candidate.await_token = next_token.clone();
            self.validate_state(&candidate)?;
        }
        let result = self.resume_scoped(
            owner,
            generation,
            event_kind,
            frame,
            completion_value,
            invoke,
        )?;
        if matches!(result, Some(MoveStep::Waiting { .. })) {
            self.update_pending_token(next_token)?;
            if let Some(MoveStep::Waiting { awaitable, .. }) = result {
                let pending = self.pending.clone().expect("waiting installs pending");
                return Ok(Some(MoveStep::Waiting { pending, awaitable }));
            }
        }
        Ok(result)
    }

    pub fn update_pending_token(&mut self, token: AwaitToken) -> Result<(), Error> {
        if token.event_kind.is_empty() {
            return Err(Error::Runtime("event kind is empty".into()));
        }
        let Some(pending) = self.pending.as_ref() else {
            return Err(Error::Runtime("sequential move is not pending".into()));
        };
        let mut state = pending.continuation.clone();
        state.await_token = token;
        self.validate_state(&state)?;
        self.pending
            .as_mut()
            .expect("pending checked above")
            .continuation = state;
        Ok(())
    }

    pub fn checkpoint(&self) -> Option<&PendingMove> {
        self.pending.as_ref()
    }
    pub fn restore_pending(&mut self, pending: Option<PendingMove>) -> Result<(), Error> {
        if let Some(ref p) = pending {
            self.validate_state(&p.continuation)?;
        }
        self.pending = pending;
        Ok(())
    }
    pub fn cancel(&mut self) -> bool {
        self.pending.take().is_some()
    }

    fn decode_result(
        &mut self,
        segment_index: usize,
        token: AwaitToken,
        value: Value,
    ) -> Result<MoveStep, Error> {
        let Value::List(values) = value else {
            return Err(Error::Value(
                "sequential entry must return an ABI list".into(),
            ));
        };
        let Some(Value::Int(kind)) = values.first() else {
            return Err(Error::Value(
                "sequential entry returned invalid result kind".into(),
            ));
        };
        match *kind {
            0 => {
                if values.len() != 2 {
                    return Err(Error::Value(
                        "sequential complete result has wrong arity".into(),
                    ));
                }
                self.pending = None;
                Ok(MoveStep::Complete(values[1].clone()))
            }
            1 => self.decode_waiting(segment_index, token, values),
            _ => Err(Error::Value(
                "sequential entry returned unknown result kind".into(),
            )),
        }
    }

    fn decode_waiting(
        &mut self,
        segment_index: usize,
        token: AwaitToken,
        values: Vec<Value>,
    ) -> Result<MoveStep, Error> {
        let segment = self
            .image
            .segments
            .get(segment_index)
            .ok_or_else(|| Error::Runtime("sequential segment index out of bounds".into()))?;
        let Some(Value::Int(raw_tag)) = values.get(1) else {
            return Err(Error::Value(
                "sequential waiting result has invalid resume tag".into(),
            ));
        };
        if *raw_tag < 1 || *raw_tag as usize >= self.image.entries.len() {
            return Err(Error::Value(
                "sequential waiting result has out-of-range resume tag".into(),
            ));
        }
        let tag = *raw_tag as u32;
        let suspension = segment
            .suspensions
            .iter()
            .find(|s| s.tag == tag)
            .ok_or_else(|| {
                Error::Value("sequential waiting result has unexpected resume tag".into())
            })?;
        let expected = 3 + suspension.output_locals.len();
        if values.len() != expected {
            return Err(Error::Value(format!(
                "sequential waiting result returned {} values, expected {expected}",
                values.len()
            )));
        }
        let locals = suspension
            .output_locals
            .iter()
            .zip(values[3..].iter())
            .map(|(slot, value)| {
                Ok(ScalarLocal {
                    slot: *slot,
                    value: scalar_from_value(value)?,
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let state = Continuation {
            source_identity: self.source_identity,
            move_identity: self.move_identity.clone(),
            resume_tag: tag,
            locals,
            await_token: token,
        };
        self.validate_state(&state)?;
        let pending = PendingMove {
            continuation: state,
        };
        let awaitable = values[2].clone();
        self.pending = Some(pending.clone());
        Ok(MoveStep::Waiting { pending, awaitable })
    }

    fn segment_args(
        &self,
        index: usize,
        state: &Continuation,
        completion: &Scalar,
    ) -> Result<Vec<Value>, Error> {
        let segment = self
            .image
            .segments
            .get(index)
            .ok_or_else(|| Error::Runtime("resume tag out of bounds".into()))?;
        if state.locals.len() != segment.input_locals.len() {
            return Err(Error::Runtime("resume scalar spill arity mismatch".into()));
        }
        let mut args = segment
            .input_locals
            .iter()
            .map(|slot| {
                state
                    .locals
                    .iter()
                    .find(|l| l.slot == *slot)
                    .map(|l| value_from_scalar(&l.value))
                    .ok_or_else(|| Error::Runtime("missing resume scalar spill".into()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if segment.resume_value {
            match completion {
                Scalar::Float(value) if !value.is_finite() => {
                    return Err(Error::Runtime(
                        "completion scalar float is not finite".into(),
                    ));
                }
                _ => {}
            }
            args.push(value_from_scalar(completion));
        }
        Ok(args)
    }
    fn validate_event(
        &self,
        state: &Continuation,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
    ) -> Result<bool, Error> {
        self.validate_state(state)?;
        Ok(state
            .await_token
            .matches(owner, generation, event_kind, frame))
    }
    fn validate_state(&self, state: &Continuation) -> Result<(), Error> {
        if state.source_identity != self.source_identity {
            return Err(Error::Runtime("sequential source identity mismatch".into()));
        }
        if state.move_identity != self.move_identity {
            return Err(Error::Runtime("sequential move identity mismatch".into()));
        }
        if state.resume_tag == 0 || state.resume_tag as usize >= self.image.entries.len() {
            return Err(Error::Runtime("sequential resume tag is invalid".into()));
        }
        if state.await_token.event_kind.is_empty() {
            return Err(Error::Runtime("event kind is empty".into()));
        }
        let segment = &self.image.segments[state.resume_tag as usize];
        if state.locals.len() != segment.input_locals.len() {
            return Err(Error::Runtime(
                "sequential scalar spill arity mismatch".into(),
            ));
        }
        for (slot, local) in segment.input_locals.iter().zip(&state.locals) {
            if local.slot != *slot {
                return Err(Error::Runtime(
                    "sequential scalar spill slot mismatch".into(),
                ));
            }
            // A slot that the lowering cannot assign one static type to is
            // represented as `Object` and checked by the scalar decoder.
            let ty = self
                .scalar_types
                .iter()
                .find(|(candidate, _)| candidate == slot)
                .map(|(_, ty)| *ty)
                .unwrap_or(Type::Object);
            if !scalar_matches(ty, &local.value) {
                return Err(Error::Runtime(
                    "sequential scalar spill type mismatch".into(),
                ));
            }
        }
        Ok(())
    }
}

fn sequential_source_identity(source: &str, move_identity: &str) -> [u8; 32] {
    let base = source_identity(source, move_identity);
    let mut hash = Sha256::new();
    hash.update(SEQUENTIAL_ABI_DOMAIN);
    hash.update(base);
    hash.finalize().into()
}
fn scalar_types(function: &pon_ir::Function) -> Result<Vec<(u32, Type)>, Error> {
    let mut values = std::collections::HashMap::new();
    let mut slots = std::collections::BTreeMap::new();
    for block in &function.blocks {
        for inst in &block.insts {
            let ty = match inst.kind {
                InstKind::Const(pon_ir::PyConst::Int(_)) => Type::IntI64,
                InstKind::Const(pon_ir::PyConst::Float(_)) => Type::Float,
                InstKind::Const(pon_ir::PyConst::Bool(_)) => Type::Bool,
                _ => inst.static_type,
            };
            values.insert(inst.result.0, ty);
            if let InstKind::StoreLocal(slot, value) = inst.kind {
                let ty = values.get(&value.0).copied().unwrap_or(Type::Object);
                slots
                    .entry(slot.0)
                    .and_modify(|old| {
                        if *old != ty {
                            // A slot written with different static scalar types
                            // has no single sound compile-time schema. Preserve
                            // it as unknown and validate the copied ABI scalar
                            // at runtime instead of depending on traversal order.
                            *old = Type::Object;
                        }
                    })
                    .or_insert(ty);
            }
        }
    }
    Ok(slots.into_iter().collect())
}
fn scalar_from_value(value: &Value) -> Result<Scalar, Error> {
    Ok(match value {
        Value::None => Scalar::None,
        Value::Int(v) => Scalar::Int(*v),
        Value::F32(v) => Scalar::Float(*v as f64),
        Value::Bool(v) => Scalar::Bool(*v),
        _ => return Err(Error::Value("sequential spill is not scalar".into())),
    })
}
fn value_from_scalar(value: &Scalar) -> Value {
    match value {
        Scalar::None => Value::None,
        Scalar::Int(v) => Value::Int(*v),
        Scalar::Float(v) => Value::F32(*v as f32),
        Scalar::Bool(v) => Value::Bool(*v),
    }
}
fn scalar_matches(ty: Type, value: &Scalar) -> bool {
    match (ty, value) {
        (Type::IntI64, Scalar::Int(_))
        | (Type::Bool, Scalar::Bool(_))
        | (Type::Bottom, Scalar::None) => true,
        (Type::Float, Scalar::Float(v)) => v.is_finite(),
        (Type::Object, Scalar::Int(_) | Scalar::Float(_) | Scalar::Bool(_) | Scalar::None) => true,
        _ => false,
    }
}
