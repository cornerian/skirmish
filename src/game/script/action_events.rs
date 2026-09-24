//! Static action markers and native countdown descriptors.
//!
//! The Python compiler lowers `@hook.marker(name, track, actions)`,
//! `@hook.countdown(field, actions)`, and `clock(field, actions)` into this
//! immutable table at resource registration. The native host schedules marker
//! rising edges on action entry and advances declared state fields in the
//! existing action state. There is no runtime callback registration, recurring
//! timer, or raw animation-track polling surface here.

use super::{LocalState, LocalValue};
use crate::game::{
    Action,
    script::scheduler::{OwnerId, ScheduleError, SchedulerState, TimerId, TimerSpec},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_MARKERS: usize = 64;
pub const MAX_COUNTDOWNS: usize = 32;
pub const MAX_CLOCKS: usize = 32;
pub const MAX_FRAMES: usize = 64;
pub const MAX_MARKER_EDGES: usize = 512;
pub const MAX_ACTIONS_PER_BINDING: usize = 8;
pub const MAX_FIELD_BYTES: usize = 64;
pub const MAX_TRACK_BYTES: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MarkerId(u16);

impl MarkerId {
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkerSpec {
    pub id: MarkerId,
    pub name: String,
    pub track: String,
    pub actions: Vec<Action>,
    pub flags: Vec<bool>,
    pub token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountdownSpec {
    pub field: String,
    pub actions: Vec<Action>,
    pub token: u64,
    #[serde(default)]
    pub phase: CountdownPhase,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountdownPhase {
    Animation,
    #[default]
    Physics,
}

/// A native phase clock declaration. The field remains ordinary typed action
/// state; the host adds exactly one f32 frame per active action phase.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockSpec {
    pub field: String,
    pub actions: Vec<Action>,
}

/// A finite action-frame deadline emitted by `@hook.frame`. Unlike a clock it
/// schedules one native event on action entry and never polls from a callback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameSpec {
    pub action: Action,
    pub frame: u32,
    pub token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionMarker {
    pub id: MarkerId,
    pub name: String,
    pub track: String,
    pub actions: Vec<Action>,
    /// Authored action-frame rising edges only. Consecutive true samples are
    /// intentionally represented by one edge.
    pub true_edges: Vec<u32>,
    pub token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountdownBinding {
    pub field: String,
    pub actions: Vec<Action>,
    pub token: u64,
    pub phase: CountdownPhase,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClockBinding {
    pub field: String,
    pub actions: Vec<Action>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameBinding {
    pub action: Action,
    pub frame: u32,
    pub token: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ActionEventTable {
    markers: Vec<ActionMarker>,
    countdowns: Vec<CountdownBinding>,
    clocks: Vec<ClockBinding>,
    frames: Vec<FrameBinding>,
    /// Registration-time dispatch indexes. Values remain binding indexes so
    /// iteration keeps authored callback order at equal action frames.
    #[serde(skip)]
    marker_actions: BTreeMap<Action, Vec<usize>>,
    #[serde(skip)]
    countdown_actions: BTreeMap<(CountdownPhase, Action), Vec<usize>>,
    #[serde(skip)]
    clock_actions: BTreeMap<Action, Vec<usize>>,
    #[serde(skip)]
    frame_actions: BTreeMap<Action, Vec<usize>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionEventTableWire {
    markers: Vec<ActionMarker>,
    countdowns: Vec<CountdownBinding>,
    clocks: Vec<ClockBinding>,
    frames: Vec<FrameBinding>,
}

impl<'de> Deserialize<'de> for ActionEventTable {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = ActionEventTableWire::deserialize(deserializer)?;
        let mut table = Self {
            markers: wire.markers,
            countdowns: wire.countdowns,
            clocks: wire.clocks,
            frames: wire.frames,
            marker_actions: BTreeMap::new(),
            countdown_actions: BTreeMap::new(),
            clock_actions: BTreeMap::new(),
            frame_actions: BTreeMap::new(),
        };
        table.rebuild_indexes();
        Ok(table)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ActionEventError {
    #[error("action event table has too many markers")]
    TooManyMarkers,
    #[error("action event table has too many countdowns")]
    TooManyCountdowns,
    #[error("action event table has too many clocks")]
    TooManyClocks,
    #[error("marker {0:?} has too many rising edges")]
    TooManyMarkerEdges(String),
    #[error("action event table has too many marker edges")]
    TooManyEdges,
    #[error("marker {0:?} has an empty name")]
    EmptyMarkerName(String),
    #[error("marker {0:?} has a name longer than {MAX_FIELD_BYTES} bytes")]
    MarkerNameTooLong(String),
    #[error("marker {0:?} has a track path longer than {MAX_TRACK_BYTES} bytes")]
    TrackPathTooLong(String),
    #[error("marker {0:?} has no action owner")]
    MarkerWithoutAction(String),
    #[error("marker {0:?} names more than {MAX_ACTIONS_PER_BINDING} action owners")]
    MarkerHasTooManyActions(String),
    #[error("marker {0:?} repeats an action owner")]
    DuplicateMarkerAction(String),
    #[error("marker id {0} is duplicated")]
    DuplicateMarkerId(u16),
    #[error("countdown field cannot be empty")]
    EmptyCountdownField,
    #[error("countdown field {0:?} is longer than {MAX_FIELD_BYTES} bytes")]
    CountdownFieldTooLong(String),
    #[error("countdown field {0:?} is bound more than once")]
    DuplicateCountdownField(String),
    #[error("countdown field {0:?} has no active action")]
    CountdownWithoutAction(String),
    #[error("countdown field {0:?} names more than {MAX_ACTIONS_PER_BINDING} active actions")]
    CountdownHasTooManyActions(String),
    #[error("countdown field {0:?} repeats an action owner")]
    DuplicateCountdownAction(String),
    #[error("clock field cannot be empty")]
    EmptyClockField,
    #[error("clock field {0:?} is longer than {MAX_FIELD_BYTES} bytes")]
    ClockFieldTooLong(String),
    #[error("clock field {0:?} is bound more than once")]
    DuplicateClockField(String),
    #[error("clock field {0:?} has no active action")]
    ClockWithoutAction(String),
    #[error("clock field {0:?} names more than {MAX_ACTIONS_PER_BINDING} active actions")]
    ClockHasTooManyActions(String),
    #[error("clock field {0:?} repeats an action owner")]
    DuplicateClockAction(String),
    #[error("marker/countdown token {0} is duplicated")]
    DuplicateToken(u64),
    #[error("action event table has too many frame deadlines")]
    TooManyFrames,
    #[error("countdown field {0:?} is missing from action state")]
    MissingCountdownField(String),
    #[error("clock field {0:?} is missing from action state")]
    MissingClockField(String),
    #[error("countdown field {0:?} must contain a finite f32 number")]
    InvalidCountdownValue(String),
    #[error("clock field {0:?} must contain a finite f32 number")]
    InvalidClockValue(String),
    #[error("native action marker scheduling failed: {0}")]
    Schedule(#[from] ScheduleError),
}

impl ActionEventTable {
    /// Compile resource tracks and static countdown declarations once at
    /// registration. Inputs are consumed into an immutable bounded table.
    pub fn compile(
        marker_specs: Vec<MarkerSpec>,
        countdown_specs: Vec<CountdownSpec>,
        clock_specs: Vec<ClockSpec>,
    ) -> Result<Self, ActionEventError> {
        Self::compile_with_frames(marker_specs, countdown_specs, clock_specs, Vec::new())
    }

    /// Compile markers, clocks, countdowns, and finite action-frame deadlines
    /// into one immutable event table.
    pub fn compile_with_frames(
        marker_specs: Vec<MarkerSpec>,
        countdown_specs: Vec<CountdownSpec>,
        clock_specs: Vec<ClockSpec>,
        frame_specs: Vec<FrameSpec>,
    ) -> Result<Self, ActionEventError> {
        if marker_specs.len() > MAX_MARKERS {
            return Err(ActionEventError::TooManyMarkers);
        }
        if countdown_specs.len() > MAX_COUNTDOWNS {
            return Err(ActionEventError::TooManyCountdowns);
        }
        if clock_specs.len() > MAX_CLOCKS {
            return Err(ActionEventError::TooManyClocks);
        }
        if frame_specs.len() > MAX_FRAMES {
            return Err(ActionEventError::TooManyFrames);
        }
        let mut tokens = BTreeSet::new();
        let mut marker_ids = BTreeSet::new();
        let mut edge_count = 0;
        let markers = marker_specs
            .into_iter()
            .map(|spec| {
                validate_marker(&spec, &mut tokens, &mut marker_ids)?;
                let true_edges = rising_edges(&spec.flags, &spec.name)?;
                if true_edges.len() > MAX_MARKER_EDGES {
                    return Err(ActionEventError::TooManyMarkerEdges(spec.name));
                }
                edge_count += true_edges.len();
                if edge_count > MAX_MARKER_EDGES {
                    return Err(ActionEventError::TooManyEdges);
                }
                Ok(ActionMarker {
                    id: spec.id,
                    name: spec.name,
                    track: spec.track,
                    actions: spec.actions,
                    true_edges,
                    token: spec.token,
                })
            })
            .collect::<Result<Vec<_>, ActionEventError>>()?;
        let mut fields = BTreeSet::new();
        let countdowns = countdown_specs
            .into_iter()
            .map(|spec| {
                validate_countdown(&spec, &mut tokens, &mut fields)?;
                Ok(CountdownBinding {
                    field: spec.field,
                    actions: spec.actions,
                    token: spec.token,
                    phase: spec.phase,
                })
            })
            .collect::<Result<Vec<_>, ActionEventError>>()?;
        let clocks = clock_specs
            .into_iter()
            .map(|spec| {
                validate_clock(&spec, &mut fields)?;
                Ok(ClockBinding {
                    field: spec.field,
                    actions: spec.actions,
                })
            })
            .collect::<Result<Vec<_>, ActionEventError>>()?;
        let frames = frame_specs
            .into_iter()
            .map(|spec| {
                if !tokens.insert(spec.token) {
                    return Err(ActionEventError::DuplicateToken(spec.token));
                }
                Ok(FrameBinding {
                    action: spec.action,
                    frame: spec.frame,
                    token: spec.token,
                })
            })
            .collect::<Result<Vec<_>, ActionEventError>>()?;
        let mut table = Self {
            markers,
            countdowns,
            clocks,
            frames,
            marker_actions: BTreeMap::new(),
            countdown_actions: BTreeMap::new(),
            clock_actions: BTreeMap::new(),
            frame_actions: BTreeMap::new(),
        };
        table.rebuild_indexes();
        Ok(table)
    }

    pub fn markers(&self) -> &[ActionMarker] {
        &self.markers
    }

    pub fn countdowns(&self) -> &[CountdownBinding] {
        &self.countdowns
    }

    pub fn clocks(&self) -> &[ClockBinding] {
        &self.clocks
    }

    pub fn frames(&self) -> &[FrameBinding] {
        &self.frames
    }

    fn rebuild_indexes(&mut self) {
        self.marker_actions = index_actions(
            self.markers
                .iter()
                .enumerate()
                .map(|(index, marker)| (index, &marker.actions)),
        );
        self.countdown_actions = index_phased_actions(
            self.countdowns
                .iter()
                .enumerate()
                .map(|(index, binding)| (index, binding.phase, &binding.actions)),
        );
        self.clock_actions = index_actions(
            self.clocks
                .iter()
                .enumerate()
                .map(|(index, binding)| (index, &binding.actions)),
        );
        self.frame_actions = index_single_actions(
            self.frames
                .iter()
                .enumerate()
                .map(|(index, frame)| (index, frame.action)),
        );
    }

    /// Native host operation performed once when an action generation starts.
    /// It schedules the table's finite authored rising edges and is
    /// transactional if any individual record exceeds scheduler bounds.
    pub fn schedule_action_markers(
        &self,
        scheduler: &mut SchedulerState,
        owner: OwnerId,
        action: Action,
    ) -> Result<Vec<TimerId>, ActionEventError> {
        let mut candidate = scheduler.clone();
        let mut ids = Vec::new();
        for &marker_index in self.marker_actions.get(&action).into_iter().flatten() {
            let marker = &self.markers[marker_index];
            for &frame in &marker.true_edges {
                ids.push(candidate.schedule(
                    owner,
                    TimerSpec::AtActionFrame { action, frame },
                    marker.token,
                )?);
            }
        }
        *scheduler = candidate;
        Ok(ids)
    }

    /// Schedule finite `@hook.frame` deadlines when an action generation
    /// starts. The caller owns generation cancellation through the scheduler's
    /// action scope.
    pub fn schedule_action_frames(
        &self,
        scheduler: &mut SchedulerState,
        owner: OwnerId,
        action: Action,
    ) -> Result<Vec<TimerId>, ActionEventError> {
        let mut candidate = scheduler.clone();
        let mut ids = Vec::new();
        for &frame_index in self.frame_actions.get(&action).into_iter().flatten() {
            let frame = &self.frames[frame_index];
            ids.push(candidate.schedule(
                owner,
                TimerSpec::AtActionFrame {
                    action,
                    frame: frame.frame,
                },
                frame.token,
            )?);
        }
        *scheduler = candidate;
        Ok(ids)
    }

    /// Native host operation for the action phase. It decrements only active
    /// declared f32 fields and returns each token exactly once when a value
    /// crosses from positive to non-positive. The field remains negative when
    /// the source arithmetic makes it negative.
    pub fn advance_countdowns(
        &self,
        action: Action,
        action_state: &mut LocalState,
    ) -> Result<Vec<u64>, ActionEventError> {
        self.advance_countdowns_at_phase(action, action_state, CountdownPhase::Physics)
    }

    pub fn advance_countdowns_at_phase(
        &self,
        action: Action,
        action_state: &mut LocalState,
        phase: CountdownPhase,
    ) -> Result<Vec<u64>, ActionEventError> {
        let mut tokens = Vec::new();
        for &binding_index in self
            .countdown_actions
            .get(&(phase, action))
            .into_iter()
            .flatten()
        {
            let binding = &self.countdowns[binding_index];
            let value = action_state
                .get_mut(&binding.field)
                .ok_or_else(|| ActionEventError::MissingCountdownField(binding.field.clone()))?;
            let LocalValue::Number(value) = value else {
                return Err(ActionEventError::InvalidCountdownValue(
                    binding.field.clone(),
                ));
            };
            let old = *value as f32;
            if !old.is_finite() {
                return Err(ActionEventError::InvalidCountdownValue(
                    binding.field.clone(),
                ));
            }
            if old > 0.0 {
                let next = old - 1.0;
                *value = f64::from(next);
                if next <= 0.0 {
                    tokens.push(binding.token);
                }
            }
        }
        Ok(tokens)
    }

    /// Native host operation for the action phase. It increments only active
    /// declared f32 fields. There is no event or callback on each increment;
    /// scripts observe the state when a legitimate event handler runs.
    pub fn advance_clocks(
        &self,
        action: Action,
        action_state: &mut LocalState,
    ) -> Result<(), ActionEventError> {
        for &binding_index in self.clock_actions.get(&action).into_iter().flatten() {
            let binding = &self.clocks[binding_index];
            let value = action_state
                .get_mut(&binding.field)
                .ok_or_else(|| ActionEventError::MissingClockField(binding.field.clone()))?;
            let LocalValue::Number(value) = value else {
                return Err(ActionEventError::InvalidClockValue(binding.field.clone()));
            };
            let old = *value as f32;
            if !old.is_finite() {
                return Err(ActionEventError::InvalidClockValue(binding.field.clone()));
            }
            *value = f64::from(old + 1.0_f32);
        }
        Ok(())
    }
}

fn index_actions<'a, I>(entries: I) -> BTreeMap<Action, Vec<usize>>
where
    I: IntoIterator<Item = (usize, &'a Vec<Action>)>,
{
    let mut index = BTreeMap::new();
    for (binding, actions) in entries {
        for &action in actions {
            index.entry(action).or_insert_with(Vec::new).push(binding);
        }
    }
    index
}

fn index_phased_actions<'a, I>(entries: I) -> BTreeMap<(CountdownPhase, Action), Vec<usize>>
where
    I: IntoIterator<Item = (usize, CountdownPhase, &'a Vec<Action>)>,
{
    let mut index = BTreeMap::new();
    for (binding, phase, actions) in entries {
        for &action in actions {
            index
                .entry((phase, action))
                .or_insert_with(Vec::new)
                .push(binding);
        }
    }
    index
}

fn index_single_actions<I>(entries: I) -> BTreeMap<Action, Vec<usize>>
where
    I: IntoIterator<Item = (usize, Action)>,
{
    let mut index = BTreeMap::new();
    for (binding, action) in entries {
        index.entry(action).or_insert_with(Vec::new).push(binding);
    }
    index
}

fn validate_marker(
    spec: &MarkerSpec,
    tokens: &mut BTreeSet<u64>,
    marker_ids: &mut BTreeSet<MarkerId>,
) -> Result<(), ActionEventError> {
    if spec.name.is_empty() {
        return Err(ActionEventError::EmptyMarkerName(spec.name.clone()));
    }
    if spec.name.len() > MAX_FIELD_BYTES {
        return Err(ActionEventError::MarkerNameTooLong(spec.name.clone()));
    }
    if spec.track.len() > MAX_TRACK_BYTES {
        return Err(ActionEventError::TrackPathTooLong(spec.track.clone()));
    }
    validate_actions(&spec.name, &spec.actions, true)?;
    if !marker_ids.insert(spec.id) {
        return Err(ActionEventError::DuplicateMarkerId(spec.id.get()));
    }
    if !tokens.insert(spec.token) {
        return Err(ActionEventError::DuplicateToken(spec.token));
    }
    Ok(())
}

fn validate_countdown(
    spec: &CountdownSpec,
    tokens: &mut BTreeSet<u64>,
    fields: &mut BTreeSet<String>,
) -> Result<(), ActionEventError> {
    if spec.field.is_empty() {
        return Err(ActionEventError::EmptyCountdownField);
    }
    if spec.field.len() > MAX_FIELD_BYTES {
        return Err(ActionEventError::CountdownFieldTooLong(spec.field.clone()));
    }
    if !fields.insert(spec.field.clone()) {
        return Err(ActionEventError::DuplicateCountdownField(
            spec.field.clone(),
        ));
    }
    validate_actions(&spec.field, &spec.actions, false)?;
    if !tokens.insert(spec.token) {
        return Err(ActionEventError::DuplicateToken(spec.token));
    }
    Ok(())
}

fn validate_clock(spec: &ClockSpec, fields: &mut BTreeSet<String>) -> Result<(), ActionEventError> {
    if spec.field.is_empty() {
        return Err(ActionEventError::EmptyClockField);
    }
    if spec.field.len() > MAX_FIELD_BYTES {
        return Err(ActionEventError::ClockFieldTooLong(spec.field.clone()));
    }
    if !fields.insert(spec.field.clone()) {
        return Err(ActionEventError::DuplicateClockField(spec.field.clone()));
    }
    if spec.actions.is_empty() {
        return Err(ActionEventError::ClockWithoutAction(spec.field.clone()));
    }
    if spec.actions.len() > MAX_ACTIONS_PER_BINDING {
        return Err(ActionEventError::ClockHasTooManyActions(spec.field.clone()));
    }
    let mut seen = Vec::new();
    for action in &spec.actions {
        if seen.contains(action) {
            return Err(ActionEventError::DuplicateClockAction(spec.field.clone()));
        }
        seen.push(*action);
    }
    Ok(())
}

fn validate_actions(owner: &str, actions: &[Action], marker: bool) -> Result<(), ActionEventError> {
    if actions.is_empty() {
        return if marker {
            Err(ActionEventError::MarkerWithoutAction(owner.to_owned()))
        } else {
            Err(ActionEventError::CountdownWithoutAction(owner.to_owned()))
        };
    }
    if actions.len() > MAX_ACTIONS_PER_BINDING {
        return if marker {
            Err(ActionEventError::MarkerHasTooManyActions(owner.to_owned()))
        } else {
            Err(ActionEventError::CountdownHasTooManyActions(
                owner.to_owned(),
            ))
        };
    }
    let mut seen = Vec::new();
    for action in actions {
        if seen.contains(action) {
            return if marker {
                Err(ActionEventError::DuplicateMarkerAction(owner.to_owned()))
            } else {
                Err(ActionEventError::DuplicateCountdownAction(owner.to_owned()))
            };
        }
        seen.push(*action);
    }
    Ok(())
}

fn rising_edges(flags: &[bool], name: &str) -> Result<Vec<u32>, ActionEventError> {
    flags
        .iter()
        .enumerate()
        .filter_map(|(index, &value)| (value && (index == 0 || !flags[index - 1])).then_some(index))
        .map(|index| {
            u32::try_from(index).map_err(|_| ActionEventError::TooManyMarkerEdges(name.to_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::script::events::Event;
    use crate::game::script::scheduler::SchedulerState;

    fn marker(flags: Vec<bool>) -> MarkerSpec {
        MarkerSpec {
            id: MarkerId::new(1),
            name: "bound_exit".into(),
            track: "up.bound.exit_flags".into(),
            actions: vec![Action::SpecialHiBound],
            flags,
            token: 11,
        }
    }

    #[test]
    fn compiler_keeps_only_true_rising_edges() {
        let table = ActionEventTable::compile(
            vec![marker(vec![true, true, false, true, true, false])],
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(table.markers()[0].true_edges, vec![0, 3]);
    }

    #[test]
    fn marker_schedule_is_finite_and_action_relative() {
        let table =
            ActionEventTable::compile(vec![marker(vec![false, true, false, true])], vec![], vec![])
                .unwrap();
        let owner = OwnerId::new(2);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        let ids = table
            .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        assert_eq!(ids.len(), 2);
        assert!(scheduler.advance(100).unwrap().is_empty());
        assert_eq!(scheduler.advance_action(owner, 1).unwrap().len(), 1);
        assert_eq!(scheduler.advance_action(owner, 3).unwrap().len(), 1);
    }

    #[test]
    fn same_frame_markers_keep_registration_order() {
        let mut first = marker(vec![false, true]);
        first.token = 11;
        let mut second = marker(vec![false, true]);
        second.id = MarkerId::new(2);
        second.token = 12;
        let table = ActionEventTable::compile(vec![first, second], vec![], vec![]).unwrap();
        let owner = OwnerId::new(4);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        table
            .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        let events = scheduler.advance_action(owner, 1).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                Event::ScheduledDeadline { token: 11, .. },
                Event::ScheduledDeadline { token: 12, .. },
            ]
        ));
    }

    #[test]
    fn animation_marker_precedes_frame_deadline_at_same_action_frame() {
        let table = ActionEventTable::compile_with_frames(
            vec![marker(vec![false, true])],
            vec![],
            vec![],
            vec![FrameSpec {
                action: Action::SpecialHiBound,
                frame: 1,
                token: 41,
            }],
        )
        .unwrap();
        let owner = OwnerId::new(7);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        table
            .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        table
            .schedule_action_frames(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        let events = scheduler.advance_action(owner, 1).unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                Event::ScheduledDeadline { token: 11, .. },
                Event::ScheduledDeadline { token: 41, .. },
            ]
        ));
    }

    #[test]
    fn restarting_an_action_clock_rearms_marker_edges_once() {
        let table =
            ActionEventTable::compile(vec![marker(vec![false, true])], vec![], vec![]).unwrap();
        let owner = OwnerId::new(5);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        table
            .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        assert_eq!(scheduler.advance_action(owner, 1).unwrap().len(), 1);
        scheduler
            .restart_action_clock(owner, Action::SpecialHiBound)
            .unwrap();
        table
            .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        assert_eq!(scheduler.advance_action(owner, 1).unwrap().len(), 1);
        assert!(scheduler.advance_action(owner, 2).unwrap().is_empty());
    }

    #[test]
    fn serde_round_trip_rebuilds_all_dispatch_indexes() {
        let table = ActionEventTable::compile_with_frames(
            vec![marker(vec![false, true])],
            vec![CountdownSpec {
                field: "lag".into(),
                actions: vec![Action::SpecialHiBound],
                token: 21,
                phase: CountdownPhase::Physics,
            }],
            vec![ClockSpec {
                field: "clock".into(),
                actions: vec![Action::SpecialHiBound],
            }],
            vec![FrameSpec {
                action: Action::SpecialHiBound,
                frame: 1,
                token: 41,
            }],
        )
        .unwrap();
        let encoded = serde_json::to_string(&table).unwrap();
        let restored: ActionEventTable = serde_json::from_str(&encoded).unwrap();
        assert_eq!(restored, table);

        let owner = OwnerId::new(6);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        assert_eq!(
            restored
                .schedule_action_markers(&mut scheduler, owner, Action::SpecialHiBound)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            restored
                .schedule_action_frames(&mut scheduler, owner, Action::SpecialHiBound)
                .unwrap()
                .len(),
            1
        );
        let mut state = LocalState::from([
            ("lag".into(), LocalValue::Number(1.0)),
            ("clock".into(), LocalValue::Number(0.0)),
        ]);
        assert_eq!(
            restored
                .advance_countdowns(Action::SpecialHiBound, &mut state)
                .unwrap(),
            vec![21]
        );
        restored
            .advance_clocks(Action::SpecialHiBound, &mut state)
            .unwrap();
        assert_eq!(state["clock"], LocalValue::Number(1.0));
    }

    #[test]
    fn frame_schedule_is_registered_and_action_relative() {
        let table = ActionEventTable::compile_with_frames(
            vec![],
            vec![],
            vec![],
            vec![FrameSpec {
                action: Action::SpecialHiBound,
                frame: 2,
                token: 41,
            }],
        )
        .unwrap();
        assert_eq!(table.frames().len(), 1);
        let owner = OwnerId::new(3);
        let mut scheduler = SchedulerState::new();
        scheduler
            .enter_action(owner, Action::SpecialHiBound)
            .unwrap();
        let ids = table
            .schedule_action_frames(&mut scheduler, owner, Action::SpecialHiBound)
            .unwrap();
        assert_eq!(ids.len(), 1);
        assert!(scheduler.advance_action(owner, 1).unwrap().is_empty());
        let events = scheduler.advance_action(owner, 2).unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            Event::ScheduledDeadline { token: 41, .. }
        ));
    }

    #[test]
    fn countdown_crossing_fires_once_and_preserves_negative_f32_value() {
        let table = ActionEventTable::compile(
            vec![],
            vec![CountdownSpec {
                field: "release_lag".into(),
                actions: vec![Action::SpecialLw, Action::SpecialLwTurn],
                token: 21,
                phase: CountdownPhase::Physics,
            }],
            vec![],
        )
        .unwrap();
        let mut state = LocalState::from([("release_lag".into(), LocalValue::Number(0.5))]);
        assert_eq!(
            table
                .advance_countdowns(Action::SpecialLw, &mut state)
                .unwrap(),
            vec![21]
        );
        assert_eq!(state["release_lag"], LocalValue::Number(-0.5));
        assert!(
            table
                .advance_countdowns(Action::SpecialLwTurn, &mut state)
                .unwrap()
                .is_empty()
        );
        assert_eq!(state["release_lag"], LocalValue::Number(-0.5));
    }

    #[test]
    fn countdown_phase_filters_animation_and_physics_independently() {
        let table = ActionEventTable::compile(
            vec![],
            vec![
                CountdownSpec {
                    field: "travel".into(),
                    actions: vec![Action::SpecialHi],
                    token: 31,
                    phase: CountdownPhase::Animation,
                },
                CountdownSpec {
                    field: "lag".into(),
                    actions: vec![Action::SpecialHi],
                    token: 32,
                    phase: CountdownPhase::Physics,
                },
            ],
            vec![],
        )
        .unwrap();
        let mut state = LocalState::from([
            ("travel".into(), LocalValue::Number(1.0)),
            ("lag".into(), LocalValue::Number(1.0)),
        ]);
        assert_eq!(
            table
                .advance_countdowns_at_phase(
                    Action::SpecialHi,
                    &mut state,
                    CountdownPhase::Animation,
                )
                .unwrap(),
            vec![31]
        );
        assert_eq!(state["travel"], LocalValue::Number(0.0));
        assert_eq!(state["lag"], LocalValue::Number(1.0));
        assert_eq!(
            table
                .advance_countdowns(Action::SpecialHi, &mut state)
                .unwrap(),
            vec![32]
        );
        assert_eq!(state["lag"], LocalValue::Number(0.0));
    }

    #[test]
    fn duplicate_tokens_and_fields_are_rejected_at_registration() {
        let first = CountdownSpec {
            field: "release_lag".into(),
            actions: vec![Action::SpecialLw],
            token: 1,
            phase: CountdownPhase::Physics,
        };
        let second = CountdownSpec {
            field: "release_lag".into(),
            actions: vec![Action::SpecialLwTurn],
            token: 2,
            phase: CountdownPhase::Physics,
        };
        assert!(matches!(
            ActionEventTable::compile(vec![], vec![first, second], vec![]),
            Err(ActionEventError::DuplicateCountdownField(_))
        ));
    }

    #[test]
    fn duplicate_marker_ids_are_rejected_at_registration() {
        let mut first = marker(vec![true]);
        let mut second = marker(vec![false, true]);
        first.token = 31;
        second.token = 32;
        assert!(matches!(
            ActionEventTable::compile(vec![first, second], vec![], vec![]),
            Err(ActionEventError::DuplicateMarkerId(1))
        ));
    }

    #[test]
    fn clock_increments_only_declared_actions_using_f32_arithmetic() {
        let table = ActionEventTable::compile(
            vec![],
            vec![],
            vec![ClockSpec {
                field: "ground_travel_frames".into(),
                actions: vec![Action::SpecialHi],
            }],
        )
        .unwrap();
        let mut state =
            LocalState::from([("ground_travel_frames".into(), LocalValue::Number(0.1))]);
        table
            .advance_clocks(Action::Wait, &mut state)
            .expect("inactive actions do not require a clock field");
        assert_eq!(state["ground_travel_frames"], LocalValue::Number(0.1));
        table.advance_clocks(Action::SpecialHi, &mut state).unwrap();
        assert_eq!(
            state["ground_travel_frames"],
            LocalValue::Number(f64::from(0.1_f32 + 1.0_f32))
        );
    }
}
