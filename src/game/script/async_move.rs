//! Native bridge for the restricted asynchronous `Move.run` ABI.
//!
//! A prepared move owns the immutable lowered plan and disposable native
//! image on the gameplay thread.  The event state stores only
//! `pon_runtime::async_move::PendingMove`, so checkpoints never retain a Pon
//! frame, host scope, or JIT pointer.

use super::starlark::NativeValue;
use skirmish_script_runtime::{
    Value,
    continuation::{AwaitToken, Scalar},
    sequential_move::{MoveStep, PendingMove, SequentialMoveExecutor},
};
use std::{cell::RefCell, collections::HashMap};

thread_local! {
    static PREPARED_MOVES: RefCell<HashMap<(u64, usize), PreparedMove>> = RefCell::new(HashMap::new());
}

/// Prepare all async move images at the explicit thread boundary. Gameplay
/// lookup never calls this function and therefore never compiles.
pub fn prepare_program(program: &super::Program) -> Result<(), String> {
    let compiled = program
        .compiled()
        .ok_or_else(|| "program is not linked".to_owned())?;
    let key = compiled.identity();
    PREPARED_MOVES.with(|cache| {
        let mut cache = cache.borrow_mut();
        for (index, behavior) in program.metadata().behaviors.iter().enumerate() {
            if behavior.run.is_none() || cache.contains_key(&(key, index)) {
                continue;
            }
            cache.insert((key, index), PreparedMove::from_program(program, index)?);
        }
        Ok(())
    })
}

pub fn with_prepared_move<R>(
    program: &super::Program,
    behavior: usize,
    f: impl FnOnce(&mut PreparedMove) -> Result<R, String>,
) -> Result<R, String> {
    let compiled = program
        .compiled()
        .ok_or_else(|| "program is not linked".to_owned())?;
    PREPARED_MOVES.with(|cache| {
        let mut cache = cache.borrow_mut();
        let entry = cache
            .get_mut(&(compiled.identity(), behavior))
            .ok_or_else(|| format!("async move {behavior} was not prepared for this thread"))?;
        f(entry)
    })
}

/// Metadata needed to bind a class move to its lowered `run` method.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveRunMetadata {
    pub identity: String,
    pub function_name: String,
    pub module_name: String,
}

impl MoveRunMetadata {
    pub fn new(identity: impl Into<String>, function_name: impl Into<String>) -> Self {
        Self::with_module(identity, function_name, "__root__")
    }

    pub fn with_module(
        identity: impl Into<String>,
        function_name: impl Into<String>,
        module_name: impl Into<String>,
    ) -> Self {
        Self {
            identity: identity.into(),
            function_name: function_name.into(),
            module_name: module_name.into(),
        }
    }
}

/// A move prepared at resource load or at the explicit gameplay thread
/// boundary. Its image is intentionally omitted from rollback state.
pub struct PreparedMove {
    metadata: MoveRunMetadata,
    source: String,
    executor: PreparedExecutor,
    compiled: Option<std::sync::Arc<super::starlark::CompiledProgram>>,
    behavior_index: usize,
}

enum PreparedExecutor {
    Sequential(Box<SequentialMoveExecutor>),
}

impl std::fmt::Debug for PreparedMove {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedMove")
            .field("metadata", &self.metadata)
            .field("source_bytes", &self.source.len())
            .field("pending", &self.checkpoint())
            .finish()
    }
}

impl PreparedMove {
    /// Build the run adapter for one exported behavior. Resource loading has
    /// already validated the callback identity; this only lowers the method
    /// body and creates the thread-local image.
    pub fn from_program(program: &super::Program, behavior_index: usize) -> Result<Self, String> {
        let behavior = program
            .metadata()
            .behaviors
            .get(behavior_index)
            .ok_or_else(|| format!("unknown move behavior index {behavior_index}"))?;
        let function_name = behavior
            .run
            .clone()
            .ok_or_else(|| format!("move behavior {behavior_index} has no async run method"))?;
        let module_name = behavior
            .run_module
            .clone()
            .ok_or_else(|| format!("move behavior {behavior_index} has no async run module"))?;
        let compiled = program
            .compiled()
            .ok_or_else(|| "program is not linked".to_owned())?;
        let source = compiled.source_for_module(&module_name).ok_or_else(|| {
            format!("async run module {module_name:?} is not in the prepared source bundle")
        })?;
        let identity = behavior
            .id
            .clone()
            .ok_or_else(|| format!("move behavior {behavior_index} has no identity"))?;
        let mut prepared = Self::prepare(
            source.to_owned(),
            MoveRunMetadata::with_module(identity, function_name, module_name),
        )?;
        prepared.compiled = Some(std::sync::Arc::new(compiled.clone()));
        prepared.behavior_index = behavior_index;
        Ok(prepared)
    }

    pub fn prepare(source: impl Into<String>, metadata: MoveRunMetadata) -> Result<Self, String> {
        let source = source.into();
        let executor =
            SequentialMoveExecutor::new(&source, &metadata.function_name, &metadata.identity)
                .map(|executor| PreparedExecutor::Sequential(Box::new(executor)))
                .map_err(|error| error.to_string())?;
        Ok(Self {
            metadata,
            source,
            executor,
            compiled: None,
            behavior_index: 0,
        })
    }

    /// Run one phase with the retained SDK move instance and a fresh
    /// MoveContext created inside the active host scope.
    pub fn invoke_scoped(
        &mut self,
        phase: skirmish_script_runtime::continuation::StepPhase,
        fighter: super::starlark::HostRef,
        action: NativeValue,
        token: Option<AwaitToken>,
    ) -> Result<MoveStep, String> {
        let compiled = self
            .compiled
            .as_ref()
            .ok_or_else(|| "move is not linked to a prepared program".to_owned())?;
        match phase {
            skirmish_script_runtime::continuation::StepPhase::Pre => {
                let value = match &mut self.executor {
                    PreparedExecutor::Sequential(executor) => executor
                        .start_scoped(
                            token.ok_or_else(|| "pre step requires await token".to_owned())?,
                            |entry, args| {
                                compiled
                                    .invoke_native_entry_in_module(
                                        &self.metadata.module_name,
                                        entry,
                                        self.behavior_index,
                                        fighter,
                                        action,
                                        args,
                                    )
                                    .map_err(|error| {
                                        skirmish_script_runtime::PonError::Runtime(
                                            error.to_string(),
                                        )
                                    })
                                    .and_then(|value| {
                                        value_to_pon(value)
                                            .map_err(skirmish_script_runtime::PonError::Runtime)
                                    })
                            },
                        )
                        .map_err(|error| error.to_string())?,
                };
                Ok(value)
            }
            skirmish_script_runtime::continuation::StepPhase::Post => {
                Err("post phase requires resume_scoped".into())
            }
        }
    }

    pub fn resume_scoped(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        fighter: super::starlark::HostRef,
        action: NativeValue,
    ) -> Result<Option<MoveStep>, String> {
        let compiled = self
            .compiled
            .as_ref()
            .ok_or_else(|| "move is not linked to a prepared program".to_owned())?;
        match &mut self.executor {
            PreparedExecutor::Sequential(executor) => executor
                .resume_scoped(
                    owner,
                    generation,
                    event_kind,
                    frame,
                    Scalar::None,
                    |entry, args| {
                        compiled
                            .invoke_native_entry_in_module(
                                &self.metadata.module_name,
                                entry,
                                self.behavior_index,
                                fighter,
                                action,
                                args,
                            )
                            .map_err(|error| {
                                skirmish_script_runtime::PonError::Runtime(error.to_string())
                            })
                            .and_then(|value| {
                                value_to_pon(value)
                                    .map_err(skirmish_script_runtime::PonError::Runtime)
                            })
                    },
                )
                .map_err(|error| error.to_string()),
        }
    }

    pub fn metadata(&self) -> &MoveRunMetadata {
        &self.metadata
    }
    pub fn checkpoint(&self) -> Option<&PendingMove> {
        match &self.executor {
            PreparedExecutor::Sequential(executor) => executor.checkpoint(),
        }
    }

    pub fn start(&mut self, args: &[Value], token: AwaitToken) -> Result<MoveStep, String> {
        let _ = (args, token);
        Err("sequential move requires invoke_scoped".into())
    }

    /// Restore a disposable image from copied rollback state. This method is
    /// used after deserialization and never re-runs the pre-await body.
    pub fn restore(&mut self, pending: PendingMove) -> Result<(), String> {
        self.restore_pending(Some(pending))
    }

    pub fn restore_pending(&mut self, pending: Option<PendingMove>) -> Result<(), String> {
        match &mut self.executor {
            PreparedExecutor::Sequential(executor) => executor
                .restore_pending(pending)
                .map_err(|error| error.to_string()),
        }
    }

    pub fn resume(
        &mut self,
        owner: u64,
        generation: u64,
        event_kind: &str,
        frame: u64,
        args: &[Value],
    ) -> Result<Option<MoveStep>, String> {
        let _ = (owner, generation, event_kind, frame, args);
        Err("sequential move requires resume_scoped".into())
    }

    pub fn cancel(&mut self) -> Result<bool, String> {
        Ok(match &mut self.executor {
            PreparedExecutor::Sequential(executor) => executor.cancel(),
        })
    }
}

fn value_to_pon(value: NativeValue) -> Result<Value, String> {
    Ok(match value {
        NativeValue::None => Value::None,
        NativeValue::Bool(v) => Value::Bool(v),
        NativeValue::Int(v) => Value::Int(v),
        NativeValue::F32(v) => Value::F32(v),
        NativeValue::String(v) => Value::String(v),
        NativeValue::List(v) => {
            Value::List(v.into_iter().map(value_to_pon).collect::<Result<_, _>>()?)
        }
        NativeValue::Tuple(v) => {
            Value::Tuple(v.into_iter().map(value_to_pon).collect::<Result<_, _>>()?)
        }
        NativeValue::Dict(v) => Value::Dict(
            v.into_iter()
                .map(|(k, v)| Ok((k, value_to_pon(v)?)))
                .collect::<Result<_, String>>()?,
        ),
        NativeValue::Vec2(_) | NativeValue::Object(_) => {
            return Err("async move result contains unsupported native object".into());
        }
    })
}

/// Convert a native result into the game-facing scalar/list ABI. Kept small
/// because unsupported object results must remain an explicit restriction.
pub fn result_value(value: Value) -> Result<NativeValue, String> {
    match value {
        Value::None => Ok(NativeValue::None),
        Value::Bool(value) => Ok(NativeValue::Bool(value)),
        Value::Int(value) => Ok(NativeValue::Int(value)),
        Value::F32(value) => Ok(NativeValue::F32(value)),
        Value::List(values) => values
            .into_iter()
            .map(result_value)
            .collect::<Result<Vec<_>, _>>()
            .map(NativeValue::List),
        Value::Tuple(values) => values
            .into_iter()
            .map(result_value)
            .collect::<Result<Vec<_>, _>>()
            .map(NativeValue::Tuple),
        Value::Dict(values) => values
            .into_iter()
            .map(|(key, value)| Ok((key, result_value(value)?)))
            .collect::<Result<_, String>>()
            .map(NativeValue::Dict),
        Value::String(value) => Ok(NativeValue::String(value)),
    }
}
