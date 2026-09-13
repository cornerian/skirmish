//! Resource and execution limits for the deterministic Luau boundary.
use skirmish::game::script::{
    Error, FighterView, HitView, Hook, LocalValue, MAX_INSTRUCTIONS, MAX_LOCALS, MAX_MEMORY_BYTES,
    MAX_SOURCE_BYTES, Program,
};
use std::collections::BTreeMap;

fn dispatch(source: &str, hook: Hook) -> Result<skirmish::game::script::ScriptResult, Error> {
    Program::new(source)?.dispatch(hook, &FighterView::default(), None, &BTreeMap::new())
}

fn dispatch_hit(source: &str) -> Result<skirmish::game::script::ScriptResult, Error> {
    Program::new(source)?.dispatch(
        Hook::BeforeReceiveHit,
        &FighterView::default(),
        Some(&HitView::default()),
        &BTreeMap::new(),
    )
}

#[test]
fn luau_compound_assignment_and_missing_hooks_are_supported() {
    let program = Program::new(
        "on_frame = function(self) self.locals.count = (self.locals.count or 0); self.locals.count += 1 end",
    )
    .unwrap();
    let first = program
        .dispatch(
            Hook::OnFrame,
            &FighterView::default(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(first.locals.get("count"), Some(&LocalValue::Integer(1)));

    let no_hook = Program::new("-- no lifecycle hooks").unwrap();
    let result = no_hook
        .dispatch(
            Hook::BeforeHit,
            &FighterView::default(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
    assert!(result.commands.is_empty());
    assert!(result.hit.is_none());
}

#[test]
fn invalid_local_values_and_commands_are_rejected() {
    let table_local = dispatch(
        "on_frame = function(self) self.locals.bad = {} end",
        Hook::OnFrame,
    )
    .unwrap_err();
    assert!(matches!(table_local, Error::Invalid(message) if message.contains("scalar")));

    let unknown_command = dispatch(
        "on_frame = function(self) self.commands[1] = { kind = 'teleport' } end",
        Hook::OnFrame,
    )
    .unwrap_err();
    assert!(
        matches!(unknown_command, Error::Invalid(message) if message.contains("unknown script command"))
    );
}

#[test]
fn hit_hook_observes_baseline_patch_before_mutating_it() {
    let result = dispatch_hit(
        "before_receive_hit = function(self, ctx) self.locals.saw_damage = ctx.apply_damage; self.locals.saw_cancelled = ctx.cancelled; ctx.apply_damage = false; ctx.cancelled = true end",
    )
    .unwrap();
    assert_eq!(
        result.locals.get("saw_damage"),
        Some(&LocalValue::Bool(true))
    );
    assert_eq!(
        result.locals.get("saw_cancelled"),
        Some(&LocalValue::Bool(false))
    );
    let hit = result.hit.unwrap();
    assert!(!hit.apply_damage);
    assert!(hit.cancelled);
}

#[test]
fn locals_and_source_are_bounded() {
    let too_many_locals = dispatch(
        &format!(
            "on_frame = function(self) for i = 1, {} do self.locals['k' .. i] = i end end",
            MAX_LOCALS + 1
        ),
        Hook::OnFrame,
    )
    .unwrap_err();
    assert!(matches!(too_many_locals, Error::Invalid(message) if message.contains("local")));

    let source = "x".repeat(MAX_SOURCE_BYTES + 1);
    assert!(matches!(Program::new(source), Err(Error::SourceTooLarge)));
}

#[test]
fn nested_pcall_cannot_escape_the_instruction_budget() {
    let program =
        Program::new("on_frame = function() pcall(function() while true do end end) end").unwrap();
    let error = program
        .dispatch(
            Hook::OnFrame,
            &FighterView::default(),
            None,
            &BTreeMap::new(),
        )
        .unwrap_err();
    assert!(matches!(error, Error::Runtime(message) if message.contains("instruction budget")));
}

#[test]
fn top_level_infinite_hook_is_stopped_by_the_instruction_budget() {
    let error = dispatch("on_frame = function() while true do end end", Hook::OnFrame).unwrap_err();
    assert!(matches!(error, Error::Runtime(message) if message.contains("instruction budget")));
}

#[test]
fn tostring_of_a_table_is_stable_or_rejected() {
    let program = Program::new(
        "on_frame = function(self) local ok, value = pcall(tostring, {}); if ok then self.locals.render = value else self.locals.render = 'error' end end",
    )
    .unwrap();
    let first = program
        .dispatch(
            Hook::OnFrame,
            &FighterView::default(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
    let second = program
        .dispatch(
            Hook::OnFrame,
            &FighterView::default(),
            None,
            &BTreeMap::new(),
        )
        .unwrap();
    assert_eq!(first.locals.get("render"), second.locals.get("render"));
}

#[test]
fn memory_and_randomness_escape_are_rejected() {
    let memory_error = dispatch(
        &format!(
            "on_frame = function(self) self.locals.payload = string.rep('x', {}) end",
            MAX_MEMORY_BYTES
        ),
        Hook::OnFrame,
    )
    .unwrap_err();
    assert!(matches!(
        memory_error,
        Error::Runtime(_) | Error::Invalid(_)
    ));

    let random_error =
        dispatch("on_frame = function() math.random() end", Hook::OnFrame).unwrap_err();
    assert!(matches!(random_error, Error::Runtime(_)));
}

#[test]
fn non_finite_numbers_cannot_enter_hit_or_local_state() {
    let hit_error = dispatch_hit("before_receive_hit = function(self, ctx) ctx.damage = 0 / 0 end")
        .unwrap_err();
    assert!(matches!(hit_error, Error::Invalid(_) | Error::Runtime(_)));

    let local_error = dispatch(
        "on_frame = function(self) self.locals.value = math.huge end",
        Hook::OnFrame,
    )
    .unwrap_err();
    assert!(matches!(local_error, Error::Invalid(_) | Error::Runtime(_)));

    const { assert!(MAX_INSTRUCTIONS > 0) };
}
