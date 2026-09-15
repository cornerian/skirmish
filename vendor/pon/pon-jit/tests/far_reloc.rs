use cranelift_codegen::{control::ControlPlane, ir::InstBuilder};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module as _, default_libcall_names};
use pon_codegen::isa::{OptLevel, make_isa};

// A deliberately distant address. The test never calls this symbol; it only
// checks that a non-colocated external call is emitted with a long-range
// relocation that JIT relocation resolution can install without i32 overflow.
const FAR_SYMBOL_ADDR: usize = 0x7fff_0000_0000;

#[test]
fn far_external_call_uses_long_range_relocation() {
    let isa = make_isa(OptLevel::None, false);
    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    builder.symbol("far_external", FAR_SYMBOL_ADDR as *const u8);
    let mut module = JITModule::new(builder);

    let mut sig = module.make_signature();
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(
        module.target_config().pointer_type(),
    ));
    let entry = module
        .declare_function("entry", Linkage::Local, &sig)
        .unwrap();
    let far = module
        .declare_function("far_external", Linkage::Import, &sig)
        .unwrap();

    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let far_ref = module.declare_func_in_func(far, &mut ctx.func);
    // Keep this assertion tied to the production policy: Import declarations
    // must lower as non-colocated before backend code emission.
    assert!(!ctx.func.dfg.ext_funcs[far_ref].colocated);
    let mut fctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
        let block = b.create_block();
        b.switch_to_block(block);
        b.seal_block(block);
        let call = b.ins().call(far_ref, &[]);
        let result = b.inst_results(call)[0];
        b.ins().return_(&[result]);
        b.finalize();
    }

    module.define_function(entry, &mut ctx).unwrap();
    module.finalize_definitions().unwrap();
}

#[test]
fn colocated_far_external_emits_the_overflowing_near_reloc() {
    let isa = make_isa(OptLevel::None, false);
    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    builder.symbol("far_external", FAR_SYMBOL_ADDR as *const u8);
    let mut module = JITModule::new(builder);
    let mut sig = module.make_signature();
    sig.returns.push(cranelift_codegen::ir::AbiParam::new(
        module.target_config().pointer_type(),
    ));
    let entry = module.declare_function("entry", Linkage::Local, &sig).unwrap();
    let far = module
        .declare_function("far_external", Linkage::Import, &sig)
        .unwrap();
    let mut ctx = module.make_context();
    ctx.func.signature = sig;
    let far_ref = module.declare_func_in_func(far, &mut ctx.func);
    // Deliberately model the bug class: a far Import whose colocated bit was
    // left true produces a near PC-relative relocation before address fixup.
    ctx.func.dfg.ext_funcs[far_ref].colocated = true;
    let mut fctx = FunctionBuilderContext::new();
    {
        let mut b = FunctionBuilder::new(&mut ctx.func, &mut fctx);
        let block = b.create_block();
        b.switch_to_block(block);
        b.seal_block(block);
        let call = b.ins().call(far_ref, &[]);
        let result = b.inst_results(call)[0];
        b.ins().return_(&[result]);
        b.finalize();
    }
    let compiled = ctx.compile(module.isa(), &mut ControlPlane::default()).unwrap();
    assert!(compiled.buffer.relocs().iter().any(|reloc| {
        matches!(
            reloc.kind,
            cranelift_codegen::binemit::Reloc::X86CallPCRel4
        )
    }));
    // Keep the declaration alive so this remains an actual module-local JIT
    // function shape, even though this diagnostic intentionally stops before
    // relocation resolution.
    let _ = entry;
}
