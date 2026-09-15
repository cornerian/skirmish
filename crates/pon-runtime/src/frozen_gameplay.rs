//! Preparation and activation of an explicit frozen gameplay object graph.
//!
//! This is intentionally separate from [`Program`]: ordinary programs remain
//! mutable until a host explicitly prepares and activates this policy.

use std::collections::HashSet;

use pon_runtime::{
    PyObject,
    object::PyType,
    types::{
        dict,
        frozen_policy::{FrozenScope, PreparedPolicy},
        function,
        type_::{PyClassDict, PyHeapInstance},
    },
};

use crate::{Error, safety::PersistentRoots};

/// A rooted, identity based snapshot of a gameplay module graph.
pub struct FrozenGameplayGraph {
    _roots: PersistentRoots,
    identities: Vec<*mut PyObject>,
    rooted_objects: usize,
    policy: PreparedPolicy,
}

/// The active immutability policy for a prepared graph.
pub struct FrozenGameplayScope<'a> {
    _graph: std::marker::PhantomData<&'a FrozenGameplayGraph>,
    _scope: FrozenScope,
}

impl FrozenGameplayGraph {
    /// Walk declared module namespaces and retained move/callback objects.
    ///
    /// Every reachable object is visited once by identity. Unsupported object
    /// layouts fail preparation with a diagnostic; they are never silently
    /// assumed immutable. `modules` and `retained` must remain valid runtime
    /// pointers while this method executes.
    /// # Safety
    ///
    /// The caller must hold Pon's runtime lock on the attached current
    /// thread and keep the runtime in a GC safe region for preparation.
    /// Pointers must identify live Pon objects.
    pub unsafe fn prepare(
        modules: &[*mut PyObject],
        retained: &[*mut PyObject],
    ) -> Result<Self, Error> {
        if modules.is_empty() && retained.is_empty() {
            return Ok(Self {
                _roots: PersistentRoots::new(),
                identities: Vec::new(),
                rooted_objects: 0,
                policy: PreparedPolicy::from_objects(&[]),
            });
        }
        let mut walk = Walker::new()?;
        for &module in modules {
            walk.object(module)?;
        }
        for &object in retained {
            walk.object(object)?;
        }

        let mut roots = PersistentRoots::new();
        for &object in &walk.objects {
            roots.push(object);
        }
        let rooted_objects = walk.objects.len();
        // PersistentRoots stores GC objects. Non-header cells and class
        // namespaces still need identity protection, so they are retained in
        // the identity vector and included in FrozenScope below.
        let mut identities = walk.objects;
        identities.extend(walk.raw_identities);
        let policy = PreparedPolicy::from_objects(&identities);
        Ok(Self {
            _roots: roots,
            identities,
            rooted_objects,
            policy,
        })
    }

    /// Activate the already prepared graph on the current attached thread.
    pub fn activate(&self) -> FrozenGameplayScope<'_> {
        FrozenGameplayScope {
            _graph: std::marker::PhantomData,
            _scope: FrozenScope::enter(&self.policy),
        }
    }

    /// Number of unique identities protected by this snapshot.
    #[must_use]
    pub fn object_count(&self) -> usize {
        self.identities.len()
    }

    /// Keep roots alive for the graph lifetime. This accessor is useful to
    /// hosts that need to document the ownership boundary without exposing
    /// mutable internals.
    #[must_use]
    pub fn rooted_object_count(&self) -> usize {
        // PersistentRoots intentionally exposes no mutable collection API.
        // The identity count is the authoritative snapshot size.
        self.rooted_objects
    }
}

struct Walker {
    seen: HashSet<usize>,
    visited_descriptors: HashSet<usize>,
    objects: Vec<*mut PyObject>,
    raw_identities: Vec<*mut PyObject>,
    list_type: *const PyType,
    tuple_type: *const PyType,
    dict_type: *const PyType,
    set_type: *const PyType,
    frozenset_type: *const PyType,
    function_type: *const PyType,
    type_type: *const PyType,
    scalar_types: HashSet<usize>,
    rejected_types: HashSet<usize>,
    staticmethod_type: *const PyType,
    classmethod_type: *const PyType,
    property_type: *const PyType,
    method_type: *const PyType,
    member_descriptor_type: *const PyType,
    getset_descriptor_type: *const PyType,
    monitoring_type: *const PyType,
    monitoring_events_type: *const PyType,
    int_type: *const PyType,
    str_type: *const PyType,
    bytes_type: *const PyType,
    sre_pattern_type: *const PyType,
}

const MAX_GRAPH_NODES: usize = 1_000_000;

#[derive(Clone, Copy)]
enum WorkItem {
    Object(*mut PyObject),
    Type(*mut PyType),
    Namespace(*mut PyClassDict),
}

impl Walker {
    fn new() -> Result<Self, Error> {
        let type_for = pon_runtime::abi::canonical_builtin_type;
        let none_type = type_for("NoneType");
        let list_type = type_for("list");
        let tuple_type = type_for("tuple");
        let dict_type = type_for("dict");
        let set_type = type_for("set");
        let frozenset_type = type_for("frozenset");
        let function_type = type_for("function");
        let type_type = type_for("type");
        let staticmethod_type = type_for("staticmethod");
        let classmethod_type = type_for("classmethod");
        let property_type = type_for("property");
        let method_type = type_for("method");
        let member_descriptor_type = type_for("member_descriptor");
        let getset_descriptor_type = type_for("getset_descriptor");
        let monitoring_type = pon_runtime::abi::monitoring_type_identity();
        let monitoring_events_type = pon_runtime::abi::monitoring_events_type_identity();
        let int_type = type_for("int");
        let str_type = type_for("str");
        let bytes_type = type_for("bytes");
        let sre_pattern_type = pon_runtime::abi::sre_pattern_type_identity();
        let scalar_names = [
            "bool",
            "int",
            "float",
            "complex",
            "str",
            "bytes",
            "range",
            "Token.MISSING",
            "object",
        ];
        let scalar_types = scalar_names
            .into_iter()
            .map(type_for)
            .chain(std::iter::once(none_type))
            .filter(|ty| !ty.is_null())
            .map(|ty| ty as usize)
            .collect();
        let rejected_types = ["bytearray", "slice"]
            .into_iter()
            .map(type_for)
            .filter(|ty| !ty.is_null())
            .map(|ty| ty as usize)
            .collect();
        if [
            list_type,
            tuple_type,
            dict_type,
            set_type,
            frozenset_type,
            function_type,
            type_type,
        ]
        .into_iter()
        .any(|ty| ty.is_null())
        {
            return Err(unsupported("canonical builtin type unavailable"));
        }
        Ok(Self {
            seen: HashSet::new(),
            visited_descriptors: HashSet::new(),
            objects: Vec::new(),
            raw_identities: Vec::new(),
            list_type,
            tuple_type,
            dict_type,
            set_type,
            frozenset_type,
            function_type,
            type_type,
            staticmethod_type,
            classmethod_type,
            property_type,
            method_type,
            member_descriptor_type,
            getset_descriptor_type,
            monitoring_type,
            monitoring_events_type,
            int_type,
            str_type,
            bytes_type,
            sre_pattern_type,
            scalar_types,
            rejected_types,
        })
    }

    fn object(&mut self, root: *mut PyObject) -> Result<(), Error> {
        let mut work = vec![WorkItem::Object(root)];
        while let Some(item) = work.pop() {
            if work.len() > MAX_GRAPH_NODES {
                return Err(unsupported("gameplay graph worklist limit exceeded"));
            }
            match item {
                WorkItem::Object(object) => self.object_one(object, &mut work)?,
                WorkItem::Type(ty) => self.type_one(ty, &mut work)?,
                WorkItem::Namespace(namespace) => self.namespace_one(namespace, &mut work)?,
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn object_collect(&mut self, root: *mut PyObject) -> Vec<Error> {
        let mut errors = Vec::new();
        let mut parents = std::collections::HashMap::<usize, usize>::new();
        let mut work = vec![WorkItem::Object(root)];
        while let Some(item) = work.pop() {
            let current = match item {
                WorkItem::Object(object) => object as usize,
                WorkItem::Type(ty) => ty as usize,
                WorkItem::Namespace(namespace) => namespace as usize,
            };
            let before = work.len();
            let result = match item {
                WorkItem::Object(object) => self.object_one(object, &mut work),
                WorkItem::Type(ty) => self.type_one(ty, &mut work),
                WorkItem::Namespace(namespace) => self.namespace_one(namespace, &mut work),
            };
            for child in work.iter().skip(before) {
                let child_id = match child {
                    WorkItem::Object(object) => *object as usize,
                    WorkItem::Type(ty) => *ty as usize,
                    WorkItem::Namespace(namespace) => *namespace as usize,
                };
                parents.entry(child_id).or_insert(current);
            }
            if let Err(error) = result {
                let mut chain = vec![current];
                let mut cursor = current;
                while let Some(parent) = parents.get(&cursor).copied() {
                    if chain.contains(&parent) {
                        break;
                    }
                    chain.push(parent);
                    cursor = parent;
                }
                errors.push(Error::Value(format!(
                    "{error}; diagnostic identity path: {chain:?}"
                )));
            }
        }
        errors
    }

    fn object_one(&mut self, object: *mut PyObject, work: &mut Vec<WorkItem>) -> Result<(), Error> {
        if object.is_null()
            || !pon_runtime::tag::is_heap(object)
            || !self.seen.insert(object as usize)
        {
            return Ok(());
        }
        if self.objects.len() >= MAX_GRAPH_NODES {
            return Err(unsupported("gameplay graph node limit exceeded"));
        }
        self.objects.push(object);
        let ty = unsafe { (*object).ob_type };
        if ty.is_null() {
            return Err(unsupported(format!(
                "object has no runtime type at 0x{:x}",
                object as usize
            )));
        }
        if self.scalar_types.contains(&(ty as usize)) {
            return Ok(());
        }
        if std::ptr::eq(ty, self.monitoring_type) || std::ptr::eq(ty, self.monitoring_events_type) {
            // These private native objects are constants-only, header-sized
            // compatibility surfaces. If monitoring mutators are added, this
            // proof must be revisited with a native capability/barrier audit.
            let header_size = std::mem::size_of::<pon_runtime::object::PyObjectHeader>();
            let proven_header_only = unsafe {
                (*ty).instance_size == header_size
                    && (*ty).tp_basicsize == header_size
                    && (*ty).tp_itemsize == 0
                    && (*ty).tp_dict.is_null()
                    && (*ty).tp_setattro.is_none()
            };
            if !proven_header_only {
                return Err(unsupported(
                    "sys.monitoring compatibility object layout changed",
                ));
            }
            return Ok(());
        }
        if std::ptr::eq(ty, self.sre_pattern_type) {
            let refs = unsafe { pon_runtime::abi::sre_pattern_python_refs(object) }
                .ok_or_else(|| unsupported("re.Pattern layout validation failed"))?;
            work.push(WorkItem::Object(refs.0));
            work.push(WorkItem::Object(refs.1));
            return Ok(());
        }
        if ty == self.list_type {
            let list = unsafe { &*object.cast::<pon_runtime::types::list::PyList>() };
            for child in unsafe { list.as_slice() } {
                work.push(WorkItem::Object(*child));
            }
        } else if ty == self.tuple_type {
            let tuple = unsafe { &*object.cast::<pon_runtime::types::tuple::PyTuple>() };
            for child in unsafe { tuple.as_slice() } {
                work.push(WorkItem::Object(*child));
            }
        } else if ty == self.dict_type {
            for entry in unsafe { dict::dict_entries_snapshot(object) }.map_err(Error::Runtime)? {
                work.push(WorkItem::Object(entry.key));
                work.push(WorkItem::Object(entry.value));
            }
        } else if ty == self.set_type || ty == self.frozenset_type {
            for child in unsafe { pon_runtime::types::set_::entries_snapshot(object) }
                .map_err(Error::Runtime)?
            {
                work.push(WorkItem::Object(child));
            }
        } else if self.rejected_types.contains(&(ty as usize)) {
            return Err(unsupported(format!(
                "unsupported mutable graph type `{}`",
                unsafe { ty.as_ref().unwrap().name() }
            )));
        } else if pon_runtime::import::module_object_registry_key(object).is_some() {
            self.module(object, work)?;
        } else if ty == self.function_type {
            self.function(object, work)?;
        } else if ty == self.staticmethod_type {
            work.push(WorkItem::Object(unsafe {
                (*object.cast::<pon_runtime::types::classmethod::PyStaticMethod>()).callable
            }));
        } else if ty == self.classmethod_type {
            work.push(WorkItem::Object(unsafe {
                (*object.cast::<pon_runtime::types::classmethod::PyClassMethod>()).callable
            }));
        } else if ty == self.property_type {
            let property = unsafe { &*object.cast::<pon_runtime::types::property::PyProperty>() };
            work.extend([
                WorkItem::Object(property.fget),
                WorkItem::Object(property.fset),
                WorkItem::Object(property.fdel),
                WorkItem::Object(property.doc),
            ]);
        } else if ty == self.method_type {
            let method = unsafe { &*object.cast::<pon_runtime::types::method::PyMethod>() };
            work.push(WorkItem::Object(method.function()));
            work.push(WorkItem::Object(method.receiver()));
        } else if ty == self.member_descriptor_type {
            // Slot descriptors carry only an owner/type offset and no outgoing
            // Python object references; their mutability is governed by the
            // frozen class namespace itself.
            return Ok(());
        } else if ty == self.getset_descriptor_type {
            let owner = unsafe { pon_runtime::descr::getset_descriptor_objclass(object) };
            if let Some(owner) = owner {
                work.push(WorkItem::Type(owner));
            }
            return Ok(());
        } else if self.is_type_object_exact(object) {
            work.push(WorkItem::Type(object.cast()));
        } else if unsafe { pon_runtime::types::type_::is_payload_subclass_instance(object) } {
            let payload_type = ty.cast_mut();
            let payload_base = unsafe { pon_runtime::mro::mro_entries(payload_type) }
                .into_iter()
                .find(|base| {
                    let base = *base as *const PyType;
                    base == self.int_type || base == self.str_type || base == self.bytes_type
                });
            if payload_base.is_none() {
                return Err(unsupported(format!(
                    "payload subclass `{}` has no canonical int/str/bytes base",
                    unsafe { ty.as_ref().unwrap().name() }
                )));
            }
            self.heap_instance(object, work)?;
            let value = unsafe { pon_runtime::types::type_::payload_subclass_value(object) }
                .ok_or_else(|| unsupported("payload subclass has no embedded builtin value"))?;
            work.push(WorkItem::Object(value));
        } else if unsafe {
            pon_runtime::types::list::is_list_subclass_instance(object)
                || pon_runtime::types::tuple::is_tuple_subclass_instance(object)
                || pon_runtime::types::dict::is_dict_subclass_instance(object)
                || pon_runtime::types::type_::is_payload_subclass_instance(object)
        } {
            let (name, bases) = unsafe {
                let mut names = Vec::new();
                let mut base = (*ty).tp_base;
                while !base.is_null() && names.len() < 16 {
                    names.push((*base).name().to_owned());
                    base = (*base).tp_base;
                }
                ((*ty).name().to_owned(), names.join(" -> "))
            };
            return Err(unsupported(format!(
                "builtin subclass instance type `{name}` at 0x{:x} (base chain: {bases}) has an unsupported payload layout",
                object as usize
            )));
        } else if unsafe { (*ty).gc_type_id }
            == pon_runtime::types::type_::TYPE_ID_HEAP_INSTANCE.0 as usize
        {
            self.heap_instance(object, work)?;
        } else {
            return Err(unsupported(format!(
                "unsupported mutable graph type `{}` at object 0x{:x} (type 0x{:x}, gc_type_id {}, basicsize {})",
                unsafe { ty.as_ref().unwrap().name() },
                object as usize,
                ty as usize,
                unsafe { (*ty).gc_type_id },
                unsafe { (*ty).tp_basicsize },
            )));
        }
        Ok(())
    }

    fn module(&mut self, module: *mut PyObject, work: &mut Vec<WorkItem>) -> Result<(), Error> {
        if let Some(values) = pon_runtime::import::module_object_attr_values(module) {
            for value in values {
                work.push(WorkItem::Object(value));
            }
        }
        let namespace = unsafe { pon_runtime::import::module_namespace_for_object(module) }
            .ok_or_else(|| unsupported("module layout disappeared"))?
            .map_err(Error::Runtime)?;
        work.push(WorkItem::Object(namespace));
        Ok(())
    }

    fn function(
        &mut self,
        function_object: *mut PyObject,
        work: &mut Vec<WorkItem>,
    ) -> Result<(), Error> {
        let function = unsafe { &*function_object.cast::<pon_runtime::object::PyFunction>() };
        work.push(WorkItem::Object(function.annotations));
        work.push(WorkItem::Object(function.attr_dict));
        let closure = function::function_record(function_object)
            .map(|record| record.closure())
            .unwrap_or_default();
        for &cell in &closure {
            self.raw_identity(cell);
            // Closure cells are Rust side storage without a PyObject header.
            // Their current value is still part of the gameplay graph.
            let value =
                unsafe { pon_runtime::types::cell::cell_get(cell.cast()).map_err(Error::Runtime)? };
            work.push(WorkItem::Object(value));
        }
        // This established runtime visitor includes positional defaults,
        // keyword-only defaults, closure cells, annotations, and function
        // module values. Skip cells here because they were handled above.
        let closure_ids: HashSet<usize> = closure.iter().map(|value| *value as usize).collect();
        let mut refs = Vec::new();
        function::visit_function_gc_refs(function_object, &mut |value| {
            if closure_ids.contains(&(value as usize)) {
                return;
            }
            refs.push(value.cast());
        });
        for value in refs {
            work.push(WorkItem::Object(value));
        }
        Ok(())
    }

    fn heap_instance(
        &mut self,
        object: *mut PyObject,
        work: &mut Vec<WorkItem>,
    ) -> Result<(), Error> {
        let instance = unsafe { &*object.cast::<PyHeapInstance>() };
        if !instance.dict.is_null() {
            work.push(WorkItem::Namespace(instance.dict));
        }
        for slot in &instance.slots {
            work.push(WorkItem::Object(slot.value));
        }
        work.push(WorkItem::Type(instance.ob_base.ob_type.cast_mut()));
        Ok(())
    }

    fn type_one(
        &mut self,
        type_object: *mut PyType,
        work: &mut Vec<WorkItem>,
    ) -> Result<(), Error> {
        if type_object.is_null() {
            return Ok(());
        }
        if !self.visited_descriptors.insert(type_object as usize) {
            return Ok(());
        }
        self.raw_identities.push(type_object.cast());
        if !unsafe { (*type_object).tp_dict }.is_null() {
            work.push(WorkItem::Namespace(unsafe {
                (*type_object).tp_dict.cast::<PyClassDict>()
            }));
        }
        // MRO/base carriers are leaked Rust side records with a NULL object
        // type; traverse their validated entries rather than casting them to
        // ordinary PyObjects.
        self.raw_identity(unsafe { (*type_object).tp_mro });
        self.raw_identity(unsafe { (*type_object).tp_bases });
        for base in unsafe { pon_runtime::mro::mro_entries(type_object) } {
            work.push(WorkItem::Type(base));
        }
        Ok(())
    }

    fn namespace_one(
        &mut self,
        namespace: *mut PyClassDict,
        work: &mut Vec<WorkItem>,
    ) -> Result<(), Error> {
        if namespace.is_null() {
            return Ok(());
        }
        if !self.seen.insert(namespace as usize) {
            return Ok(());
        }
        self.raw_identities.push(namespace.cast());
        for (_, value) in unsafe { &*namespace }.iter() {
            work.push(WorkItem::Object(value));
        }
        Ok(())
    }

    fn raw_identity(&mut self, object: *mut PyObject) {
        if !object.is_null() && self.seen.insert(object as usize) {
            self.raw_identities.push(object);
        }
    }

    fn is_type_object_exact(&self, object: *mut PyObject) -> bool {
        let mut meta = unsafe { (*object).ob_type.cast_mut() };
        while !meta.is_null() {
            if meta == self.type_type.cast_mut() {
                return true;
            }
            meta = unsafe { (*meta).tp_base };
        }
        false
    }
}

fn unsupported(message: impl Into<String>) -> Error {
    Error::Value(format!("cannot freeze gameplay graph: {}", message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::safety::{Attachment, PersistentRoots, StackBoundary};
    use pon_runtime::{
        abi, intern,
        types::type_::{ClassKeyword, build_class_from_namespace, new_namespace},
    };

    #[test]
    #[ignore = "diagnostic: records first unsupported reachable Fox SDK node"]
    fn diagnostic_real_fox_graph_first_unsupported_node() {
        let archive = std::fs::File::open(
            "/tmp/skirmish-stdlib-release-proof/pon-stdlib-final-sorted.tar.gz",
        )
        .expect("verified Pon stdlib archive");
        let digest = |hex: &str| -> [u8; 32] {
            (0..32)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let library = crate::StandardLibrary::from_archive(
            archive,
            digest("5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c"),
        )
        .expect("verified Pon stdlib");
        let root = std::env::temp_dir().join("skirmish-pon-fox-graph-diagnostic");
        let materialized = library.materialize(&root).expect("materialize stdlib");
        let bundle = crate::SourceBundle::from_directory(
            "fox-graph-diagnostic-sdk",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/api"),
        )
        .expect("SDK source bundle")
        .materialize(std::env::temp_dir().join("skirmish-pon-fox-graph-bundle"))
        .expect("materialize SDK bundle");
        let program = crate::Program::new(
            include_str!("../../../scripts/fighters/fox.py"),
            "fox.py",
            std::iter::empty::<&str>(),
        )
        .with_standard_library(&materialized);
        let prepared = program
            .prepare_for_thread_in_bundle(&bundle)
            .expect("prepare real Fox program");
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let mut modules = prepared
            .owned_modules
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(_, module)| *module)
            .collect::<Vec<_>>();
        let root_module = pon_runtime::import::cached_module(intern(&prepared.module_name))
            .expect("Fox root module");
        modules.push(root_module);
        let mut diagnostic_errors = Vec::new();
        for (name, module) in prepared.owned_modules.as_deref().unwrap_or_default() {
            if let Err(error) = unsafe { FrozenGameplayGraph::prepare(&[*module], &[]) } {
                if name == "fighter"
                    && let Some(Ok(namespace)) =
                        unsafe { pon_runtime::import::module_namespace_for_object(*module) }
                    && let Ok(dict) = unsafe { pon_runtime::types::dict::dict_ref(namespace) }
                {
                    for entry in &dict.entries {
                        let attr = unsafe { pon_runtime::types::type_::unicode_text(entry.key) }
                            .unwrap_or("<non-text>");
                        let mut walker = Walker::new().expect("diagnostic walker");
                        let attr_errors = walker.object_collect(entry.value);
                        if !attr_errors.is_empty() {
                            let attr_error = attr_errors
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("; ");
                            let evidence = unsafe {
                                if entry.value.is_null() || !pon_runtime::tag::is_heap(entry.value)
                                {
                                    "non-heap/null".to_owned()
                                } else {
                                    let ty = (*entry.value).ob_type;
                                    format!(
                                        "object=0x{:x} type=0x{:x} type_name={} type_gc_id={} type_basicsize={} type_base=0x{:x}",
                                        entry.value as usize,
                                        ty as usize,
                                        ty.as_ref().map(|v| v.name()).unwrap_or("<null>"),
                                        ty.as_ref().map(|v| v.gc_type_id).unwrap_or(0),
                                        ty.as_ref().map(|v| v.tp_basicsize).unwrap_or(0),
                                        ty.as_ref().map(|v| v.tp_base as usize).unwrap_or(0),
                                    )
                                }
                            };
                            diagnostic_errors.push(format!(
                                "root module `{name}` attr `{attr}`: {attr_error}; {evidence}"
                            ));
                        }
                    }
                }
                diagnostic_errors.push(format!("root module `{name}`: {error}"));
            }
        }
        let result = unsafe { FrozenGameplayGraph::prepare(&modules, &prepared.callback_slots) };
        if !diagnostic_errors.is_empty() {
            panic!(
                "real Fox graph diagnostic batch ({} entries):\n{}",
                diagnostic_errors.len(),
                diagnostic_errors.join("\n")
            );
        }
        match result {
            Ok(graph) => panic!(
                "real Fox graph unexpectedly freezeable: {} objects",
                graph.object_count()
            ),
            Err(error) => panic!("real Fox graph diagnostic: {error}"),
        }
    }

    #[test]
    fn empty_snapshot_can_be_entered_without_changing_mutable_program_defaults() {
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[], &[]) }.unwrap();
        assert_eq!(snapshot.object_count(), 0);
        let _scope = snapshot.activate();
    }

    #[test]
    fn cyclic_lists_are_visited_once_and_rooted() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let first = unsafe { abi::seq::pon_build_list(std::ptr::null_mut(), 0) };
        let second = unsafe { abi::seq::pon_build_list(std::ptr::null_mut(), 0) };
        assert!(!first.is_null() && !second.is_null());
        let mut roots = PersistentRoots::new();
        roots.push(first);
        roots.push(second);
        unsafe {
            assert_eq!(abi::seq::pon_list_append(first, second), first);
            assert_eq!(abi::seq::pon_list_append(second, first), second);
        }
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[first], &[]) }.unwrap();
        assert_eq!(snapshot.rooted_object_count(), 2);
        assert_eq!(snapshot.object_count(), 2);
    }

    #[test]
    fn mutable_bytearray_is_rejected_during_preparation() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let bytearray_type =
            unsafe { abi::pon_load_global(pon_runtime::intern("bytearray"), std::ptr::null_mut()) };
        let object = unsafe { abi::pon_call(bytearray_type, std::ptr::null_mut(), 0) };
        assert!(!object.is_null());
        let mut roots = PersistentRoots::new();
        roots.push(object);
        let result = unsafe { FrozenGameplayGraph::prepare(&[object.cast()], &[]) };
        let error = match result {
            Ok(_) => panic!("bytearray unexpectedly accepted"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("bytearray"));
    }

    #[test]
    fn spoofed_builtin_names_and_builtin_list_subclass_are_layout_checked() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let type_object = unsafe { abi::pon_load_global(intern("type"), std::ptr::null_mut()) };
        let list_object = unsafe { abi::pon_load_global(intern("list"), std::ptr::null_mut()) };
        assert!(!type_object.is_null() && !list_object.is_null());

        let meta_namespace = new_namespace();
        let meta = unsafe {
            build_class_from_namespace("PonFrozenMeta", &[type_object], meta_namespace, &[])
        };
        assert!(!meta.is_null());
        let spoof_namespace = new_namespace();
        let spoof = unsafe { build_class_from_namespace("list", &[], spoof_namespace, &[]) };
        assert!(!spoof.is_null());
        let subclass_namespace = new_namespace();
        let subclass = unsafe {
            build_class_from_namespace(
                "PonFrozenListSubclass",
                &[list_object],
                subclass_namespace,
                &[],
            )
        };
        assert!(!subclass.is_null());
        let instance = unsafe { abi::pon_call(subclass, std::ptr::null_mut(), 0) };
        assert!(!instance.is_null());
        let mut roots = PersistentRoots::new();
        for object in [type_object, list_object, meta, spoof, subclass, instance] {
            roots.push(object);
        }
        let spoof_snapshot = unsafe { FrozenGameplayGraph::prepare(&[spoof], &[]) }
            .expect("spoofed list name must use its actual layout");
        assert!(spoof_snapshot.object_count() > 0);
        let result = unsafe { FrozenGameplayGraph::prepare(&[instance], &[]) };
        let error = match result {
            Ok(_) => panic!("builtin list subclass must be rejected"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("unsupported payload layout"),
            "{error}"
        );
    }

    #[test]
    fn str_payload_subclass_payload_and_instance_namespace_are_frozen() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe { assert_eq!(abi::pon_runtime_init(), 0) };
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let str_type = unsafe { abi::pon_load_global(intern("str"), std::ptr::null_mut()) };
        let type_type = unsafe { abi::pon_load_global(intern("type"), std::ptr::null_mut()) };
        let subclass = unsafe {
            build_class_from_namespace("FrozenStrSubclass", &[str_type], new_namespace(), &[])
        };
        assert!(!subclass.is_null());
        let text = unsafe { abi::pon_const_str(b"flag".as_ptr(), 4) };
        let new = unsafe { abi::pon_get_attr(str_type, intern("__new__"), std::ptr::null_mut()) };
        let mut argv = [subclass.cast::<PyObject>(), text];
        let instance = unsafe { abi::pon_call(new, argv.as_mut_ptr(), argv.len()) };
        assert!(!instance.is_null());
        let key = intern("mutable_marker");
        let before = unsafe { abi::pon_const_int(7) };
        assert_eq!(unsafe { abi::pon_set_attr(instance, key, before) }, 0);
        let mut roots = PersistentRoots::new();
        roots.push(instance);
        roots.push(subclass);
        roots.push(type_type);
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[instance], &[]) }
            .expect("str payload subclass layout");
        let _scope = snapshot.activate();
        let after = unsafe { abi::pon_const_int(8) };
        assert_eq!(unsafe { abi::pon_set_attr(instance, key, after) }, -1);
        pon_runtime::thread_state::pon_err_clear();
        assert_eq!(
            unsafe { abi::pon_get_attr(instance, key, std::ptr::null_mut()) },
            before
        );
    }

    #[test]
    fn compiled_pattern_named_group_is_traversed_and_groupindex_is_frozen() {
        let archive = std::fs::File::open(
            "/tmp/skirmish-stdlib-release-proof/pon-stdlib-final-sorted.tar.gz",
        )
        .expect("verified Pon stdlib archive");
        let digest = |hex: &str| -> [u8; 32] {
            (0..32)
                .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
                .collect::<Vec<_>>()
                .try_into()
                .unwrap()
        };
        let library = crate::StandardLibrary::from_archive(
            archive,
            digest("5c7ce12a21f4d5ae49a8cb2c425911bc4de863f427e0ff174fb74ff7bf63639c"),
        )
        .expect("verified Pon stdlib");
        let materialized = library
            .materialize(std::env::temp_dir().join("skirmish-pon-pattern-stdlib"))
            .expect("materialize stdlib");
        let mut program = crate::Program::new(
            "import re\npattern = re.compile('(?P<word>a)')\ndef read():\n    return pattern\n",
            "pattern_graph.py",
            ["read"],
        )
        .with_standard_library(&materialized)
        .prepare_for_thread()
        .expect("compile named-group pattern");
        let module_name = program.module_name.clone();
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe { assert_eq!(abi::pon_runtime_init(), 0) };
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let module = pon_runtime::import::cached_module(intern(&module_name)).unwrap();
        let pattern = pon_runtime::import::module_object_attr(module, intern("pattern"))
            .expect("compiled pattern module binding");
        let groupindex =
            unsafe { abi::pon_get_attr(pattern, intern("groupindex"), std::ptr::null_mut()) };
        assert!(!groupindex.is_null());
        let key = unsafe { abi::pon_const_str(b"word".as_ptr(), 4) };
        let before = unsafe { abi::map::pon_dict_get_item(groupindex, key) };
        assert!(!before.is_null(), "named group entry");
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[pattern], &[]) }
            .expect("compiled pattern graph");
        let _scope = snapshot.activate();
        let replacement = unsafe { abi::pon_const_int(99) };
        assert_eq!(
            unsafe { abi::map::pon_dict_set_item_status(groupindex, key, replacement) },
            -1
        );
        pon_runtime::thread_state::pon_err_clear();
        let after = unsafe { abi::map::pon_dict_get_item(groupindex, key) };
        assert!(!after.is_null(), "preserved named group entry");
        assert_eq!(after, before);
        let _ = &mut program;
    }

    #[test]
    fn custom_metaclass_namespace_cycle_is_visited_without_invalid_layout_cast() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let type_object = unsafe { abi::pon_load_global(intern("type"), std::ptr::null_mut()) };
        let meta = unsafe {
            build_class_from_namespace("FighterMeta", &[type_object], new_namespace(), &[])
        };
        assert!(!meta.is_null());
        let cycle = unsafe { abi::seq::pon_build_list(std::ptr::null_mut(), 0) };
        assert!(!cycle.is_null());
        unsafe {
            assert_eq!(abi::seq::pon_list_append(cycle, cycle), cycle);
        }
        let namespace = new_namespace();
        unsafe {
            (*namespace).set(intern("cycle"), cycle);
        }
        let class = unsafe {
            build_class_from_namespace(
                "Fighter",
                &[],
                namespace,
                &[ClassKeyword {
                    name: intern("metaclass"),
                    value: meta,
                }],
            )
        };
        assert!(!class.is_null());
        let mut roots = PersistentRoots::new();
        for object in [type_object, meta, cycle, class] {
            roots.push(object);
        }
        let snapshot =
            unsafe { FrozenGameplayGraph::prepare(&[class], &[]) }.expect("custom class graph");
        assert!(snapshot.object_count() >= 4);
        assert!(snapshot.rooted_object_count() >= 3);
    }

    #[test]
    fn module_graph_preparation_roots_namespace_mirror_and_blocks_global_mutation() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let payload = unsafe { abi::seq::pon_build_list(std::ptr::null_mut(), 0) };
        let module = pon_runtime::import::install_module(
            "pon_frozen_graph_module",
            [(intern("payload"), payload)],
        )
        .unwrap();
        assert!(!module.is_null() && !payload.is_null());
        let key = intern("__frozen_graph_module_regression__");
        let value = unsafe { abi::pon_const_int(17) };
        unsafe {
            assert_eq!(abi::pon_set_attr(module, key, value), 0);
        }
        let namespace = unsafe { pon_runtime::import::module_namespace_for_object(module) }
            .unwrap()
            .unwrap();
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[module], &[]) }.unwrap();
        assert!(snapshot.object_count() >= 2);
        let _scope = snapshot.activate();
        let replacement = unsafe { abi::pon_const_int(18) };
        assert_eq!(unsafe { abi::pon_set_attr(module, key, replacement) }, -1);
        assert_eq!(
            unsafe {
                abi::map::pon_dict_set_item_status(
                    namespace,
                    abi::pon_const_str(b"__frozen_graph_module_regression__".as_ptr(), 35),
                    replacement,
                )
            },
            -1
        );
        pon_runtime::thread_state::pon_err_clear();
        assert_eq!(
            pon_runtime::import::module_object_attr(module, key),
            Some(value)
        );
        pon_runtime::thread_state::pon_err_clear();
    }

    #[test]
    fn sys_graph_explicitly_rejects_host_state_subclass_nodes() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let sys = pon_runtime::import::cached_module(intern("sys")).unwrap_or(std::ptr::null_mut());
        assert!(!sys.is_null());
        let mut roots = PersistentRoots::new();
        roots.push(sys);
        let result = unsafe { FrozenGameplayGraph::prepare(&[sys], &[]) };
        let error = match result {
            Ok(_) => panic!("sys host state unexpectedly accepted"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("unsupported mutable graph type"),
            "{error}"
        );
    }

    #[test]
    fn sys_monitoring_constants_stubs_have_exact_header_only_layout() {
        let _lock = crate::PON_RUNTIME_LOCK.lock().unwrap();
        let _parked = crate::safety::GcSafeRegion::suspend_all();
        let _attachment = Attachment::acquire().unwrap();
        unsafe {
            assert_eq!(abi::pon_runtime_init(), 0);
        }
        let mut marker = 0usize;
        let _stack = unsafe { StackBoundary::capture((&mut marker as *mut usize).cast()) };
        let sys = pon_runtime::import::cached_module(intern("sys")).unwrap();
        let monitoring = pon_runtime::import::module_object_attr(sys, intern("monitoring"))
            .expect("sys.monitoring seed");
        assert_eq!(
            unsafe { (*monitoring).ob_type } as *const _,
            abi::monitoring_type_identity()
        );
        let events =
            unsafe { abi::pon_get_attr(monitoring, intern("events"), std::ptr::null_mut()) };
        assert!(!events.is_null());
        assert_eq!(
            unsafe { (*events).ob_type } as *const _,
            abi::monitoring_events_type_identity()
        );
        for object in [monitoring, events] {
            let ty = unsafe { (*object).ob_type };
            let header_size = std::mem::size_of::<pon_runtime::object::PyObjectHeader>();
            assert_eq!(unsafe { (*ty).instance_size }, header_size);
            assert_eq!(unsafe { (*ty).tp_basicsize }, header_size);
            assert_eq!(unsafe { (*ty).tp_itemsize }, 0);
            assert!(unsafe { (*ty).tp_dict.is_null() });
            assert!(unsafe { (*ty).tp_setattro.is_none() });
        }
        let snapshot = unsafe { FrozenGameplayGraph::prepare(&[monitoring], &[]) }
            .expect("proven constants-only monitoring leaf");
        assert!(snapshot.object_count() >= 1);
    }
}
