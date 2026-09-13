//! Deterministic Luau character-script boundary.
//!
//! Scripts communicate through plain tables and a small validated result. A
//! VM is created for every dispatch, so checkpointing never has to capture a
//! foreign interpreter heap. The simulation owns applying the returned
//! commands and hit patch transactionally.

use mlua::{Function, Lua, LuaSerdeExt, Table, Value, VmState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Hard limits for one script dispatch. These are deliberately fixed until
/// resource validation grows a script-resource section.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_INSTRUCTIONS: u32 = 100_000;
pub const MAX_MEMORY_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_LOCALS: usize = 128;
pub const MAX_LOCAL_KEY_BYTES: usize = 64;
pub const MAX_LOCAL_STRING_BYTES: usize = 256;
pub const MAX_COMMANDS: usize = 16;

/// Selects the bundled source owned by each fighter when its reflector exists.
pub fn bundled_source(specials: Option<&crate::characters::Specials>) -> Option<&'static str> {
    match specials {
        Some(crate::characters::Specials::Fox { down: Some(_), .. }) => {
            Some(include_str!("../../scripts/fighters/fox.luau"))
        }
        Some(crate::characters::Specials::Falco { down: Some(_), .. }) => {
            Some(include_str!("../../scripts/fighters/falco.luau"))
        }
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LocalValue {
    Bool(bool),
    Integer(i64),
    Number(f64),
    String(String),
}

pub type LocalState = BTreeMap<String, LocalValue>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hook {
    BeforeHit,
    BeforeReceiveHit,
    AfterHit,
    AfterReceiveHit,
    OnFrame,
    OnProjectileContact,
}

impl Hook {
    fn name(self) -> &'static str {
        match self {
            Self::BeforeHit => "before_hit",
            Self::BeforeReceiveHit => "before_receive_hit",
            Self::AfterHit => "after_hit",
            Self::AfterReceiveHit => "after_receive_hit",
            Self::OnFrame => "on_frame",
            Self::OnProjectileContact => "on_projectile_contact",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FighterView {
    pub id: u8,
    pub action: String,
    pub action_frame: u32,
    pub velocity: [f32; 2],
    pub grounded: bool,
    pub percent: f32,
    pub hitlag: f32,
    pub hitstun: u32,
    pub flags: BTreeMap<String, bool>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HitView {
    pub frame: u32,
    pub attacker: u8,
    pub defender: u8,
    pub damage: f32,
    pub angle: f32,
    pub base_knockback: u32,
    pub knockback_growth: u32,
    pub knockback: f32,
    pub hitbox_group: u8,
    pub projectile: bool,
    pub max_damage: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitPatch {
    pub cancelled: bool,
    pub damage: f32,
    pub angle: f32,
    pub knockback: f32,
    pub apply_damage: bool,
    pub apply_knockback: bool,
    pub apply_hitlag: bool,
    pub apply_hitstun: bool,
    #[serde(default)]
    pub reflect: bool,
}

impl Default for HitPatch {
    fn default() -> Self {
        Self {
            cancelled: false,
            damage: 0.0,
            angle: 0.0,
            knockback: 0.0,
            apply_damage: true,
            apply_knockback: true,
            apply_hitlag: true,
            apply_hitstun: true,
            reflect: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Command {
    SetAction(String),
    SetVelocity([f32; 2]),
    ApplyHitlag { fighter: u8, frames: u32 },
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScriptResult {
    pub locals: LocalState,
    pub commands: Vec<Command>,
    pub hit: Option<HitPatch>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("script source exceeds {MAX_SOURCE_BYTES} bytes")]
    SourceTooLarge,
    #[error("script has no valid chunk: {0}")]
    Compile(String),
    #[error("script hook failed: {0}")]
    Runtime(String),
    #[error("script returned invalid data: {0}")]
    Invalid(String),
}

impl Program {
    pub fn has_hook(&self, hook: Hook) -> Result<bool, Error> {
        let lua = bounded_vm()?;
        lua.load(&self.source)
            .exec()
            .map_err(|e| Error::Runtime(e.to_string()))?;
        Ok(matches!(
            lua.globals().get::<Value>(hook.name()).map_err(lua_error)?,
            Value::Function(_)
        ))
    }

    pub fn new(source: impl Into<String>) -> Result<Self, Error> {
        let source = source.into();
        if source.len() > MAX_SOURCE_BYTES {
            return Err(Error::SourceTooLarge);
        }
        let lua = Lua::new();
        lua.load(&source)
            .into_function()
            .map_err(|e| Error::Compile(e.to_string()))?;
        Ok(Self { source })
    }

    pub fn dispatch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        locals: &LocalState,
    ) -> Result<ScriptResult, Error> {
        self.dispatch_internal(hook, fighter, hit, locals, None)
    }

    fn dispatch_internal(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: Option<&HitView>,
        locals: &LocalState,
        baseline: Option<&HitPatch>,
    ) -> Result<ScriptResult, Error> {
        if locals.len() > MAX_LOCALS {
            return Err(Error::Invalid("too many local variables".into()));
        }
        let lua = bounded_vm()?;
        let globals = lua.globals();
        lua.load(&self.source)
            .exec()
            .map_err(|e| Error::Runtime(e.to_string()))?;
        let function: Function = match globals.get(hook.name()).map_err(lua_error)? {
            Value::Function(f) => f,
            Value::Nil => {
                return Ok(ScriptResult {
                    locals: locals.clone(),
                    ..Default::default()
                });
            }
            _ => {
                return Err(Error::Invalid(format!(
                    "{} must be a function",
                    hook.name()
                )));
            }
        };
        let self_table = lua.create_table().map_err(lua_error)?;
        self_table.set("id", fighter.id).map_err(lua_error)?;
        self_table
            .set("action", fighter.action.as_str())
            .map_err(lua_error)?;
        self_table
            .set("action_frame", fighter.action_frame)
            .map_err(lua_error)?;
        self_table
            .set(
                "velocity",
                lua.to_value(&fighter.velocity).map_err(lua_error)?,
            )
            .map_err(lua_error)?;
        self_table
            .set("grounded", fighter.grounded)
            .map_err(lua_error)?;
        self_table
            .set("percent", fighter.percent)
            .map_err(lua_error)?;
        self_table
            .set("hitlag", fighter.hitlag)
            .map_err(lua_error)?;
        self_table
            .set("hitstun", fighter.hitstun)
            .map_err(lua_error)?;
        self_table
            .set("flags", lua.to_value(&fighter.flags).map_err(lua_error)?)
            .map_err(lua_error)?;
        self_table
            .set("locals", lua.to_value(locals).map_err(lua_error)?)
            .map_err(lua_error)?;
        let self_commands = lua.create_table().map_err(lua_error)?;
        self_table
            .set("commands", self_commands.clone())
            .map_err(lua_error)?;
        let set_action = lua
            .create_function(|lua, (this, action): (Table, String)| {
                this.set("action", action.clone())?;
                let commands: Table = this.get("commands")?;
                let command = lua.create_table()?;
                command.set("kind", "set_action")?;
                command.set("action", action)?;
                commands.set(commands.raw_len() + 1, command)?;
                Ok(())
            })
            .map_err(lua_error)?;
        self_table
            .set("set_action", set_action)
            .map_err(lua_error)?;
        let set_velocity = lua
            .create_function(|lua, (this, x, y): (Table, f32, f32)| {
                let velocity = [x, y];
                if !velocity.iter().all(|value| value.is_finite()) {
                    return Err(mlua::Error::RuntimeError("non-finite velocity".into()));
                }
                this.set("velocity", velocity)?;
                let commands: Table = this.get("commands")?;
                let command = lua.create_table()?;
                command.set("kind", "set_velocity")?;
                command.set("velocity", velocity)?;
                commands.set(commands.raw_len() + 1, command)
            })
            .map_err(lua_error)?;
        self_table
            .set("set_velocity", set_velocity)
            .map_err(lua_error)?;
        let ctx = lua.create_table().map_err(lua_error)?;
        if let Some(hit) = hit {
            ctx.set("frame", hit.frame).map_err(lua_error)?;
            ctx.set("attacker", hit.attacker).map_err(lua_error)?;
            ctx.set("defender", hit.defender).map_err(lua_error)?;
            ctx.set("damage", hit.damage).map_err(lua_error)?;
            ctx.set("angle", hit.angle).map_err(lua_error)?;
            ctx.set("base_knockback", hit.base_knockback)
                .map_err(lua_error)?;
            ctx.set("knockback_growth", hit.knockback_growth)
                .map_err(lua_error)?;
            ctx.set("knockback", hit.knockback).map_err(lua_error)?;
            ctx.set("hitbox_group", hit.hitbox_group)
                .map_err(lua_error)?;
            ctx.set("projectile", hit.projectile).map_err(lua_error)?;
            ctx.set("max_damage", hit.max_damage).map_err(lua_error)?;
            let patch = baseline.cloned().unwrap_or_default();
            for (name, value) in [
                ("cancelled", patch.cancelled),
                ("apply_damage", patch.apply_damage),
                ("apply_knockback", patch.apply_knockback),
                ("apply_hitlag", patch.apply_hitlag),
                ("apply_hitstun", patch.apply_hitstun),
                ("reflect", patch.reflect),
            ] {
                ctx.set(name, value).map_err(lua_error)?;
            }
            ctx.set("commands", lua.create_table().map_err(lua_error)?)
                .map_err(lua_error)?;
        }
        let returned: Value = function
            .call((self_table.clone(), ctx.clone()))
            .map_err(|e| Error::Runtime(e.to_string()))?;
        parse_result(&lua, returned, &self_table, &ctx, hit.is_some())
    }

    /// Dispatch with an already modified hit baseline. This is used when the
    /// attacker hook runs before the defender hook; defender scripts must see
    /// the attacker's cancellation and resolution gates rather than a stale
    /// copy of the original hit.
    pub fn dispatch_with_patch(
        &self,
        hook: Hook,
        fighter: &FighterView,
        hit: &HitView,
        baseline: &HitPatch,
        locals: &LocalState,
    ) -> Result<ScriptResult, Error> {
        let result = self.dispatch_internal(hook, fighter, Some(hit), locals, Some(baseline))?;
        let Some(mut patch) = result.hit else {
            return Ok(result);
        };
        // A defender may further restrict a hit, but cannot resurrect an
        // attacker cancellation or re-enable a gate the attacker disabled.
        patch.cancelled |= baseline.cancelled;
        patch.apply_damage &= baseline.apply_damage;
        patch.apply_knockback &= baseline.apply_knockback;
        patch.apply_hitlag &= baseline.apply_hitlag;
        patch.apply_hitstun &= baseline.apply_hitstun;
        Ok(ScriptResult {
            hit: Some(patch),
            ..result
        })
    }
}

fn bounded_vm() -> Result<Lua, Error> {
    let lua = Lua::new();
    lua.set_memory_limit(MAX_MEMORY_BYTES)
        .map_err(|e| Error::Runtime(e.to_string()))?;
    let used = std::cell::Cell::new(0u32);
    // Luau exposes an interrupt callback rather than Lua's instruction
    // hook. The callback is periodic, so this is an execution budget,
    // not an exact instruction count.
    lua.set_interrupt(move |_lua| {
        let next = used.get().saturating_add(1);
        used.set(next);
        if next > MAX_INSTRUCTIONS {
            Err(mlua::Error::RuntimeError(
                "instruction budget exceeded".into(),
            ))
        } else {
            Ok(VmState::Continue)
        }
    });
    let globals = lua.globals();
    for name in ["require", "io", "os", "debug"] {
        globals.set(name, Value::Nil).map_err(lua_error)?;
    }
    if let Ok(math) = globals.get::<Table>("math") {
        math.set("random", Value::Nil).map_err(lua_error)?;
        math.set("randomseed", Value::Nil).map_err(lua_error)?;
    }
    // Luau's default tostring(table) includes an allocation address. Keep
    // persisted script state independent of VM allocation layout.
    let stable_tostring = lua
        .create_function(|_, value: Value| match value {
            Value::Nil => Ok("nil".to_owned()),
            Value::Boolean(value) => Ok(value.to_string()),
            Value::Integer(value) => Ok(value.to_string()),
            Value::Number(value) if value.is_finite() => Ok(value.to_string()),
            Value::Number(_) => Err(mlua::Error::RuntimeError(
                "non-finite tostring value".into(),
            )),
            Value::String(value) => Ok(value.to_str()?.to_owned()),
            Value::Table(_) => Ok("table".to_owned()),
            Value::Function(_) => Ok("function".to_owned()),
            Value::Thread(_) => Ok("thread".to_owned()),
            Value::UserData(_) | Value::LightUserData(_) => Ok("userdata".to_owned()),
            Value::Error(_) => Ok("error".to_owned()),
            Value::Vector(_) | Value::Buffer(_) | Value::Other(_) => Ok("value".to_owned()),
        })
        .map_err(lua_error)?;
    globals
        .set("tostring", stable_tostring)
        .map_err(lua_error)?;
    Ok(lua)
}

fn lua_error(error: mlua::Error) -> Error {
    Error::Runtime(error.to_string())
}

fn parse_result(
    lua: &Lua,
    returned: Value,
    self_table: &Table,
    ctx: &Table,
    has_hit: bool,
) -> Result<ScriptResult, Error> {
    let output = match returned {
        Value::Nil => None,
        Value::Table(table) => Some(table),
        _ => return Err(Error::Invalid("hook must return a table or nil".into())),
    };
    let locals_table = match output.as_ref() {
        Some(table) => table
            .get::<Option<Table>>("locals")
            .map_err(lua_error)?
            .unwrap_or(self_table.get::<Table>("locals").map_err(lua_error)?),
        None => self_table.get::<Table>("locals").map_err(lua_error)?,
    };
    let locals = local_state(lua, locals_table)?;
    let mut command_list = parse_commands(self_table.get::<Table>("commands").map_err(lua_error)?)?;
    if let Some(table) = match output.as_ref() {
        Some(output) => output.get::<Option<Table>>("commands").map_err(lua_error)?,
        None => None,
    } {
        command_list.extend(parse_commands(table)?);
    }
    let hit = if has_hit {
        let patch = match output.as_ref() {
            Some(output) => output
                .get::<Option<Table>>("hit")
                .map_err(lua_error)?
                .unwrap_or_else(|| ctx.clone()),
            None => ctx.clone(),
        };
        Some(HitPatch {
            cancelled: patch.get("cancelled").map_err(lua_error)?,
            damage: finite(patch.get("damage").map_err(lua_error)?)?,
            angle: finite(patch.get("angle").map_err(lua_error)?)?,
            knockback: finite(patch.get("knockback").map_err(lua_error)?)?,
            apply_damage: patch.get("apply_damage").map_err(lua_error)?,
            apply_knockback: patch.get("apply_knockback").map_err(lua_error)?,
            apply_hitlag: patch.get("apply_hitlag").map_err(lua_error)?,
            apply_hitstun: patch.get("apply_hitstun").map_err(lua_error)?,
            reflect: patch.get("reflect").unwrap_or(false),
        })
    } else {
        None
    };
    Ok(ScriptResult {
        locals,
        commands: command_list,
        hit,
    })
}

impl Program {
    /// Query the generic projectile-contact policy. Collision and reflection
    /// physics remain engine-owned; scripts return only this disposition bit.
    pub fn projectile_contact(
        &self,
        fighter: &FighterView,
        damage: f32,
        max_damage: i32,
    ) -> Result<bool, Error> {
        let hit = HitView {
            damage,
            max_damage,
            projectile: true,
            ..Default::default()
        };
        Ok(self
            .dispatch(
                Hook::OnProjectileContact,
                fighter,
                Some(&hit),
                &LocalState::new(),
            )?
            .hit
            .is_some_and(|patch| patch.reflect))
    }
}

fn finite(value: f32) -> Result<f32, Error> {
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| Error::Invalid("non-finite number".into()))
}

fn local_state(lua: &Lua, table: Table) -> Result<LocalState, Error> {
    let mut result = LocalState::new();
    for pair in table.pairs::<String, Value>() {
        let (key, value) = pair.map_err(lua_error)?;
        if key.len() > MAX_LOCAL_KEY_BYTES {
            return Err(Error::Invalid("local key is too long".into()));
        }
        if result.len() == MAX_LOCALS {
            return Err(Error::Invalid("too many local variables".into()));
        }
        let value = match value {
            Value::Boolean(value) => LocalValue::Bool(value),
            Value::Integer(value) => LocalValue::Integer(value),
            Value::Number(value) if value.is_finite() => LocalValue::Number(value),
            Value::String(value) => {
                let value = value.to_str().map_err(lua_error)?.to_owned();
                if value.len() > MAX_LOCAL_STRING_BYTES {
                    return Err(Error::Invalid("local string is too long".into()));
                }
                LocalValue::String(value)
            }
            _ => return Err(Error::Invalid(format!("local {key:?} must be a scalar"))),
        };
        result.insert(key, value);
    }
    let _ = lua;
    Ok(result)
}

pub fn validate_program(program: &Program) -> Result<(), Error> {
    Program::new(program.source.clone()).map(|_| ())
}

pub fn validate_state(state: &LocalState) -> Result<(), Error> {
    if state.len() > MAX_LOCALS {
        return Err(Error::Invalid("too many local variables".into()));
    }
    for (key, value) in state {
        if key.is_empty() {
            return Err(Error::Invalid("local key cannot be empty".into()));
        }
        if key.len() > MAX_LOCAL_KEY_BYTES {
            return Err(Error::Invalid("local key is too long".into()));
        }
        if matches!(value, LocalValue::String(value) if value.len() > MAX_LOCAL_STRING_BYTES) {
            return Err(Error::Invalid("local string is too long".into()));
        }
        if matches!(value, LocalValue::Number(value) if !value.is_finite()) {
            return Err(Error::Invalid("non-finite local number".into()));
        }
    }
    Ok(())
}

/// Apply validated generic commands after a successful hook.
pub(crate) fn apply_commands(
    state: &mut super::State,
    actor: usize,
    commands: &[Command],
) -> Result<(), super::Error> {
    if actor >= state.fighters.len() {
        return Err(super::Error::Data("invalid script actor".into()));
    }
    for command in commands {
        match command {
            Command::SetAction(name) => {
                let action = parse_action(name).ok_or_else(|| {
                    super::Error::Data(format!("unsupported script action {name:?}"))
                })?;
                super::simulation::enter(&mut state.fighters[actor], action);
            }
            Command::SetVelocity(velocity) => {
                if !velocity.iter().all(|value| value.is_finite()) {
                    return Err(super::Error::NonFinite);
                }
                state.fighters[actor].velocity = *velocity;
            }
            Command::ApplyHitlag { fighter, frames } => {
                let target = state
                    .fighters
                    .get_mut(usize::from(*fighter))
                    .ok_or_else(|| super::Error::Data("invalid script hitlag target".into()))?;
                target.hitlag = target.hitlag.max(*frames as f32);
            }
        }
    }
    Ok(())
}

fn parse_action(name: &str) -> Option<super::Action> {
    Some(match name {
        "wait" => super::Action::Wait,
        "walk" => super::Action::Walk,
        "dash" => super::Action::Dash,
        "run" => super::Action::Run,
        "fall" => super::Action::Fall,
        "jump" => super::Action::Jump,
        "jump_aerial" => super::Action::JumpAerial,
        "guard_on" => super::Action::GuardOn,
        "guard" => super::Action::Guard,
        "guard_off" => super::Action::GuardOff,
        "escape_air" => super::Action::EscapeAir,
        "landing" => super::Action::Landing,
        "damage" => super::Action::Damage,
        _ => return None,
    })
}

fn parse_commands(table: Table) -> Result<Vec<Command>, Error> {
    let mut result = Vec::new();
    for pair in table.sequence_values::<Table>() {
        if result.len() >= MAX_COMMANDS {
            return Err(Error::Invalid("too many script commands".into()));
        }
        let command = pair.map_err(lua_error)?;
        let kind: String = command.get("kind").map_err(lua_error)?;
        result.push(match kind.as_str() {
            "set_action" => Command::SetAction(command.get("action").map_err(lua_error)?),
            "set_velocity" => {
                let velocity: [f32; 2] = command.get("velocity").map_err(lua_error)?;
                if !velocity.iter().all(|value| value.is_finite()) {
                    return Err(Error::Invalid("non-finite velocity command".into()));
                }
                Command::SetVelocity(velocity)
            }
            "apply_hitlag" => Command::ApplyHitlag {
                fighter: command.get("fighter").map_err(lua_error)?,
                frames: command.get("frames").map_err(lua_error)?,
            },
            _ => return Err(Error::Invalid(format!("unknown script command {kind:?}"))),
        });
    }
    Ok(result)
}
