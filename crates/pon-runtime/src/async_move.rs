//! Rollback-safe driver for one lowered `Move.run` await boundary.
//!
//! The driver owns a disposable native image only while executing. Its
//! pending state is copied into [`PendingMove`] before returning to gameplay,
//! so rollback never retains a Pon generator, host handle, or JIT pointer.

use crate::continuation::{
    AwaitToken, Continuation, ContinuationError, ContinuationPlan, NativeStep, Scalar, ScalarLocal,
};
use crate::{Error, Value};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PendingMove {
    pub continuation: Continuation,
}

#[derive(Debug)]
pub enum MoveStep {
    Waiting {
        pending: PendingMove,
        awaitable: Value,
    },
    Complete(Value),
}

pub struct AsyncMoveExecutor {
    plan: ContinuationPlan,
    image: NativeStep,
    pending: Option<PendingMove>,
}

impl AsyncMoveExecutor {
    pub fn new(plan: ContinuationPlan, source: &str) -> Result<Self, Error> {
        let image = plan.compile_native(source)?;
        Ok(Self {
            plan,
            image,
            pending: None,
        })
    }

    /// Recreate an executor from rollback data without executing the entry
    /// step again. The native image is rebuilt from immutable source and the
    /// copied continuation is validated before it becomes active.
    pub fn restore(
        plan: ContinuationPlan,
        source: &str,
        pending: PendingMove,
    ) -> Result<Self, Error> {
        plan.event_matches(
            &pending.continuation,
            pending.continuation.await_token.owner,
            pending.continuation.await_token.generation,
            &pending.continuation.await_token.event_kind,
            pending.continuation.await_token.deadline_frame,
        )
        .map_err(|e| Error::Runtime(e.to_string()))?;
        let image = plan.compile_native(source)?;
        Ok(Self {
            plan,
            image,
            pending: Some(pending),
        })
    }

    pub fn start(&mut self, args: &[Value], token: AwaitToken) -> Result<MoveStep, Error> {
        if self.pending.is_some() {
            return Err(Error::Runtime("async move is already pending".into()));
        }
        let value = self.image.call_values(args)?;
        self.start_result(token, value)
    }

    /// Execute the pre step through a caller-owned scoped native bridge.
    pub fn start_scoped<F>(&mut self, token: AwaitToken, invoke: F) -> Result<MoveStep, Error>
    where
        F: FnOnce(
            &mut NativeStep,
            crate::continuation::StepPhase,
            Option<Value>,
        ) -> Result<Value, Error>,
    {
        if self.pending.is_some() {
            return Err(Error::Runtime("async move is already pending".into()));
        }
        let value = invoke(&mut self.image, crate::continuation::StepPhase::Pre, None)?;
        self.start_result(token, value)
    }

    /// Consume a pre-await result produced by an externally scoped native
    /// invoker. The invoker owns Pon runtime/host scope; this method owns only
    /// decoding and checkpoint creation.
    pub fn start_result(&mut self, token: AwaitToken, value: Value) -> Result<MoveStep, Error> {
        if self.pending.is_some() {
            return Err(Error::Runtime("async move is already pending".into()));
        }
        let mut state = self
            .plan
            .initial(&self.plan.move_identity, token)
            .map_err(|e| Error::Runtime(e.to_string()))?;
        let Value::List(values) = value else {
            return Err(Error::Value(
                "continuation pre-step must return a list".into(),
            ));
        };
        let (awaitable, local) = match values.as_slice() {
            [awaitable] => (awaitable.clone(), None),
            [awaitable, local] => (awaitable.clone(), Some(local.clone())),
            _ => {
                return Err(Error::Value(
                    "continuation pre-step returned wrong arity".into(),
                ));
            }
        };
        if let Some(value) = local {
            let scalar = scalar_from_value(value)?;
            let (slot, _) = self
                .plan
                .locals
                .first()
                .ok_or_else(|| Error::Value("unexpected scalar spill".into()))?;
            state.locals = vec![ScalarLocal {
                slot: *slot,
                value: scalar,
            }];
        }
        self.plan
            .validate(&state)
            .map_err(|e| Error::Runtime(e.to_string()))?;
        let pending = PendingMove {
            continuation: state,
        };
        self.pending = Some(pending.clone());
        Ok(MoveStep::Waiting { pending, awaitable })
    }

    pub fn resume(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
    ) -> Result<Option<MoveStep>, Error> {
        self.resume_with_args(owner, generation, event_kind, frame, &[])
    }

    pub fn resume_with_args(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        args: &[Value],
    ) -> Result<Option<MoveStep>, Error> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        if !self
            .plan
            .event_matches(&pending.continuation, owner, generation, event_kind, frame)
            .map_err(|e| Error::Runtime(e.to_string()))?
        {
            return Ok(None);
        }
        let state = pending.clone();
        let value = if state.continuation.locals.is_empty() {
            self.image.call_post_with_args(args, None)?
        } else {
            let scalar = value_from_scalar(&state.continuation.locals[0].value);
            self.image.call_post_with_args(args, Some(scalar))?
        };
        self.complete_result(state, value)
    }

    /// Consume a post-await result produced by an externally scoped native
    /// invoker. The checkpoint remains owned until this method receives a
    /// successful result and commits completion.
    pub fn resume_result(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        value: Value,
    ) -> Result<Option<MoveStep>, Error> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        if !self
            .plan
            .event_matches(&pending.continuation, owner, generation, event_kind, frame)
            .map_err(|e| Error::Runtime(e.to_string()))?
        {
            return Ok(None);
        }
        let state = pending.clone();
        self.complete_result(state, value)
    }

    /// Validate an event before invoking the caller-owned scoped native bridge.
    /// The copied checkpoint remains present if the bridge returns an error.
    pub fn resume_scoped<F>(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        invoke: F,
    ) -> Result<Option<MoveStep>, Error>
    where
        F: FnOnce(
            &mut NativeStep,
            crate::continuation::StepPhase,
            Option<Value>,
        ) -> Result<Value, Error>,
    {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        if !self
            .plan
            .event_matches(&pending.continuation, owner, generation, event_kind, frame)
            .map_err(|e| Error::Runtime(e.to_string()))?
        {
            return Ok(None);
        }
        let scalar = pending
            .continuation
            .locals
            .first()
            .map(|local| value_from_scalar(&local.value));
        let value = invoke(
            &mut self.image,
            crate::continuation::StepPhase::Post,
            scalar,
        )?;
        self.complete_result(pending.clone(), value)
    }

    /// Replace pending state from an authoritative rollback record without
    /// compiling, invoking, or rerunning the pre-await step.
    pub fn restore_pending(&mut self, pending: Option<PendingMove>) -> Result<(), Error> {
        if let Some(ref pending) = pending {
            self.plan
                .event_matches(
                    &pending.continuation,
                    pending.continuation.await_token.owner,
                    pending.continuation.await_token.generation,
                    &pending.continuation.await_token.event_kind,
                    pending.continuation.await_token.deadline_frame,
                )
                .map_err(|e| Error::Runtime(e.to_string()))?;
        }
        self.pending = pending;
        Ok(())
    }

    fn complete_result(
        &mut self,
        _state: PendingMove,
        value: Value,
    ) -> Result<Option<MoveStep>, Error> {
        self.pending.take();
        Ok(Some(MoveStep::Complete(value)))
    }

    pub fn cancel(&mut self) -> Result<bool, ContinuationError> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(false);
        };
        self.plan.cancel(&pending.continuation)?;
        self.pending.take();
        Ok(true)
    }

    pub fn checkpoint(&self) -> Option<&PendingMove> {
        self.pending.as_ref()
    }
}

fn scalar_from_value(value: Value) -> Result<Scalar, Error> {
    Ok(match value {
        Value::None => Scalar::None,
        Value::Int(v) => Scalar::Int(v),
        Value::F32(v) => Scalar::Float(v as f64),
        Value::Bool(v) => Scalar::Bool(v),
        _ => {
            return Err(Error::Value(
                "await continuation local is not scalar".into(),
            ));
        }
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
