//! Phase-D JIT tier-up driver.
//!
//! The runtime owns hotness counters and the function dispatch cell; the JIT
//! owns the concrete tier-1 code.  This module bridges the two with a
//! process-wide runtime hook that queues from `pon-runtime`, compiles a tier-1
//! body, installs the entry through `PyFunction::entry`, and keeps the
//! executable module alive for as long as the owning [`TierUpDriver`] lives.

use std::{
    collections::HashMap,
    ffi::c_void,
    ptr,
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, SyncSender, TrySendError},
    },
};

use cranelift_codegen::ir::AbiParam;
use cranelift_frontend::FunctionBuilderContext;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{FuncId, Linkage, Module as ClifModule, ModuleError, default_libcall_names};
use pon_codegen::{
    ModuleAnnotations, OptimizingPlan,
    baseline::{
        CodegenError, NameMap, compile_function as compile_baseline_function, compile_osr_function,
        osr_live_values,
    },
    helpers::declare_helpers,
    infer_module_types,
    isa::{OptLevel, make_isa},
    lowering_steps, optimizing, plan_function,
};
use pon_ir::ir::{BlockId, Function, Module as IrModule};
use pon_runtime::{
    abi::{
        HELPERS, TIER1_CALL_THRESHOLD, TIER1_LOOP_THRESHOLD, TierUpRootVisit, pon_tierup_set_hook,
        pon_tierup_set_root_hook,
    },
    feedback::{FeedbackVec, TypeTag},
    object::{
        PyFunction, PyObject, TIER_STATE_DISABLED, TIER_STATE_QUEUED, TIER_STATE_TIER0,
        TIER_STATE_TIER1, Tier1Code,
    },
};

/// Function-entry hotness threshold mirrored from the runtime probe.
pub const CALL_THRESHOLD: u32 = TIER1_CALL_THRESHOLD;
/// Loop-backedge hotness threshold mirrored from the runtime probe.
pub const LOOP_THRESHOLD: u32 = TIER1_LOOP_THRESHOLD;

const COMPILE_QUEUE_CAPACITY: usize = 64;
static ROUTES: LazyLock<Mutex<HashMap<usize, std::sync::Weak<DriverState>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
// This registry is deliberately separate from ROUTES.  A route is an
// address lookup (and may be replaced by a later engine); the live registry
// keeps a closing engine visible to the root hook until its reservations have
// drained.
static LIVE_STATES: LazyLock<Mutex<Vec<std::sync::Weak<DriverState>>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
static SERVICE: LazyLock<Arc<TierUpService>> = LazyLock::new(|| Arc::new(TierUpService::new()));
static SYNC_TIERUP: LazyLock<bool> = LazyLock::new(|| match std::env::var_os("PON_SYNC_TIERUP") {
    Some(value) => !value.as_os_str().is_empty(),
    None => false,
});
static DEBUG_TIER: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("PON_DEBUG").is_ok_and(|value| value.split(',').any(|flag| flag.trim() == "tier"))
});
static TIER_COMPILE_DISABLE_LOGGED: AtomicBool = AtomicBool::new(false);

/// Process-local owner for tier-up metadata and installed tier-1 modules.
pub struct TierUpDriver {
    state: Arc<DriverState>,
    service: Arc<TierUpService>,
    owned: bool,
}

struct DriverState {
    shared: Arc<Mutex<DriverShared>>,
    pins: Arc<Mutex<HashMap<usize, usize>>>,
    shutting_down: Arc<AtomicBool>,
    lifecycle: Mutex<Lifecycle>,
    closed: Condvar,
    inflight: AtomicUsize,
}

struct Lifecycle {
    closing: bool,
    inflight: usize,
}

struct TierUpService {
    compile_tx: SyncSender<WorkItem>,
}

struct WorkItem {
    state: Arc<DriverState>,
    request: CompileRequest,
    _pin: PinGuard,
    _reservation: Reservation,
    #[cfg(test)]
    panic: bool,
}

struct Reservation {
    state: Arc<DriverState>,
}

struct PinGuard {
    pins: Arc<Mutex<HashMap<usize, usize>>>,
    function: *mut PyFunction,
}

impl Drop for PinGuard {
    fn drop(&mut self) {
        unpin(&self.pins, self.function);
    }
}

// SAFETY: the pointer is the same pinned non-moving GC allocation carried by
// CompileRequest; only its address is retained and unpin is synchronized.
unsafe impl Send for PinGuard {}

impl Drop for Reservation {
    fn drop(&mut self) {
        self.state.finish_request();
    }
}

struct DriverShared {
    modules: Vec<RegisteredModule>,
    functions: Vec<RegisteredFunction>,
    installed: Vec<Box<Tier1Compilation>>,
}

impl DriverShared {
    fn new() -> Self {
        Self {
            modules: Vec::new(),
            functions: Vec::new(),
            installed: Vec::new(),
        }
    }
}

impl TierUpService {
    fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel::<WorkItem>(COMPILE_QUEUE_CAPACITY);
        std::thread::Builder::new()
            .name("pon-tierup".to_owned())
            .spawn(move || {
                while let Ok(work) = rx.recv() {
                    let state = Arc::clone(&work.state);
                    let function = work.request.function.as_ptr();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        #[cfg(test)]
                        if work.panic {
                            panic!("test worker fault");
                        }
                        process_request(&state.shared, &state, work.request);
                    }));
                    if result.is_err() {
                        // WorkItem's reservation and pin guards still run as
                        // it leaves this iteration; keep the process-wide
                        // worker available for subsequent engines.
                        retire_queued_to_tier0(function);
                    }
                }
            })
            .expect("failed to spawn pon tier-up compiler thread");
        Self { compile_tx: tx }
    }
}

impl DriverState {
    fn reserve(self: &Arc<Self>) -> Option<Reservation> {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("tier-up lifecycle mutex poisoned");
        if lifecycle.closing {
            return None;
        }
        lifecycle.inflight += 1;
        self.inflight.fetch_add(1, Ordering::AcqRel);
        Some(Reservation {
            state: Arc::clone(self),
        })
    }

    fn finish_request(&self) {
        let mut lifecycle = self
            .lifecycle
            .lock()
            .expect("tier-up lifecycle mutex poisoned");
        debug_assert!(lifecycle.inflight > 0);
        lifecycle.inflight -= 1;
        self.inflight.fetch_sub(1, Ordering::AcqRel);
        if lifecycle.inflight == 0 {
            self.closed.notify_all();
        }
    }

    fn wait_closed(&self) {
        let guard = self
            .lifecycle
            .lock()
            .expect("tier-up lifecycle mutex poisoned");
        let _guard = self
            .closed
            .wait_while(guard, |state| state.inflight != 0)
            .expect("tier-up lifecycle mutex poisoned");
    }

    fn is_closing(&self) -> bool {
        self.lifecycle
            .lock()
            .expect("tier-up lifecycle mutex poisoned")
            .closing
    }
}

#[derive(Clone)]
struct RegisteredFunction {
    tier0_entry: *const u8,
    module_index: usize,
    function_index: usize,
    feedback_len: usize,
}

// SAFETY: Registered tier-0 entry pointers are immutable executable addresses
// produced by Cranelift finalization and are used only for identity
// comparisons.
unsafe impl Send for RegisteredFunction {}

struct RegisteredModule {
    ir: Arc<IrModule>,
}

#[allow(
    dead_code,
    reason = "tier-1 executable modules and plans are retained for dispatch and later precise-root \
	          consumers"
)]
struct Tier1Compilation {
    module: JITModule,
    entry: *const u8,
    osr_entry: *const u8,
    osr_loop_header: Option<BlockId>,
    function_index: usize,
    feedback_len: usize,
    plan: OptimizingPlan,
    lowering_steps: Vec<pon_codegen::LoweringStep>,
    feedback: Vec<Option<(TypeTag, TypeTag)>>,
}

// SAFETY: A finalized `Tier1Compilation` is moved wholesale from the background
// compiler into the driver-retained installed list. Its executable code is
// immutable after `finalize_definitions`; the driver joins the compiler before
// dropping retained modules.
unsafe impl Send for Tier1Compilation {}

#[allow(
    dead_code,
    reason = "tier-up failures are intentionally swallowed by the runtime hook after resetting the \
	          tier state"
)]
#[derive(Debug)]
enum TierUpCompileError {
    Codegen(CodegenError),
    Module(ModuleError),
    MissingFunction { function_index: usize },
}

impl From<CodegenError> for TierUpCompileError {
    fn from(error: CodegenError) -> Self {
        Self::Codegen(error)
    }
}

impl From<ModuleError> for TierUpCompileError {
    fn from(error: ModuleError) -> Self {
        Self::Module(error)
    }
}

impl TierUpDriver {
    /// Build an empty driver backed by the process-wide compiler service.
    #[must_use]
    pub fn new() -> Self {
        let shared = Arc::new(Mutex::new(DriverShared::new()));
        let pins = Arc::new(Mutex::new(HashMap::new()));
        let shutting_down = Arc::new(AtomicBool::new(false));
        let state = Arc::new(DriverState {
            shared,
            pins,
            shutting_down,
            lifecycle: Mutex::new(Lifecycle {
                closing: false,
                inflight: 0,
            }),
            closed: Condvar::new(),
            inflight: AtomicUsize::new(0),
        });
        LIVE_STATES
            .lock()
            .expect("tier-up live-state mutex poisoned")
            .push(Arc::downgrade(&state));
        Self {
            state,
            service: Arc::clone(&SERVICE),
            owned: true,
        }
    }

    /// Record finalized tier-0 entrypoints for a just-compiled IR module, both
    /// in this driver's registry and in the process-wide inspection registry.
    pub fn register_module(
        &mut self,
        ir_module: &IrModule,
        func_ids: &[FuncId],
        module: &JITModule,
    ) {
        let ir = Arc::new(ir_module.clone());
        let mut inspect_entries = Vec::new();
        {
            let mut shared = self
                .state
                .shared
                .lock()
                .expect("tier-up registry mutex poisoned");
            let module_index = shared.modules.len();
            shared.modules.push(RegisteredModule {
                ir: Arc::clone(&ir),
            });

            shared
                .functions
                .extend(ir_module.functions.iter().enumerate().filter_map(
                    |(function_index, function)| {
                        let func_id = *func_ids.get(function_index)?;
                        let tier0_entry = module.get_finalized_function(func_id);
                        inspect_entries.push((function_index, tier0_entry));
                        Some(RegisteredFunction {
                            tier0_entry,
                            module_index,
                            function_index,
                            feedback_len: feedback_len(function),
                        })
                    },
                ));
            let mut routes = ROUTES.lock().expect("tier-up route mutex poisoned");
            for record in shared.functions.iter() {
                routes.insert(record.tier0_entry as usize, Arc::downgrade(&self.state));
            }
        }
        crate::inspect::register_functions(&ir, inspect_entries);
    }

    /// Compile and install a queued function on the current thread.
    ///
    /// Request preparation, pinning, state transitions, and retirement all flow
    /// through the same processor used by the background compiler.
    unsafe fn compile_and_install(&self, function: *mut PyFunction, reason: CompileReason) {
        let Some(_reservation) = self.state.reserve() else {
            retire_queued_to_tier0(function);
            return;
        };
        if self.state.shutting_down.load(Ordering::Acquire) {
            retire_queued_to_tier0(function);
            return;
        }
        let Some(request) = (unsafe { self.prepare_request(function, reason) }) else {
            return;
        };
        let _pin = PinGuard {
            pins: Arc::clone(&self.state.pins),
            function: request.function.as_ptr(),
        };
        process_request(&self.state.shared, &self.state, request);
    }

    unsafe fn enqueue(&self, function: *mut PyFunction, reason: CompileReason) {
        let Some(reservation) = self.state.reserve() else {
            retire_queued_to_tier0(function);
            return;
        };
        if self.state.shutting_down.load(Ordering::Acquire) {
            retire_queued_to_tier0(function);
            return;
        }
        let Some(request) = (unsafe { self.prepare_request(function, reason) }) else {
            return;
        };
        let work = WorkItem {
            state: Arc::clone(&self.state),
            _pin: PinGuard {
                pins: Arc::clone(&self.state.pins),
                function: request.function.as_ptr(),
            },
            request,
            _reservation: reservation,
            #[cfg(test)]
            panic: false,
        };
        if let Err(error) = self.service.compile_tx.try_send(work) {
            let work = match error {
                TrySendError::Full(work) | TrySendError::Disconnected(work) => work,
            };
            retire_queued_to_tier0(work.request.function.as_ptr());
        }
    }

    unsafe fn prepare_request(
        &self,
        function: *mut PyFunction,
        reason: CompileReason,
    ) -> Option<CompileRequest> {
        if function.is_null() {
            return None;
        }
        let function_ref = unsafe { &*function };
        if function_ref.tier_state.load(Ordering::Acquire) != TIER_STATE_QUEUED {
            return None;
        }
        let Some(ir_snapshot_id) = self.find_record_id(function_ref.code) else {
            disable_queued(function_ref);
            return None;
        };
        let feedback_len = {
            let shared = self
                .state
                .shared
                .lock()
                .expect("tier-up registry mutex poisoned");
            shared
                .functions
                .get(ir_snapshot_id.0 as usize)
                .map(|record| record.feedback_len)
                .unwrap_or(0)
        };
        unsafe { ensure_feedback(function_ref, feedback_len) };
        let feedback_snapshot = unsafe { feedback_snapshot(function_ref, feedback_len) };
        pin(&self.state.pins, function);
        Some(CompileRequest {
            // SAFETY: `pin` inserted the function into the GC-visible pin set.
            function: unsafe { SendPtr::new(function) },
            ir_snapshot_id,
            feedback_snapshot,
            reason,
        })
    }

    fn find_record_id(&self, tier0_entry: *const u8) -> Option<IrSnapshotId> {
        let shared = self
            .state
            .shared
            .lock()
            .expect("tier-up registry mutex poisoned");
        shared
            .functions
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, record)| {
                (record.tier0_entry == tier0_entry).then_some(IrSnapshotId(index as u32))
            })
    }
}

impl Default for TierUpDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TierUpDriver {
    fn drop(&mut self) {
        if !self.owned {
            return;
        }
        self.close();
    }
}

impl TierUpDriver {
    /// Stop accepting work and wait until queued/in-flight jobs have retired.
    /// This must run before the owning tier-0 executable image is dropped.
    pub fn close(&self) {
        {
            let mut lifecycle = self
                .state
                .lifecycle
                .lock()
                .expect("tier-up lifecycle mutex poisoned");
            if lifecycle.closing {
                drop(lifecycle);
                self.state.wait_closed();
                return;
            }
            lifecycle.closing = true;
            self.state.shutting_down.store(true, Ordering::Release);
        }
        let mut routes = ROUTES.lock().expect("tier-up route mutex poisoned");
        routes.retain(|_, owner| {
            owner
                .upgrade()
                .is_some_and(|state| !Arc::ptr_eq(&state, &self.state))
        });
        drop(routes);
        self.state.wait_closed();
        let mut live = LIVE_STATES
            .lock()
            .expect("tier-up live-state mutex poisoned");
        live.retain(|owner| {
            owner
                .upgrade()
                .is_some_and(|state| !Arc::ptr_eq(&state, &self.state))
        });
    }
}

/// Install this driver's runtime hook.
///
/// The hook resolves the owning state through the process-wide route registry;
/// the driver's address does not need to remain stable across engine moves.
pub fn register_runtime_hook(driver: &mut TierUpDriver) {
    let _ = driver;
    unsafe { pon_tierup_set_root_hook((tierup_pin_roots as *const ()).cast_mut()) };
    if std::env::var_os("PON_TIER0_ONLY").is_some() {
        unsafe { pon_tierup_set_hook(ptr::null_mut()) };
    } else {
        unsafe { pon_tierup_set_hook((tierup_hook as *const ()).cast_mut()) };
    }
}

/// Retained for engine teardown symmetry. The process-wide hook is state
/// routed, so closing the driver's state is sufficient and no global hook
/// pointer is cleared here.
pub fn unregister_runtime_hook(driver: &TierUpDriver) {
    let _ = driver;
}

unsafe extern "C" fn tierup_hook(function: *mut PyFunction, reason: u32) {
    if function.is_null() {
        return;
    }
    let state = ROUTES.lock().ok().and_then(|routes| {
        routes
            .get(&unsafe { (*function).code as usize })
            .and_then(std::sync::Weak::upgrade)
    });
    let Some(state) = state else {
        if !function.is_null() {
            retire_queued_to_tier0(function);
        }
        return;
    };

    let reason = decode_reason(reason);
    let driver = TierUpDriver {
        state: Arc::clone(&state),
        service: Arc::clone(&SERVICE),
        owned: false,
    };
    unsafe {
        if *SYNC_TIERUP {
            driver.compile_and_install(function, reason);
        } else {
            driver.enqueue(function, reason);
        }
    };
}

unsafe extern "C" fn tierup_pin_roots(visit: TierUpRootVisit, ctx: *mut c_void) {
    let states: Vec<_> = LIVE_STATES
        .lock()
        .ok()
        .map(|mut live| {
            let states = live.iter().filter_map(std::sync::Weak::upgrade).collect();
            live.retain(|owner| owner.upgrade().is_some());
            states
        })
        .unwrap_or_default();
    for state in states {
        let pins = state.pins.lock().expect("tier-up pin mutex poisoned");
        for address in pins.keys().copied() {
            unsafe { visit(address as *mut u8, ctx) };
        }
    }
}

fn decode_reason(reason: u32) -> CompileReason {
    if reason == 0 {
        CompileReason::Call
    } else {
        CompileReason::LoopBackEdge(BlockId(reason - 1))
    }
}

fn process_request(
    shared: &Arc<Mutex<DriverShared>>,
    state: &Arc<DriverState>,
    request: CompileRequest,
) {
    let function = request.function.as_ptr();
    if function.is_null() {
        return;
    }
    if state.is_closing() {
        retire_queued_to_tier0(function);
        return;
    }
    let function_ref = unsafe { &*function };
    if function_ref.tier_state.load(Ordering::Acquire) != TIER_STATE_QUEUED {
        return;
    }

    let Some((record, mut ir_module)) = clone_registered_ir(shared, request.ir_snapshot_id) else {
        disable_queued(function_ref);
        return;
    };
    infer_module_types(&mut ir_module, &ModuleAnnotations::default());
    let Some(ir_function) = ir_module.functions.get(record.function_index) else {
        disable_queued(function_ref);
        return;
    };
    let Some(plan) = plan_function(ir_function) else {
        disable_queued(function_ref);
        return;
    };
    let steps = lowering_steps(&plan);
    let compile_result = compile_tier1_module(
        &ir_module,
        record.function_index,
        record.feedback_len,
        request.feedback_snapshot,
        request.reason,
        plan,
        steps,
    );
    match compile_result {
        Ok(compilation) if compilation.entry != record.tier0_entry => {
            if !install_compilation_for_state(shared, function_ref, compilation, state) {
                // close may have won the lifecycle gate while compilation was
                // running.  Leave no queued function pointing at a retired
                // tier-0 image; the CAS also preserves a newer tier-1 owner.
                retire_queued_to_tier0(function);
            }
        }
        Ok(_) => {
            disable_queued(function_ref);
        }
        Err(error) => {
            log_compile_disable_once(record.function_index, request.reason, &error);
            disable_queued(function_ref);
        }
    }
}

fn clone_registered_ir(
    shared: &Arc<Mutex<DriverShared>>,
    snapshot: IrSnapshotId,
) -> Option<(RegisteredFunction, IrModule)> {
    let shared = shared.lock().expect("tier-up registry mutex poisoned");
    let record = shared.functions.get(snapshot.0 as usize)?.clone();
    // Deep clone: the tier-1 pipeline mutates its module copy (type inference).
    let module = shared.modules.get(record.module_index)?.ir.as_ref().clone();
    Some((record, module))
}

#[cfg(test)]
fn install_compilation(
    shared: &Arc<Mutex<DriverShared>>,
    function: &PyFunction,
    compilation: Tier1Compilation,
) -> bool {
    install_compilation_inner(shared, function, compilation, None)
}

fn install_compilation_for_state(
    shared: &Arc<Mutex<DriverShared>>,
    function: &PyFunction,
    compilation: Tier1Compilation,
    state: &Arc<DriverState>,
) -> bool {
    install_compilation_inner(shared, function, compilation, Some(state))
}

fn install_compilation_inner(
    shared: &Arc<Mutex<DriverShared>>,
    function: &PyFunction,
    compilation: Tier1Compilation,
    state: Option<&Arc<DriverState>>,
) -> bool {
    // Closing takes this same gate before it begins draining.  In particular,
    // no compilation may publish tier-1 code after close has returned.
    let lifecycle = state.map(|state| {
        state
            .lifecycle
            .lock()
            .expect("tier-up lifecycle mutex poisoned")
    });
    if lifecycle
        .as_ref()
        .is_some_and(|lifecycle| lifecycle.closing)
    {
        return false;
    }
    if function
        .tier_state
        .compare_exchange(
            TIER_STATE_QUEUED,
            TIER_STATE_TIER1,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_err()
    {
        return false;
    }

    let mut compilation = Box::new(compilation);
    let entry = compilation.entry;
    let osr_entry = compilation.osr_entry;
    let osr_loop_header = compilation.osr_loop_header;
    let handle = (&mut *compilation as *mut Tier1Compilation).cast::<c_void>();

    unsafe {
        *function.tier1.get() = Some(Tier1Code { entry, handle });
    }
    function.deopt_count.store(0, Ordering::Release);
    function.entry.store(entry.cast_mut(), Ordering::Release);
    if let Some(header) = osr_loop_header {
        function.osr_loop_header.store(header.0, Ordering::Relaxed);
        function
            .osr_entry
            .store(osr_entry.cast_mut(), Ordering::Release);
    } else {
        function.osr_entry.store(ptr::null_mut(), Ordering::Release);
    }

    shared
        .lock()
        .expect("tier-up registry mutex poisoned")
        .installed
        .push(compilation);
    true
}

fn compile_tier1_module(
    ir_module: &IrModule,
    function_index: usize,
    feedback_len: usize,
    feedback: Vec<Option<(TypeTag, TypeTag)>>,
    reason: CompileReason,
    plan: OptimizingPlan,
    lowering_steps: Vec<pon_codegen::LoweringStep>,
) -> Result<Tier1Compilation, TierUpCompileError> {
    let mut module = make_tier1_module();
    let helpers = declare_helpers(&mut module)?;
    let func_ids = declare_tier1_functions(&mut module, ir_module)?;
    let osr = prepare_osr_entry(&mut module, ir_module, function_index, reason)?;
    let names = NameMap::from_ir_module(ir_module);
    let entry_arg_counts = pon_codegen::baseline::entry_arg_counts(ir_module);
    let mut ctx = module.make_context();
    let mut fctx = FunctionBuilderContext::new();
    for (index, function) in ir_module.functions.iter().enumerate() {
        if index == function_index {
            optimizing::compile_function(
                &mut module,
                &helpers,
                &func_ids,
                &names,
                function,
                &plan,
                &mut ctx,
                &mut fctx,
            )?;
            crate::mark_noncolocated_calls(&mut ctx);
            crate::trace_relocs(&mut ctx, module.isa(), index);
        } else {
            compile_baseline_function(
                &mut module,
                &helpers,
                &func_ids,
                &ir_module.functions,
                &names,
                function,
                entry_arg_counts[index],
                &mut ctx,
                &mut fctx,
                // Tier-1 modules never register safepoint maps; keep their
                // baseline companions on the conservative scan path.
                false,
            )?;
            crate::mark_noncolocated_calls(&mut ctx);
            crate::trace_relocs(&mut ctx, module.isa(), index);
        }
        module.define_function(func_ids[index], &mut ctx)?;
    }
    if let Some((osr_id, header, live_values)) = &osr {
        let function = ir_module
            .functions
            .get(function_index)
            .ok_or(TierUpCompileError::MissingFunction { function_index })?;
        compile_osr_function(
            &mut module,
            &helpers,
            &func_ids,
            &ir_module.functions,
            &names,
            function,
            *header,
            live_values,
            &mut ctx,
            &mut fctx,
        )?;
        crate::mark_noncolocated_calls(&mut ctx);
        crate::trace_relocs(&mut ctx, module.isa(), function_index);
        module.define_function(*osr_id, &mut ctx)?;
    }
    module.finalize_definitions()?;
    let func_id = *func_ids
        .get(function_index)
        .ok_or(TierUpCompileError::MissingFunction { function_index })?;
    let entry = module.get_finalized_function(func_id);
    let (osr_entry, osr_loop_header) = osr
        .as_ref()
        .map(|(id, header, _)| (module.get_finalized_function(*id), Some(*header)))
        .unwrap_or((ptr::null(), None));

    Ok(Tier1Compilation {
        module,
        entry,
        osr_entry,
        osr_loop_header,
        function_index,
        feedback_len,
        plan,
        feedback,
        lowering_steps,
    })
}

fn make_tier1_module() -> JITModule {
    let isa = make_isa(OptLevel::Speed, false);
    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    for helper in HELPERS {
        builder.symbol(helper.symbol, helper.address.cast::<u8>());
    }
    builder.symbol(
        pon_runtime::abi::CURRENT_LINE_SYMBOL,
        pon_runtime::abi::current_line_cell_address(),
    );
    crate::register_threading_symbols(&mut builder);
    JITModule::new(builder)
}

pub(crate) fn declare_tier1_functions(
    module: &mut JITModule,
    ir_module: &IrModule,
) -> Result<Vec<FuncId>, ModuleError> {
    let mut sig = module.make_signature();
    let ptr_ty = module.target_config().pointer_type();
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.returns.push(AbiParam::new(ptr_ty));

    ir_module
        .functions
        .iter()
        .enumerate()
        .map(|(index, _function)| {
            module.declare_function(&format!("__pon_tier1_fn_{index}"), Linkage::Local, &sig)
        })
        .collect()
}

fn prepare_osr_entry(
    module: &mut JITModule,
    ir_module: &IrModule,
    function_index: usize,
    reason: CompileReason,
) -> Result<Option<(FuncId, BlockId, Vec<pon_ir::ir::Value>)>, ModuleError> {
    let CompileReason::LoopBackEdge(header) = reason else {
        return Ok(None);
    };
    let Some(function) = ir_module.functions.get(function_index) else {
        return Ok(None);
    };
    let live_values = osr_live_values(function, header);
    if function.n_locals.saturating_add(live_values.len()) > OSR_MAX_LIVE {
        return Ok(None);
    }
    let mut sig = module.make_signature();
    let ptr_ty = module.target_config().pointer_type();
    sig.params.push(AbiParam::new(ptr_ty));
    sig.returns.push(AbiParam::new(ptr_ty));
    let id = module.declare_function(
        &format!("__pon_tier1_osr_fn_{function_index}_block_{}", header.0),
        Linkage::Local,
        &sig,
    )?;
    Ok(Some((id, header, live_values)))
}

fn feedback_len(function: &Function) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| block.insts.iter())
        .filter_map(|inst| inst.feedback_slot)
        .map(|slot| slot.0 as usize + 1)
        .max()
        .unwrap_or(0)
}

unsafe fn feedback_snapshot(function: &PyFunction, len: usize) -> Vec<Option<(TypeTag, TypeTag)>> {
    if len == 0 {
        return Vec::new();
    }

    let feedback = unsafe { &*function.feedback.get() };
    let Some(feedback) = feedback.as_ref() else {
        return vec![None; len];
    };

    (0..len)
        .map(|index| feedback.get(index).and_then(|cell| cell.speculate()))
        .collect()
}

unsafe fn ensure_feedback(function: &PyFunction, len: usize) {
    if len == 0 {
        return;
    }

    let feedback = unsafe { &mut *function.feedback.get() };
    let needs_install = feedback
        .as_ref()
        .map_or(true, |existing| existing.len() < len);
    if needs_install {
        *feedback = Some(FeedbackVec::new(len));
    }
}

fn pin(pins: &Arc<Mutex<HashMap<usize, usize>>>, function: *mut PyFunction) {
    *pins
        .lock()
        .expect("tier-up pin mutex poisoned")
        .entry(function as usize)
        .or_insert(0) += 1;
}

fn unpin(pins: &Arc<Mutex<HashMap<usize, usize>>>, function: *mut PyFunction) {
    let mut pins = pins.lock().expect("tier-up pin mutex poisoned");
    let key = function as usize;
    if let Some(count) = pins.get_mut(&key) {
        *count -= 1;
        if *count == 0 {
            pins.remove(&key);
        }
    }
}

fn retire_queued_to_tier0(function: *mut PyFunction) {
    if function.is_null() {
        return;
    }
    let function = unsafe { &*function };
    if function
        .tier_state
        .compare_exchange(
            TIER_STATE_QUEUED,
            TIER_STATE_TIER0,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        function
            .entry
            .store(function.code.cast_mut(), Ordering::Release);
        function.osr_entry.store(ptr::null_mut(), Ordering::Release);
    }
}

fn disable_queued(function: &PyFunction) {
    if function
        .tier_state
        .compare_exchange(
            TIER_STATE_QUEUED,
            TIER_STATE_DISABLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .is_ok()
    {
        function
            .entry
            .store(function.code.cast_mut(), Ordering::Release);
        function.osr_entry.store(ptr::null_mut(), Ordering::Release);
    }
}

fn log_compile_disable_once(
    function_index: usize,
    reason: CompileReason,
    error: &TierUpCompileError,
) {
    if !*DEBUG_TIER {
        return;
    }
    if TIER_COMPILE_DISABLE_LOGGED.swap(true, Ordering::AcqRel) {
        return;
    }
    eprintln!(
        "pon tier-up: permanently disabled a hot function after tier-1 compile error \
		 (function_index={function_index}, reason={reason:?}): {error:?}"
    );
}

// ─── J0.5 pin: OSR + background compilation (implemented by O1) ─────────────
//
// Frozen interfaces for the background-compilation and OSR-entry contracts.
// See `plans/pon-pin-J05-osr-bg-compile.md` for the full design: queue
// lifecycle, install protocol, tier-state machine, OSR transfer layout, and
// deopt-thrash back-off.

/// Maximum number of boxed slots an [`OsrTransferBuffer`] can carry.
///
/// Functions whose OSR live set exceeds this are OSR-ineligible and tier up
/// through the function-entry path only.
pub const OSR_MAX_LIVE: usize = 16;

/// Deopt count within one tier-1 epoch that triggers a thrash reset (J0.5
/// back-off policy; see the design doc, §5).
pub const DEOPT_THRASH_THRESHOLD: u32 = 64;
/// Maximum exponent for the thrash re-try hotness backoff (`threshold <<
/// epoch`).
pub const DEOPT_BACKOFF_MAX_SHIFT: u32 = 6;
/// Tier epoch after which a thrashing function is pinned to tier-0 for good.
pub const DEOPT_PIN_EPOCH: u8 = 8;

/// A `*mut PyFunction` that may cross onto the background compiler thread.
///
/// `PyFunction` allocations live on the non-moving `pon-gc` heap, so the
/// address itself is stable for the allocation's whole lifetime.  Liveness is
/// NOT automatic: functions are collectible (`finalize_function` in
/// `pon-runtime/src/abi.rs` runs for unreachable functions), so the enqueue
/// protocol must pin the function in the driver's GC-visible pin set before a
/// `SendPtr` is constructed and unpin it only after the request retires.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub struct SendPtr(*mut PyFunction);

// SAFETY: The pointer is only dereferenced by the compiler thread while the
// pointee is pinned in the driver's GC pin set (enqueue pins, retire unpins),
// so the allocation cannot be swept, and the pon-gc heap is non-moving, so the
// address cannot change.  All cross-thread field accesses go through the
// pointee's atomics (`tier_state`, `entry`); the `tier1` UnsafeCell is written
// only after winning the QUEUED->TIER1 claim (design doc, §3).
unsafe impl Send for SendPtr {}

impl SendPtr {
    /// Wrap a function pointer for the compile queue.
    ///
    /// # Safety
    /// The caller must guarantee the pointee stays pinned (GC-reachable via the
    /// driver's pin set or another root) for the whole lifetime of this value.
    #[must_use]
    #[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
    pub unsafe fn new(function: *mut PyFunction) -> Self {
        Self(function)
    }

    /// The wrapped raw pointer.
    #[must_use]
    #[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
    pub fn as_ptr(self) -> *mut PyFunction {
        self.0
    }
}

/// Why a function was submitted for tier-1 compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub enum CompileReason {
    /// The function-entry hotness probe crossed [`CALL_THRESHOLD`].
    Call,
    /// A loop back-edge probe crossed [`LOOP_THRESHOLD`]; the payload names the
    /// loop-header block that identifies the OSR point.
    LoopBackEdge(BlockId),
}

/// Stable identity of a registered IR snapshot: an index into the driver's
/// append-only `functions` registry (which names its module and function).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub struct IrSnapshotId(pub u32);

/// One unit of tier-1 compilation work.
///
/// Everything the compiler needs is captured on the submitting thread: the
/// feedback snapshot is taken before compilation so the compiler never reads
/// the function's live `FeedbackVec` cells concurrently with mutator writes.
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub struct CompileRequest {
    /// The queued function; pinned by the driver until this request retires.
    pub function: SendPtr,
    /// Registered IR snapshot to compile (resolved at enqueue time).
    pub ir_snapshot_id: IrSnapshotId,
    /// Monomorphic speculation snapshot, indexed by feedback slot.
    pub feedback_snapshot: Vec<Option<(TypeTag, TypeTag)>>,
    /// What made the function hot.
    pub reason: CompileReason,
}

const _: () = {
    const fn assert_send<T: Send>() {}
    assert_send::<CompileRequest>();
};

/// Stack-allocated live-state carrier for an OSR-entry transfer (J0.5).
///
/// Layout contract (repr(C); on 64-bit: 8-byte header words, then pointers):
///
/// ```text
/// offset 0                : loop_header (u32)  — IR BlockId of the OSR point
/// offset 4                : live_count  (u32)  — occupied prefix of `slots`
/// offset 8 .. 8 + 16*ptr  : slots[OSR_MAX_LIVE] — boxed live values
/// ```
///
/// Slot order is the canonical OSR live set for `loop_header`: every function
/// local slot in ascending `LocalId` order (NULL for unbound locals), then SSA
/// values live-in at the loop header (backward liveness on the raw IR) in
/// ascending `Value` index order.  Tier-0 writes the buffer at the back-edge;
/// the OSR body's entry block reads it with the same ordering rule.
#[repr(C)]
#[derive(Debug)]
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub struct OsrTransferBuffer {
    /// IR `BlockId.0` of the loop header this transfer targets.
    pub loop_header: u32,
    /// Number of occupied leading `slots`.
    pub live_count: u32,
    /// Boxed live values, canonical order; entries past `live_count` are NULL.
    pub slots: [*mut PyObject; OSR_MAX_LIVE],
}

/// ABI of a tier-1 OSR entry: consumes a transfer buffer, runs the function to
/// completion from the loop header, and returns the function's return value
/// (NULL with the thread-state exception set on error), so tier-0 forwards the
/// result unchanged.
#[allow(dead_code, reason = "J0.5 pin: consumed by O1 in a later wave")]
pub type OsrEntryFn = unsafe extern "C" fn(buffer: *mut OsrTransferBuffer) -> *mut PyObject;

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc, Mutex, OnceLock, Weak,
            atomic::{AtomicUsize, Ordering},
        },
        thread,
        time::{Duration, Instant},
    };

    use pon_codegen::{ModuleAnnotations, infer_module_types, plan_function};
    use pon_ir::{
        Type,
        ir::{
            BinOp, Block, BlockId, Function, FunctionId, Inst, InstKind, LocalId,
            Module as IrModule, NameId, PyConst, Terminator, Value,
        },
    };

    use super::*;

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn close_waits_for_reservation_and_rejects_later_admission() {
        let _serial = test_lock();
        let driver = TierUpDriver::new();
        let reservation = driver.state.reserve().expect("admission should succeed");
        let state = Arc::clone(&driver.state);
        let join = thread::spawn(move || state.wait_closed());
        thread::sleep(Duration::from_millis(10));
        assert!(!join.is_finished());
        drop(reservation);
        join.join().unwrap();
        let mut function = PyFunction::new(std::ptr::null(), 1_usize as *const u8, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        driver.close();
        unsafe { driver.enqueue(&mut function, CompileReason::Call) };
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER0
        );
    }

    struct RootCount {
        target: usize,
        count: AtomicUsize,
    }
    unsafe extern "C" fn count_root(object: *mut u8, context: *mut c_void) {
        let roots = unsafe { &*context.cast::<RootCount>() };
        if object as usize == roots.target {
            roots.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn root_hook_visits_pins_while_closing_reservation_drains() {
        let _serial = test_lock();
        let driver = TierUpDriver::new();
        let mut function = PyFunction::new(std::ptr::null(), 1_usize as *const u8, 0, 0);
        pin(&driver.state.pins, &mut function);
        let reservation = driver.state.reserve().unwrap();
        let state = Arc::clone(&driver.state);
        let join = thread::spawn(move || {
            state.lifecycle.lock().unwrap().closing = true;
            state.wait_closed();
        });
        thread::sleep(Duration::from_millis(10));
        let count = RootCount {
            target: &mut function as *mut _ as usize,
            count: AtomicUsize::new(0),
        };
        unsafe {
            tierup_pin_roots(count_root, &count as *const _ as *mut c_void);
        }
        assert_eq!(count.count.load(Ordering::Relaxed), 1);
        drop(reservation);
        join.join().unwrap();
        unpin(&driver.state.pins, &mut function);
    }

    #[test]
    fn queue_failure_cleanup_retires_and_releases_both_guards() {
        let _serial = test_lock();
        for disconnected in [false, true] {
            let driver = TierUpDriver::new();
            let mut function = PyFunction::new(std::ptr::null(), 1_usize as *const u8, 0, 0);
            function
                .tier_state
                .store(TIER_STATE_QUEUED, Ordering::Release);
            pin(&driver.state.pins, &mut function);
            let reservation = driver.state.reserve().unwrap();
            let request = CompileRequest {
                function: unsafe { SendPtr::new(&mut function) },
                ir_snapshot_id: IrSnapshotId(0),
                feedback_snapshot: Vec::new(),
                reason: CompileReason::Call,
            };
            let (tx, rx) = mpsc::sync_channel(0);
            let work = WorkItem {
                state: Arc::clone(&driver.state),
                request,
                _reservation: reservation,
                _pin: PinGuard {
                    pins: Arc::clone(&driver.state.pins),
                    function: &mut function,
                },
                #[cfg(test)]
                panic: false,
            };
            if disconnected {
                drop(rx);
            }
            let error = tx.try_send(work).unwrap_err();
            let work = match error {
                TrySendError::Full(work) | TrySendError::Disconnected(work) => work,
            };
            retire_queued_to_tier0(work.request.function.as_ptr());
            drop(work);
            assert_eq!(
                function.tier_state.load(Ordering::Acquire),
                TIER_STATE_TIER0
            );
            assert!(driver.state.pins.lock().unwrap().is_empty());
            assert_eq!(driver.state.inflight.load(Ordering::Acquire), 0);
        }
    }

    #[test]
    fn route_replacement_close_is_conditional_and_service_is_shared() {
        let _serial = test_lock();
        let first = TierUpDriver::new();
        let second = TierUpDriver::new();
        assert!(Arc::ptr_eq(&first.service, &second.service));
        let address = 0x12345usize;
        ROUTES
            .lock()
            .unwrap()
            .insert(address, Arc::downgrade(&first.state));
        ROUTES
            .lock()
            .unwrap()
            .insert(address, Arc::downgrade(&second.state));
        first.close();
        assert!(
            ROUTES
                .lock()
                .unwrap()
                .get(&address)
                .and_then(Weak::upgrade)
                .is_some_and(|s| Arc::ptr_eq(&s, &second.state))
        );
    }

    #[test]
    fn many_drivers_use_one_bounded_worker_service() {
        let _serial = test_lock();
        let drivers: Vec<_> = (0..128).map(|_| TierUpDriver::new()).collect();
        let service = Arc::clone(&drivers[0].service);
        assert!(
            drivers
                .iter()
                .all(|driver| Arc::ptr_eq(&driver.service, &service))
        );
        assert_eq!(COMPILE_QUEUE_CAPACITY, 64);
    }

    #[test]
    fn state_aware_install_rejects_late_compilation() {
        let _serial = test_lock();
        let driver = TierUpDriver::new();
        let tier0 = 0x1111usize as *const u8;
        let tier1 = 0x2222usize as *const u8;
        let mut inferred = arithmetic_range_loop_module();
        infer_module_types(&mut inferred, &ModuleAnnotations::default());
        let plan = plan_function(&inferred.functions[0]).unwrap();
        let compilation = Tier1Compilation {
            module: make_tier1_module(),
            entry: tier1,
            osr_entry: ptr::null(),
            osr_loop_header: None,
            function_index: 0,
            feedback_len: 0,
            plan,
            lowering_steps: Vec::new(),
            feedback: Vec::new(),
        };
        let function = PyFunction::new(ptr::null(), tier0, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        function.entry.store(tier0.cast_mut(), Ordering::Release);
        driver.state.lifecycle.lock().unwrap().closing = true;
        assert!(!install_compilation_for_state(
            &driver.state.shared,
            &function,
            compilation,
            &driver.state
        ));
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_QUEUED
        );
        assert_eq!(function.entry.load(Ordering::Acquire).cast_const(), tier0);
    }

    #[test]
    fn worker_panic_cleanup_retires_queued_job_and_releases_guards() {
        let _serial = test_lock();
        let driver = TierUpDriver::new();
        let mut function = PyFunction::new(ptr::null(), 0x3333usize as *const u8, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        pin(&driver.state.pins, &mut function);
        let reservation = driver.state.reserve().unwrap();
        let request = CompileRequest {
            function: unsafe { SendPtr::new(&mut function) },
            ir_snapshot_id: IrSnapshotId(0),
            feedback_snapshot: Vec::new(),
            reason: CompileReason::Call,
        };
        let work = WorkItem {
            state: Arc::clone(&driver.state),
            request,
            _pin: PinGuard {
                pins: Arc::clone(&driver.state.pins),
                function: &mut function,
            },
            #[cfg(test)]
            panic: false,
            _reservation: reservation,
        };
        let function_ptr = work.request.function.as_ptr();
        let work = WorkItem {
            panic: true,
            ..work
        };
        SERVICE.compile_tx.send(work).unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while driver.state.inflight.load(Ordering::Acquire) != 0 && Instant::now() < deadline {
            thread::yield_now();
        }
        assert_eq!(driver.state.inflight.load(Ordering::Acquire), 0);
        retire_queued_to_tier0(function_ptr);
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER0
        );
        assert!(driver.state.pins.lock().unwrap().is_empty());
        assert_eq!(driver.state.inflight.load(Ordering::Acquire), 0);
        {
            let mut shared = driver.state.shared.lock().unwrap();
            shared.modules.push(RegisteredModule {
                ir: Arc::new(arithmetic_range_loop_module()),
            });
            shared.functions.push(RegisteredFunction {
                tier0_entry: 0x4444usize as *const u8,
                module_index: 0,
                function_index: 0,
                feedback_len: 0,
            });
        }
        let mut next = PyFunction::new(ptr::null(), 0x4444usize as *const u8, 0, 0);
        next.tier_state.store(TIER_STATE_QUEUED, Ordering::Release);
        unsafe {
            driver.enqueue(&mut next, CompileReason::Call);
        }
        let deadline = Instant::now() + Duration::from_secs(2);
        while next.tier_state.load(Ordering::Acquire) == TIER_STATE_QUEUED
            && Instant::now() < deadline
        {
            thread::yield_now();
        }
        assert_ne!(next.tier_state.load(Ordering::Acquire), TIER_STATE_QUEUED);
    }

    #[test]
    fn tier_up_infers_cloned_ir_before_planning_range_loop() {
        let ir_module = arithmetic_range_loop_module();
        assert!(
            plan_function(&ir_module.functions[0]).is_none(),
            "raw tier-0 IR must not already carry enough metadata for an optimizing plan"
        );

        let mut inferred_module = ir_module.clone();
        infer_module_types(&mut inferred_module, &ModuleAnnotations::default());
        let inferred_plan = plan_function(&inferred_module.functions[0])
            .expect("fixture should plan once the existing inference pass seeds metadata");
        assert!(
            inferred_plan
                .fast_path
                .values
                .iter()
                .any(|value| value.value == Value(7) && value.ty == Type::IntI64),
            "inference should make the arithmetic body an IntI64 fast-path candidate: \
			 {inferred_plan:?}"
        );

        let tier0_entry = 1_usize as *const u8;
        let driver = TierUpDriver::new();
        {
            let mut shared = driver.state.shared.lock().expect("test tier-up mutex");
            shared.modules.push(RegisteredModule {
                ir: Arc::new(ir_module),
            });
            shared.functions.push(RegisteredFunction {
                tier0_entry,
                module_index: 0,
                function_index: 0,
                feedback_len: 0,
            });
        }

        let mut function = PyFunction::new(std::ptr::null(), tier0_entry, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        function.hotness.store(CALL_THRESHOLD, Ordering::Release);

        unsafe { driver.compile_and_install(&mut function, CompileReason::Call) };

        assert_eq!(
            inferred_type(
                &driver
                    .state
                    .shared
                    .lock()
                    .expect("test tier-up mutex")
                    .modules[0]
                    .ir,
                Value(7)
            ),
            Type::Bottom,
            "tier-up should infer a cloned module and leave the registered tier-0 IR raw"
        );
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER1,
            "tier-up should install tier-1 code when clone-local inference enables planning"
        );
        assert_ne!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_DISABLED,
            "successful tier-up must not leave the function permanently disabled"
        );
        let shared = driver.state.shared.lock().expect("test tier-up mutex");
        let compilation = shared
            .installed
            .first()
            .expect("typed clone should produce an optimizing tier-1 compilation");
        assert!(
            compilation
                .plan
                .fast_path
                .values
                .iter()
                .any(|value| value.value == Value(7) && value.ty == Type::IntI64),
            "installed plan should come from inferred arithmetic metadata: {:?}",
            compilation.plan
        );
    }

    #[test]
    fn synchronous_compile_preserves_loop_backedge_reason_for_osr() {
        let tier0_entry = 1_usize as *const u8;
        let driver = TierUpDriver::new();
        {
            let mut shared = driver.state.shared.lock().expect("test tier-up mutex");
            shared.modules.push(RegisteredModule {
                ir: Arc::new(arithmetic_range_loop_module()),
            });
            shared.functions.push(RegisteredFunction {
                tier0_entry,
                module_index: 0,
                function_index: 0,
                feedback_len: 0,
            });
        }

        let mut function = PyFunction::new(std::ptr::null(), tier0_entry, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);

        unsafe {
            driver.compile_and_install(&mut function, CompileReason::LoopBackEdge(BlockId(1)));
        };

        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER1
        );
        assert_eq!(
            function.osr_loop_header.load(Ordering::Relaxed),
            1,
            "synchronous tier-up should retain the decoded loop-backedge header"
        );
        assert!(
            !function.osr_entry.load(Ordering::Acquire).is_null(),
            "loop-backedge tier-up should publish an OSR entry"
        );
    }

    #[test]
    fn failed_tier_up_disables_future_queue_attempts() {
        let tier0_entry = 1_usize as *const u8;
        let unknown_entry = 2_usize as *const u8;
        let driver = TierUpDriver::new();
        {
            let mut shared = driver.state.shared.lock().expect("test tier-up mutex");
            shared.modules.push(RegisteredModule {
                ir: Arc::new(arithmetic_range_loop_module()),
            });
            shared.functions.push(RegisteredFunction {
                tier0_entry,
                module_index: 0,
                function_index: 0,
                feedback_len: 0,
            });
        }

        let mut function = PyFunction::new(std::ptr::null(), unknown_entry, 0, 0);
        function
            .entry
            .store(unknown_entry.cast_mut(), Ordering::Release);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);

        unsafe { driver.compile_and_install(&mut function, CompileReason::Call) };

        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_DISABLED,
            "functions that cannot be mapped to tier-1 metadata should not be re-queued on every hot \
			 probe"
        );
        assert_eq!(
            function.entry.load(Ordering::Acquire).cast_const(),
            unknown_entry,
            "failed tier-up keeps dispatching through the tier-0 entry"
        );
    }

    #[test]
    fn background_queue_installs_and_unpins_function() {
        let tier0_entry = 1_usize as *const u8;
        let driver = TierUpDriver::new();
        {
            let mut shared = driver.state.shared.lock().expect("test tier-up mutex");
            shared.modules.push(RegisteredModule {
                ir: Arc::new(arithmetic_range_loop_module()),
            });
            shared.functions.push(RegisteredFunction {
                tier0_entry,
                module_index: 0,
                function_index: 0,
                feedback_len: 0,
            });
        }

        let mut function = PyFunction::new(std::ptr::null(), tier0_entry, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        unsafe { driver.enqueue(&mut function, CompileReason::Call) };

        let deadline = Instant::now() + Duration::from_secs(5);
        while function.tier_state.load(Ordering::Acquire) == TIER_STATE_QUEUED
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER1
        );
        assert!(
            driver.state.pins.lock().expect("test pin mutex").is_empty(),
            "completed background request must unpin the function"
        );
        assert_eq!(
            driver
                .state
                .shared
                .lock()
                .expect("test tier-up mutex")
                .installed
                .len(),
            1
        );
    }

    #[test]
    fn install_publishes_osr_entry_when_compilation_has_one() {
        let tier0_entry = 1_usize as *const u8;
        let tier1_entry = 2_usize as *const u8;
        let osr_entry = 3_usize as *const u8;
        let driver = TierUpDriver::new();
        let mut inferred_module = arithmetic_range_loop_module();
        infer_module_types(&mut inferred_module, &ModuleAnnotations::default());
        let plan = plan_function(&inferred_module.functions[0]).expect("fixture should plan");
        let compilation = Tier1Compilation {
            module: make_tier1_module(),
            entry: tier1_entry,
            osr_entry,
            osr_loop_header: Some(BlockId(1)),
            function_index: 0,
            feedback_len: 0,
            plan,
            lowering_steps: Vec::new(),
            feedback: Vec::new(),
        };
        let function = PyFunction::new(std::ptr::null(), tier0_entry, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);

        assert!(install_compilation(
            &driver.state.shared,
            &function,
            compilation
        ));
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER1
        );
        assert_eq!(
            function.entry.load(Ordering::Acquire).cast_const(),
            tier1_entry
        );
        assert_eq!(function.osr_loop_header.load(Ordering::Relaxed), 1);
        assert_eq!(
            function.osr_entry.load(Ordering::Acquire).cast_const(),
            osr_entry
        );
    }

    #[test]
    fn deopt_note_defers_then_eventually_disables_thrashing_function() {
        let tier0_entry = 1_usize as *const u8;
        let tier1_entry = 2_usize as *mut u8;
        let mut function = PyFunction::new(std::ptr::null(), tier0_entry, 0, 0);
        function.entry.store(tier1_entry, Ordering::Release);
        function
            .tier_state
            .store(TIER_STATE_TIER1, Ordering::Release);
        function
            .deopt_count
            .store(DEOPT_THRASH_THRESHOLD - 1, Ordering::Release);
        function.hotness.store(CALL_THRESHOLD, Ordering::Release);
        function
            .loop_hotness
            .store(LOOP_THRESHOLD, Ordering::Release);

        unsafe {
            pon_runtime::abi::pon_deopt_note((&mut function as *mut PyFunction).cast::<PyObject>())
        };
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            pon_runtime::object::TIER_STATE_DEFERRED
        );
        assert_eq!(
            function.entry.load(Ordering::Acquire).cast_const(),
            tier0_entry
        );
        assert_eq!(function.tier_epoch.load(Ordering::Acquire), 1);
        assert_eq!(function.deopt_count.load(Ordering::Acquire), 0);
        assert_eq!(function.hotness.load(Ordering::Acquire), 0);
        assert_eq!(function.loop_hotness.load(Ordering::Acquire), 0);

        function.entry.store(tier1_entry, Ordering::Release);
        function
            .tier_state
            .store(TIER_STATE_TIER1, Ordering::Release);
        function
            .tier_epoch
            .store(DEOPT_PIN_EPOCH, Ordering::Release);
        function
            .deopt_count
            .store(DEOPT_THRASH_THRESHOLD - 1, Ordering::Release);

        unsafe {
            pon_runtime::abi::pon_deopt_note((&mut function as *mut PyFunction).cast::<PyObject>())
        };
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_DISABLED
        );
        assert_eq!(
            function.entry.load(Ordering::Acquire).cast_const(),
            tier0_entry
        );
    }

    #[test]
    fn drivers_share_one_bounded_service_and_pin_reservations_are_counted() {
        let first = TierUpDriver::new();
        let second = TierUpDriver::new();
        assert!(Arc::ptr_eq(&first.service, &second.service));
        let mut function = PyFunction::new(std::ptr::null(), 1_usize as *const u8, 0, 0);
        pin(&first.state.pins, &mut function);
        pin(&first.state.pins, &mut function);
        assert_eq!(
            first
                .state
                .pins
                .lock()
                .unwrap()
                .get(&(std::ptr::addr_of_mut!(function) as usize)),
            Some(&2)
        );
        unpin(&first.state.pins, &mut function);
        assert_eq!(first.state.pins.lock().unwrap().len(), 1);
        unpin(&first.state.pins, &mut function);
        assert!(first.state.pins.lock().unwrap().is_empty());
    }

    #[test]
    fn closing_driver_retires_queued_work_without_late_install() {
        let driver = TierUpDriver::new();
        let mut function = PyFunction::new(std::ptr::null(), 1_usize as *const u8, 0, 0);
        function
            .tier_state
            .store(TIER_STATE_QUEUED, Ordering::Release);
        driver.state.shutting_down.store(true, Ordering::Release);
        unsafe { driver.enqueue(&mut function, CompileReason::Call) };
        assert_eq!(
            function.tier_state.load(Ordering::Acquire),
            TIER_STATE_TIER0
        );
        assert_eq!(driver.state.inflight.load(Ordering::Acquire), 0);
    }

    fn arithmetic_range_loop_module() -> IrModule {
        IrModule {
            functions: vec![Function {
                name: "sum_range".to_owned(),
                arity: 0,
                params: Default::default(),
                is_coroutine: false,
                is_generator: false,
                is_async_generator: false,
                n_locals: 1,
                blocks: vec![
                    Block {
                        id: BlockId(0),
                        insts: vec![
                            Inst::new(Value(0), InstKind::LoadBuiltin(NameId(0))),
                            Inst::new(Value(1), InstKind::Const(PyConst::Int(0))),
                            Inst::new(Value(2), InstKind::Const(PyConst::Int(32))),
                            Inst::new(
                                Value(3),
                                InstKind::Call {
                                    callee: Value(0),
                                    args: vec![Value(1), Value(2)],
                                },
                            ),
                            Inst::new(Value(4), InstKind::GetIter { iterable: Value(3) }),
                        ],
                        term: Terminator::Jump(BlockId(1)),
                    },
                    Block {
                        id: BlockId(1),
                        insts: vec![Inst::new(Value(5), InstKind::ForNext { iter: Value(4) })],
                        term: Terminator::ForLoop {
                            iter: Value(4),
                            body: BlockId(2),
                            done: BlockId(3),
                        },
                    },
                    Block {
                        id: BlockId(2),
                        insts: vec![
                            Inst::new(Value(6), InstKind::Const(PyConst::Int(1))),
                            Inst::new(
                                Value(7),
                                InstKind::BinaryOp {
                                    op: BinOp::Add,
                                    lhs: Value(5),
                                    rhs: Value(6),
                                },
                            ),
                            Inst::new(Value(8), InstKind::StoreLocal(LocalId(0), Value(7))),
                        ],
                        term: Terminator::Jump(BlockId(1)),
                    },
                    Block {
                        id: BlockId(3),
                        insts: vec![Inst::new(Value(9), InstKind::LoadLocal(LocalId(0)))],
                        term: Terminator::Return(Value(9)),
                    },
                ],
            }],
            main: FunctionId(0),
            names: vec!["range".to_owned()],
        }
    }

    fn inferred_type(module: &IrModule, value: Value) -> Type {
        module
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.insts.iter())
            .find_map(|inst| (inst.result == value).then_some(inst.inferred_type))
            .expect("fixture value should exist")
    }
}
