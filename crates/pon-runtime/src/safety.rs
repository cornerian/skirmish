//! RAII guards for the host/runtime boundary.
//!
//! These guards keep the runtime attached while Rust owns raw `PyObject`
//! pointers, publish a live conservative stack boundary only for the duration
//! of a generated-code call, and expose the runtime's public scoped-root
//! registry for heap-backed argument buffers.

use std::{cell::UnsafeCell, marker::PhantomData, sync::MutexGuard};

use pon_runtime::{PyObject, ScopedRootSourceEntry, aot_entry::capture_stack_base};

thread_local! {
    static ATTACH_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    // Prepared handles on one host thread may park nested safe regions.  An
    // invocation suspends the single underlying Pon region while retaining
    // this logical count, so another idle handle cannot leave the active
    // thread marked safe.
    static IDLE_SAFE_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Keeps the current OS thread attached until the guard is dropped.
///
/// Attachment is reference-counted per thread because the upstream attach API
/// is idempotent while detach removes the thread from the live registry.
#[must_use]
pub struct Attachment {
    active: bool,
    _not_send: PhantomData<std::rc::Rc<()>>,
}

/// Keeps an attached thread visible to the collector while it is idle. A
/// null stack bound is intentional: all live host-owned Pon pointers must be
/// in explicit roots while this guard is active.
pub struct GcSafeRegion {
    active: bool,
    _not_send: PhantomData<std::rc::Rc<()>>,
}

impl GcSafeRegion {
    pub fn enter() -> Self {
        IDLE_SAFE_DEPTH.with(|depth| {
            if depth.get() == 0 {
                pon_runtime::thread_state_lock().enter_gc_safe_region(std::ptr::null_mut());
            }
            depth.set(depth.get().saturating_add(1));
        });
        Self {
            active: true,
            _not_send: PhantomData,
        }
    }

    /// Temporarily leaves the underlying safe region for an active call.
    /// The logical ownership count remains intact for all parked handles.
    pub fn suspend_all() -> SuspendedGcSafeRegions {
        let depth = IDLE_SAFE_DEPTH.with(|current| {
            let depth = current.get();
            if depth != 0 {
                current.set(0);
                // SAFETY: the logical idle region owns the underlying region
                // being left. The upstream helper also polls after leaving,
                // closing the race where a collector requests a stop between
                // publication of the active state and the next generated-code
                // safepoint.
                let _ = pon_runtime::sync::leave_blocking_region();
            }
            depth
        });
        SuspendedGcSafeRegions {
            depth,
            restored: false,
            _not_send: PhantomData,
        }
    }
}

impl Drop for GcSafeRegion {
    fn drop(&mut self) {
        if self.active {
            IDLE_SAFE_DEPTH.with(|depth| {
                let remaining = depth.get().saturating_sub(1);
                depth.set(remaining);
                if remaining == 0 {
                    // SAFETY: this is the matching leave for the underlying
                    // region entered when the logical depth reached one.
                    let _ = pon_runtime::sync::leave_blocking_region();
                }
            });
            self.active = false;
        }
    }
}

pub struct SuspendedGcSafeRegions {
    depth: usize,
    restored: bool,
    _not_send: PhantomData<std::rc::Rc<()>>,
}

impl SuspendedGcSafeRegions {
    pub fn restore(mut self) {
        self.restore_inner();
    }

    fn restore_inner(&mut self) {
        if self.restored || self.depth == 0 {
            self.restored = true;
            return;
        }
        IDLE_SAFE_DEPTH.with(|depth| {
            if depth.get() == 0 {
                pon_runtime::thread_state_lock().enter_gc_safe_region(std::ptr::null_mut());
            }
            depth.set(depth.get().saturating_add(self.depth));
        });
        self.restored = true;
    }
}

impl Drop for SuspendedGcSafeRegions {
    fn drop(&mut self) {
        self.restore_inner();
    }
}

impl Attachment {
    pub fn acquire() -> Result<Self, String> {
        let attached = ATTACH_DEPTH.with(|depth| {
            let count = depth.get();
            if count == 0 && unsafe { pon_runtime::pon_thread_attach() }.is_null() {
                return false;
            }
            depth.set(count + 1);
            true
        });
        if attached {
            Ok(Self {
                active: true,
                _not_send: PhantomData,
            })
        } else {
            Err(pon_runtime::pon_err_message().unwrap_or_else(|| "thread attach failed".into()))
        }
    }
}

impl Drop for Attachment {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        ATTACH_DEPTH.with(|depth| {
            let count = depth.get();
            if count <= 1 {
                depth.set(0);
                let _ = unsafe { pon_runtime::pon_thread_detach() };
            } else {
                depth.set(count - 1);
            }
        });
        self.active = false;
    }
}

/// Temporarily publishes a conservative stack boundary and restores the
/// previous boundary on every exit path.
#[must_use]
pub struct StackBoundary {
    previous: *mut u8,
    _not_send: PhantomData<std::rc::Rc<()>>,
}

impl StackBoundary {
    /// # Safety
    ///
    /// `marker` must remain addressable for the complete lifetime of the
    /// returned guard, usually as a local in the caller's enclosing frame.
    pub unsafe fn capture(marker: *mut u8) -> Self {
        let previous = pon_runtime::thread_state_lock().stack_base;
        capture_stack_base(marker);
        Self {
            previous,
            _not_send: PhantomData,
        }
    }
}

impl Drop for StackBoundary {
    fn drop(&mut self) {
        capture_stack_base(self.previous);
    }
}

unsafe fn rooted_vec_thunk(address: usize, push: &mut dyn FnMut(*mut PyObject)) {
    // SAFETY: the guard keeps the RootedVec alive and immovable while this
    // callback is registered; UnsafeCell permits shared GC inspection while
    // the host construction handle exists.
    let values = unsafe { &*(*(address as *const RootedVec)).values.get() };
    for &value in values {
        if !value.is_null() && pon_runtime::tag::is_heap(value) {
            push(value);
        }
    }
}

/// Heap-backed raw object pointers that may survive a nested runtime call.
#[derive(Default)]
pub struct RootedVec {
    values: UnsafeCell<Vec<*mut PyObject>>,
    _not_send: PhantomData<std::rc::Rc<()>>,
}

impl RootedVec {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers this buffer as a GC root source until the returned guard
    /// drops. The borrow prevents moving the buffer while registered.
    pub fn guard(&self) -> RootedVecGuard<'_> {
        let address = self as *const RootedVec as usize;
        pon_runtime::thread_state_lock()
            .scoped_root_sources
            .push(ScopedRootSourceEntry {
                addr: address,
                thunk: rooted_vec_thunk,
            });
        RootedVecGuard {
            owner: self,
            address,
        }
    }
}

/// Mutable view of a registered [`RootedVec`].
pub struct RootedVecGuard<'a> {
    owner: &'a RootedVec,
    address: usize,
}

impl RootedVecGuard<'_> {
    pub fn push(&mut self, value: *mut PyObject) {
        // SAFETY: this guard is the sole host-side constructor handle.
        unsafe { &mut *self.owner.values.get() }.push(value);
    }

    #[must_use]
    pub fn len(&self) -> usize {
        // SAFETY: owner remains alive and immovable for the guard lifetime.
        unsafe { (&*self.owner.values.get()).len() }
    }

    #[must_use]
    pub fn as_mut_ptr(&mut self) -> *mut *mut PyObject {
        // SAFETY: owner remains alive and immovable for the guard lifetime.
        unsafe { (&mut *self.owner.values.get()).as_mut_ptr() }
    }
}

impl Drop for RootedVecGuard<'_> {
    fn drop(&mut self) {
        let mut state: MutexGuard<'static, pon_runtime::PonThreadState> =
            pon_runtime::thread_state_lock();
        if let Some(index) = state
            .scoped_root_sources
            .iter()
            .rposition(|entry| entry.addr == self.address)
        {
            state.scoped_root_sources.remove(index);
        }
    }
}

/// Heap-backed roots whose lifetime spans a prepared program rather than one
/// host call. The owner is boxed so the registered root-source address stays
/// stable while the program is alive.
pub struct PersistentRoots {
    owner: Box<RootedVec>,
    address: usize,
}

impl PersistentRoots {
    pub fn new() -> Self {
        let owner = Box::new(RootedVec::new());
        let address = (&*owner as *const RootedVec) as usize;
        pon_runtime::thread_state_lock()
            .scoped_root_sources
            .push(ScopedRootSourceEntry {
                addr: address,
                thunk: rooted_vec_thunk,
            });
        Self { owner, address }
    }

    pub fn push(&mut self, value: *mut PyObject) {
        // SAFETY: this is the sole host-side owner of the persistent buffer.
        unsafe { &mut *self.owner.values.get() }.push(value);
    }
}

impl Default for PersistentRoots {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PersistentRoots {
    fn drop(&mut self) {
        let mut state = pon_runtime::thread_state_lock();
        if let Some(index) = state
            .scoped_root_sources
            .iter()
            .rposition(|entry| entry.addr == self.address)
        {
            state.scoped_root_sources.remove(index);
        }
    }
}
