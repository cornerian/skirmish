//! Opt-in gameplay immutability checks.
//!
//! This is deliberately a small runtime seam rather than a property of all
//! Pon objects.  A host enters a scope with the exact object identities which
//! form its frozen program graph.  Every central mutator asks this module
//! before changing a container, namespace, heap instance, or closure cell.
//!
//! The object list is an explicit graph contract: callers must enumerate all
//! reachable mutable objects (including aliases) and must keep those objects
//! GC-rooted for the scope's lifetime.  Unknown object layouts are therefore
//! rejected by the host's graph enumerator instead of silently being treated
//! as immutable.  The scope owns only identity tokens; it never owns or frees
//! runtime objects, so dropping a scope cannot leak them or leave stale
//! addresses active for a later allocation.

use std::{cell::RefCell, collections::HashSet, rc::Rc, sync::Arc};

use crate::object::PyObject;

thread_local! {
    static ACTIVE: RefCell<Vec<Rc<ActiveEntry>>> = const { RefCell::new(Vec::new()) };
}

/// Reusable immutable identity registry prepared once from a complete rooted
/// program graph.  Construction is the only operation that hashes the graph;
/// activating this policy for another callback is O(1).
#[derive(Clone, Debug)]
pub struct PreparedPolicy {
    identities: Arc<HashSet<usize>>,
}

impl PreparedPolicy {
    /// Prepares an identity registry from the host-enumerated graph.
    pub fn from_objects(objects: &[*mut PyObject]) -> Self {
        Self {
            identities: Arc::new(
                objects
                    .iter()
                    .filter_map(|object| {
                        let address = *object as usize;
                        (address != 0 && crate::tag::is_heap(*object)).then_some(address)
                    })
                    .collect(),
            ),
        }
    }
}

struct ActiveEntry {
    identities: Arc<HashSet<usize>>,
}

/// RAII activation of an explicit frozen object identity set.
#[must_use]
pub struct FrozenScope {
    entry: Option<Rc<ActiveEntry>>,
}

impl FrozenScope {
    /// Activates a prepared identity registry for the current thread.
    ///
    /// NULL and tagged immediate values are ignored.  The host must retain GC
    /// roots for the corresponding heap objects until this value is dropped.
    pub fn enter(policy: &PreparedPolicy) -> Self {
        let entry = Rc::new(ActiveEntry {
            identities: Arc::clone(&policy.identities),
        });
        ACTIVE.with(|active| active.borrow_mut().push(Rc::clone(&entry)));
        Self { entry: Some(entry) }
    }
}

impl Drop for FrozenScope {
    fn drop(&mut self) {
        if let Some(entry) = self.entry.take() {
            ACTIVE.with(|active| {
                let mut active = active.borrow_mut();
                if let Some(index) = active
                    .iter()
                    .position(|candidate| Rc::ptr_eq(candidate, &entry))
                {
                    active.remove(index);
                }
            });
        }
    }
}

/// Returns an error when a protected object is about to be mutated.
pub(crate) fn check(object: *mut PyObject) -> Result<(), String> {
    check_address(object.cast())
}

/// Same identity check for runtime-owned payloads without a `PyObject` header,
/// such as closure cells.
pub(crate) fn check_address(object: *const ()) -> Result<(), String> {
    let address = object as usize;
    if address == 0 {
        return Ok(());
    }
    let protected = ACTIVE.with(|active| {
        active
            .borrow()
            .iter()
            .rev()
            .any(|entry| entry.identities.contains(&address))
    });
    if protected {
        Err("frozen gameplay object is immutable".to_owned())
    } else {
        Ok(())
    }
}

/// Checks a source-of-truth module mutation.
pub(crate) fn check_module(object: *mut PyObject) -> Result<(), String> {
    check(object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ptr;

    use crate::{
        abi::{
            attr::pon_load_attr,
            map::{
                pon_build_map, pon_build_set, pon_dict_clear, pon_dict_del_item_status,
                pon_dict_set_item_status, pon_dict_update, pon_map_insert, pon_set_add,
                pon_set_discard, pon_set_update,
            },
            number::{BINARY_MUL, BINARY_OR, pon_number_inplace},
            object::{pon_del_attr, pon_set_attr},
            pon_call, pon_const_int, pon_const_str, pon_delete_global, pon_load_global,
            pon_runtime_init, pon_store_global,
            seq::{
                pon_build_list, pon_list_append, pon_list_sort, pon_seq_get_item, pon_seq_len,
                pon_seq_set_item,
            },
        },
        import,
        intern::intern,
        thread_state::{pon_err_clear, test_state_lock},
        types::{
            cell::cell_get,
            type_::{build_class_from_namespace, new_namespace},
        },
    };

    fn text(value: &str) -> *mut PyObject {
        unsafe { pon_const_str(value.as_ptr(), value.len()) }
    }

    fn init() -> std::sync::MutexGuard<'static, ()> {
        let guard = test_state_lock();
        unsafe { assert_eq!(pon_runtime_init(), 0) };
        pon_err_clear();
        guard
    }

    fn assert_error() {
        assert!(crate::thread_state::pon_err_occurred());
        assert_eq!(
            crate::thread_state::pon_err_message().as_deref(),
            Some("frozen gameplay object is immutable")
        );
        pon_err_clear();
    }

    #[test]
    fn scope_is_thread_local_and_drops_identity_set() {
        let value = Box::into_raw(Box::new(PyObject::new(core::ptr::null())));
        assert!(check(value).is_ok());
        let policy = PreparedPolicy::from_objects(&[value]);
        {
            let _scope = FrozenScope::enter(&policy);
            assert!(check(value).is_err());
        }
        assert!(check(value).is_ok());
        unsafe {
            drop(Box::from_raw(value));
        }
    }

    #[test]
    fn out_of_order_drop_removes_exact_scope() {
        let first = Box::into_raw(Box::new(PyObject::new(core::ptr::null())));
        let second = Box::into_raw(Box::new(PyObject::new(core::ptr::null())));
        let outer_policy = PreparedPolicy::from_objects(&[first]);
        let inner_policy = PreparedPolicy::from_objects(&[second]);
        let outer = FrozenScope::enter(&outer_policy);
        let inner = FrozenScope::enter(&inner_policy);
        drop(outer);
        assert!(check(second).is_err());
        drop(inner);
        assert!(check(first).is_ok());
        assert!(check(second).is_ok());
        unsafe {
            drop(Box::from_raw(first));
            drop(Box::from_raw(second));
        }
    }

    #[test]
    fn native_list_mutators_reject_a_frozen_heap_list_but_reads_and_new_lists_work() {
        let _guard = init();
        unsafe {
            let one = pon_const_int(1);
            let two = pon_const_int(2);
            let mut values = [one, two];
            let list = pon_build_list(values.as_mut_ptr(), values.len());
            let replacement = pon_const_int(9);
            let policy = PreparedPolicy::from_objects(&[list]);
            let scope = FrozenScope::enter(&policy);

            assert!(pon_list_append(list, replacement).is_null());
            assert_error();
            assert_eq!(pon_seq_set_item(list, pon_const_int(0), replacement), -1);
            assert_error();
            assert!(pon_list_sort(list).is_null());
            assert_error();

            for method in ["clear", "reverse"] {
                let callable = pon_load_attr(list, intern(method), ptr::null_mut());
                assert!(!callable.is_null(), "list.{method} lookup");
                assert!(pon_call(callable, ptr::null_mut(), 0).is_null());
                assert_error();
            }
            let index = pon_load_attr(list, intern("index"), ptr::null_mut());
            let mut index_args = [one];
            let index_result = pon_call(index, index_args.as_mut_ptr(), index_args.len());
            assert!(
                !index_result.is_null(),
                "read-only list.index remains allowed"
            );

            // Augmented repeat is an ABI call too. It may produce a fresh result,
            // but must never rewrite the protected receiver.
            let repeated = pon_number_inplace(BINARY_MUL, list, pon_const_int(2), ptr::null_mut());
            assert!(!repeated.is_null());
            assert_eq!(pon_seq_len(list), 2);
            assert_eq!(pon_seq_get_item(list, pon_const_int(0)), one);
            let fresh = pon_build_list(ptr::null_mut(), 0);
            assert!(!fresh.is_null());
            assert!(!pon_list_append(fresh, replacement).is_null());
            assert_eq!(pon_seq_len(fresh), 1);
            drop(scope);
        }
    }

    #[test]
    fn native_dict_mutators_reject_a_frozen_heap_dict_and_preserve_entries() {
        let _guard = init();
        unsafe {
            let key = text("frozen-key");
            let value = pon_const_int(1);
            let mut pairs = [key, value];
            let dict = pon_build_map(pairs.as_mut_ptr(), 1);
            let other_key = text("other-key");
            let other_value = pon_const_int(2);
            let mut other_pairs = [other_key, other_value];
            let other = pon_build_map(other_pairs.as_mut_ptr(), 1);
            let policy = PreparedPolicy::from_objects(&[dict, other]);
            let scope = FrozenScope::enter(&policy);

            assert!(pon_map_insert(dict, other_key, other_value).is_null());
            assert_error();
            assert_eq!(pon_dict_set_item_status(dict, other_key, other_value), -1);
            assert_error();
            assert!(pon_dict_update(dict, other).is_null());
            assert_error();
            assert!(pon_dict_clear(dict).is_null());
            assert_error();
            assert_eq!(pon_dict_del_item_status(dict, key), -1);
            assert_error();

            assert!(!crate::abi::map::pon_dict_get_item(dict, key).is_null());
            assert_eq!(
                crate::abi::map::pon_dict_get_item(dict, other_key),
                ptr::null_mut()
            );
            pon_err_clear();
            drop(scope);
        }
    }

    #[test]
    fn module_global_store_and_delete_reject_frozen_module_and_namespace_mirror() {
        let _guard = init();
        let name = intern("__pon_frozen_policy_regression_global__");
        let value = unsafe { pon_const_int(41) };
        let replacement = unsafe { pon_const_int(42) };
        assert!(import::active_module_object().is_none());
        import::begin_module_execution("sys").expect("sys is initialized and cached");
        let module = import::active_module_object().expect("active sys module");
        let namespace = unsafe { import::module_namespace_for_object(module) }
            .expect("module object")
            .expect("live module namespace");
        unsafe {
            assert_eq!(pon_store_global(name, value), value);
            assert_eq!(pon_load_global(name, ptr::null_mut()), value);
            pon_err_clear();
            assert_eq!(
                crate::abi::map::pon_dict_get_item(
                    namespace,
                    text("__pon_frozen_policy_regression_global__")
                ),
                value
            );
            let policy = PreparedPolicy::from_objects(&[module, namespace]);
            let _scope = FrozenScope::enter(&policy);
            assert!(pon_store_global(name, replacement).is_null());
            assert_error();
            assert_eq!(pon_load_global(name, ptr::null_mut()), value);
            assert_eq!(
                crate::abi::map::pon_dict_get_item(
                    namespace,
                    text("__pon_frozen_policy_regression_global__")
                ),
                value
            );
            assert!(pon_delete_global(name).is_null());
            assert_error();
            assert_eq!(pon_load_global(name, ptr::null_mut()), value);
            pon_err_clear();
        }
        import::end_module_execution("sys");
    }

    #[test]
    fn native_class_attribute_and_closure_cell_mutators_reject_frozen_objects() {
        let _guard = init();
        unsafe {
            let namespace = new_namespace();
            assert!(!namespace.is_null());
            let class = build_class_from_namespace("PonFrozenPolicyClass", &[], namespace, &[]);
            assert!(!class.is_null());
            let attr_name = intern("__pon_frozen_policy_regression_attr__");
            let attr_value = pon_const_int(7);
            assert_eq!(pon_set_attr(class, attr_name, attr_value), 0);
            assert_eq!(pon_del_attr(class, attr_name), 0);
            let policy = PreparedPolicy::from_objects(&[class]);
            let scope = FrozenScope::enter(&policy);
            assert_eq!(pon_set_attr(class, attr_name, attr_value), -1);
            assert_error();
            assert_eq!(pon_del_attr(class, attr_name), -1);
            assert_error();
            drop(scope);

            let initial = pon_const_int(11);
            let replacement = pon_const_int(12);
            let cell = crate::abi::call::pon_make_cell(initial);
            assert!(!cell.is_null());
            let policy = PreparedPolicy::from_objects(&[cell]);
            let scope = FrozenScope::enter(&policy);
            assert!(crate::abi::call::pon_cell_set(cell, replacement).is_null());
            assert_error();
            assert!(crate::abi::call::pon_cell_delete(cell).is_null());
            assert_error();
            assert_eq!(
                cell_get(cell.cast()).expect("cell remains assigned"),
                initial
            );
            drop(scope);
            assert!(!crate::abi::call::pon_cell_set(cell, replacement).is_null());
            assert_eq!(
                cell_get(cell.cast()).expect("cell replacement"),
                replacement
            );
        }
    }

    #[test]
    fn property_doc_and_descriptor_mutation_reject_frozen_identity_with_unfrozen_control() {
        let _guard = init();
        unsafe {
            let property_type = crate::abi::pon_load_global(intern("property"), ptr::null_mut());
            let property = crate::abi::pon_call(property_type, ptr::null_mut(), 0);
            assert!(!property.is_null());
            let doc_name = intern("__doc__");
            let doc = text("frozen property");
            assert_eq!(crate::abi::pon_set_attr(property, doc_name, doc), 0);
            let receiver = crate::abi::seq::pon_build_list(ptr::null_mut(), 0);
            let value = crate::abi::pon_const_int(3);
            assert!(!receiver.is_null());
            let scope = PreparedPolicy::from_objects(&[property, receiver]);
            let _scope = FrozenScope::enter(&scope);

            assert_eq!(crate::abi::pon_set_attr(property, doc_name, doc), -1);
            assert!(
                crate::thread_state::pon_err_message()
                    .as_deref()
                    .is_some_and(|message| message.contains("frozen gameplay object is immutable"))
            );
            pon_err_clear();
            assert_eq!(
                crate::types::property::property_descr_set(property, receiver, value),
                -1
            );
            assert!(
                crate::thread_state::pon_err_message()
                    .as_deref()
                    .is_some_and(|message| message.contains("frozen gameplay object is immutable"))
            );
            pon_err_clear();
            assert_eq!(
                crate::types::property::property_descr_set(property, receiver, ptr::null_mut()),
                -1
            );
            assert!(
                crate::thread_state::pon_err_message()
                    .as_deref()
                    .is_some_and(|message| message.contains("frozen gameplay object is immutable"))
            );
            pon_err_clear();
        }
    }

    #[test]
    fn native_getset_descriptor_set_honors_frozen_function_receiver() {
        let _guard = init();
        unsafe extern "C" fn dummy_entry(
            _argv: *mut *mut PyObject,
            _argc: usize,
        ) -> *mut PyObject {
            ptr::null_mut()
        }

        unsafe {
            // Obtain the descriptor from the canonical runtime function type,
            // then exercise its native tp_descr_set entry against a real
            // function object.  This covers the low-level getset path rather
            // than a regular function attribute helper.
            let function = crate::abi::pon_make_function(
                dummy_entry as *const u8,
                0,
                intern("frozen_getset_receiver"),
            );
            assert!(!function.is_null());
            let function_type = (*function).ob_type.cast_mut();
            let descriptor = crate::abi::pon_get_attr(
                function_type.cast(),
                intern("__defaults__"),
                ptr::null_mut(),
            );
            assert!(!descriptor.is_null());
            let setter = (*(*descriptor).ob_type)
                .tp_descr_set
                .expect("canonical getset descriptor setter");

            let mut first_items = [pon_const_int(1)];
            let first = crate::abi::seq::pon_build_tuple(first_items.as_mut_ptr(), 1);
            assert!(!first.is_null());
            assert_eq!(setter(descriptor, function, first), 0);
            pon_err_clear();

            let mut replacement_items = [pon_const_int(2)];
            let replacement =
                crate::abi::seq::pon_build_tuple(replacement_items.as_mut_ptr(), 1);
            assert!(!replacement.is_null());
            let policy = PreparedPolicy::from_objects(&[function]);
            let _scope = FrozenScope::enter(&policy);
            assert_eq!(setter(descriptor, function, replacement), -1);
            assert_error();

            let getter = (*(*descriptor).ob_type)
                .tp_descr_get
                .expect("canonical getset descriptor getter");
            let current = getter(descriptor, function, function_type.cast());
            assert_eq!(current, first);
            pon_err_clear();
            assert_eq!(setter(descriptor, function, ptr::null_mut()), -1);
            assert_error();
            let current = getter(descriptor, function, function_type.cast());
            assert_eq!(current, first);
        }
    }

    #[test]
    fn slotted_member_descriptor_set_and_delete_honor_frozen_receiver() {
        let _guard = init();
        unsafe {
            let namespace = new_namespace();
            let slots_name = intern("__slots__");
            let slot_name = text("fighter_state");
            let mut slot_values = [slot_name];
            let slots = crate::abi::seq::pon_build_tuple(slot_values.as_mut_ptr(), 1);
            (*namespace).set(slots_name, slots);
            let class = build_class_from_namespace("PonFrozenSlotted", &[], namespace, &[]);
            assert!(!class.is_null());
            let instance = crate::abi::pon_call(class, ptr::null_mut(), 0);
            assert!(!instance.is_null());
            let descriptor =
                crate::abi::pon_get_attr(class, intern("fighter_state"), ptr::null_mut());
            assert!(!descriptor.is_null());
            let value = pon_const_int(73);

            let setter = (*(*descriptor).ob_type)
                .tp_descr_set
                .expect("member descriptor setter");
            assert_eq!(setter(descriptor, instance, value), 0);
            let policy = PreparedPolicy::from_objects(&[class, instance, descriptor]);
            let _scope = FrozenScope::enter(&policy);
            assert_eq!(setter(descriptor, instance, value), -1);
            assert_error();
            assert_eq!(setter(descriptor, instance, ptr::null_mut()), -1);
            assert_error();
        }
    }

    #[test]
    fn native_set_mutators_reject_frozen_heap_set_and_preserve_members() {
        let _guard = init();
        unsafe {
            let first = pon_const_int(1);
            let second = pon_const_int(2);
            let mut entries = [first, second];
            let set = pon_build_set(entries.as_mut_ptr(), entries.len());
            let mut other_entries = [pon_const_int(3)];
            let other = pon_build_set(other_entries.as_mut_ptr(), 1);
            assert!(!set.is_null() && !other.is_null());
            let policy = PreparedPolicy::from_objects(&[set, other]);
            let _scope = FrozenScope::enter(&policy);

            assert!(pon_set_add(set, pon_const_int(4)).is_null());
            assert_error();
            assert!(pon_set_discard(set, first).is_null());
            assert_error();
            assert!(pon_set_update(set, other).is_null());
            assert_error();
            assert!(pon_number_inplace(BINARY_OR, set, other, ptr::null_mut()).is_null());
            assert_error();
            for method in ["pop", "clear"] {
                let callable = pon_load_attr(set, intern(method), ptr::null_mut());
                assert!(!callable.is_null());
                assert!(pon_call(callable, ptr::null_mut(), 0).is_null());
                assert_error();
            }
            assert_eq!(crate::types::set_::set_contains(set, first), Ok(true));
            assert_eq!(crate::types::set_::set_contains(set, second), Ok(true));

            // The variadic bulk-update methods are distinct ABI-backed
            // trampolines and must check the receiver before rebuilding it.
            for method in [
                "intersection_update",
                "difference_update",
                "symmetric_difference_update",
            ] {
                let mut control_entries = [pon_const_int(1), pon_const_int(2)];
                let control = pon_build_set(control_entries.as_mut_ptr(), control_entries.len());
                let callable = pon_load_attr(control, intern(method), ptr::null_mut());
                let mut args = [other];
                assert!(
                    !pon_call(callable, args.as_mut_ptr(), args.len()).is_null(),
                    "unfrozen {method}"
                );

                let mut frozen_entries = [pon_const_int(1), pon_const_int(2)];
                let frozen = pon_build_set(frozen_entries.as_mut_ptr(), frozen_entries.len());
                let policy = PreparedPolicy::from_objects(&[frozen, other]);
                let _scope = FrozenScope::enter(&policy);
                let callable = pon_load_attr(frozen, intern(method), ptr::null_mut());
                assert!(
                    pon_call(callable, args.as_mut_ptr(), args.len()).is_null(),
                    "frozen {method}"
                );
                assert_error();
                assert_eq!(
                    crate::types::set_::set_contains(frozen, frozen_entries[0]),
                    Ok(true)
                );
                assert_eq!(
                    crate::types::set_::set_contains(frozen, frozen_entries[1]),
                    Ok(true)
                );
            }
        }
    }
}
