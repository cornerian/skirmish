//! Captain Falcon's Falcon Dive capture relation.
//!
//! Native source gives this interaction its own victim motion (`CaptureCaptain`)
//! and does not route it through the ordinary grab pair scheduler. Its
//! attachment and release hit are consumed from the Captain resource tree;
//! older fixtures without that optional tree retain the relation-only
//! lifecycle until their resources are upgraded.

use super::{
    Action, Error, Fighter, MatchData, State as MatchState, data::Hitbox,
    script::resources::CaptainDiveCapture,
};
use serde::Serialize;

/// Which native side of the relation owns the supported attachment behavior.
///
/// Grounded Dive contact calls the native XRotN-to-TransN2 helper for the
/// holder/victim pair. Airborne contact uses the authored holder/victim
/// anchors to keep the victim attached to the Captain.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attachment {
    #[default]
    None,
    GroundedHolderToVictim,
    AirborneVictimToHolder,
}

/// Checkpointed Captain Falcon Dive relation state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    /// Set only on the holder side of the relation.
    pub victim: Option<usize>,
    /// Set only on the captured-victim side of the relation.
    pub captor: Option<usize>,
    /// The same mode is mirrored on both sides for strict validation.
    pub attachment: Attachment,
}

impl State {
    pub(crate) fn is_empty(self) -> bool {
        self == Self::default()
    }

    pub(crate) fn holds_victim(self) -> bool {
        self.victim.is_some() && self.captor.is_none()
    }

    pub(crate) fn is_captured(self) -> bool {
        self.captor.is_some() && self.victim.is_none()
    }
}

/// Captain's native capture callback, entered after the attacker's
/// `before_hit` hook changed `SpecialHi`/`SpecialAirHi` to `SpecialHiCatch`.
/// This is intentionally not exposed to generic grab scanning or pair logic.
pub(crate) fn capture(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
    victim_was_grounded: bool,
) -> Result<(), Error> {
    if holder > 1 || victim > 1 || holder == victim {
        return Err(Error::Physics("invalid Falcon Dive capture pair".into()));
    }
    if state.fighters[holder].action != Action::SpecialHiCatch
        || !state.fighters[holder].special_capture.is_empty()
        || !state.fighters[victim].special_capture.is_empty()
        || state.fighters[holder].grab != super::grab::State::default()
        || state.fighters[victim].grab != super::grab::State::default()
    {
        return Err(Error::Physics(
            "Captain capture requires an unpaired SpecialHiCatch holder".into(),
        ));
    }

    // The capture callback can carry a percent-only damage payload. Resolve
    // its stale value before mutating the relation so malformed rules cannot
    // leave a half-entered pair behind. Unlike an ordinary hit, this event
    // deliberately does not touch hitlag, knockback, or the victim action.
    let capture_damage = captain_dive_capture(data, holder)
        .and_then(|capture| capture.damage)
        .map(|damage| {
            crate::game::staling::hit(
                &state.fighters[holder].staling,
                damage,
                data.rules.staling.as_ref(),
            )
        })
        .transpose()?;

    let attachment = if victim_was_grounded {
        Attachment::GroundedHolderToVictim
    } else {
        Attachment::AirborneVictimToHolder
    };
    state.fighters[holder].special_capture = State {
        victim: Some(victim),
        captor: None,
        attachment,
    };
    state.fighters[victim].special_capture = State {
        victim: None,
        captor: Some(holder),
        attachment,
    };

    // ftCa_SpecialLw_800E5128 and ftCo_8009CA0C clear both sides' transient
    // velocity/knockback before the dedicated victim motion starts. The
    // holder/victim pose snap itself is intentionally unsupported until the
    // source-compatible XRotN/TransN2 resources exist.
    for player in [holder, victim] {
        let fighter = &mut state.fighters[player];
        fighter.velocity = [0.0; 2];
        fighter.knockback = [0.0; 2];
        fighter.ground_knockback = 0.0;
        fighter.ground_velocity = 0.0;
    }
    state.fighters[victim].facing = -state.fighters[holder].facing;
    crate::game::simulation::enter(&mut state.fighters[victim], Action::CaptureCaptain);
    if let Some(staled) = capture_damage {
        state.fighters[victim].percent =
            (state.fighters[victim].percent + staled.damage).min(999.0);
        if data.rules.staling.is_some() {
            state.fighters[holder]
                .staling
                .queue
                .record(staled.identity, false);
        }
    }
    Ok(())
}

/// Process the dedicated catch-to-throw transition. The Captain animation
/// callback enters `SpecialHiThrow`; this function clears the relation before
/// any later generic grab/collision work can observe it. A cleared relation is
/// never released a second time.
pub(crate) fn update_pairs(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].special_capture.victim else {
            continue;
        };
        if victim != 1 - holder
            || state.fighters[victim].special_capture.captor != Some(holder)
            || state.fighters[victim].action != Action::CaptureCaptain
        {
            return Err(Error::Physics("invalid Captain capture relation".into()));
        }
        if state.fighters[holder].action == Action::SpecialHiThrow {
            let release_frame =
                captain_dive_capture(data, holder).map_or(0, |capture| capture.throw.release_frame);
            if state.fighters[holder].action_frame >= release_frame {
                release(data, state, holder, victim)?;
            }
        } else if state.fighters[holder].action != Action::SpecialHiCatch {
            return Err(Error::Physics(
                "Captain capture holder left catch without release".into(),
            ));
        }
    }
    Ok(())
}

/// Break a relation if an external lifecycle (death, forced action change,
/// or a restored older checkpoint) leaves the dedicated catch state.
pub(crate) fn release_broken_pairs(state: &mut MatchState) {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].special_capture.victim else {
            continue;
        };
        if state.fighters[holder].action == Action::SpecialHiThrow
            && state.fighters[victim].action == Action::CaptureCaptain
        {
            // The normal path is consumed by `update_pairs`, which performs
            // the one Catch -> Throw detach. Keep this fallback from clearing
            // the relation before that dedicated transition is observed.
            continue;
        }
        if state.fighters[holder].action != Action::SpecialHiCatch
            || state.fighters[victim].action != Action::CaptureCaptain
        {
            release_without_hit(state, holder, victim);
        }
    }
}

/// Tear down a dedicated relation before a fighter is removed from play.
/// Unlike `release_broken_pairs`, this is unconditional: a stock loss must
/// not leave a Catch -> CaptureCaptain pair checkpointed across the death or
/// respawn transition.
pub(crate) fn break_for_player(state: &mut MatchState, player: usize) {
    if player > 1 {
        return;
    }
    if let Some(victim) = state.fighters[player].special_capture.victim
        && victim <= 1
    {
        release_without_hit(state, player, victim);
    } else if let Some(holder) = state.fighters[player].special_capture.captor
        && holder <= 1
    {
        release_without_hit(state, holder, player);
    }
}

pub(crate) fn attach_all(data: &MatchData, state: &mut MatchState) -> Result<(), Error> {
    for holder in 0..2 {
        let Some(victim) = state.fighters[holder].special_capture.victim else {
            continue;
        };
        let Some(capture) = captain_dive_capture(data, holder) else {
            continue;
        };
        match state.fighters[holder].special_capture.attachment {
            Attachment::GroundedHolderToVictim => crate::game::grab::attach_holder_points(
                data,
                state,
                holder,
                victim,
                capture.attachment,
            )?,
            Attachment::AirborneVictimToHolder => crate::game::grab::attach_points(
                data,
                state,
                holder,
                victim,
                capture.attachment,
                false,
            )?,
            Attachment::None => {}
        }
    }
    Ok(())
}

fn captain_dive_capture(data: &MatchData, holder: usize) -> Option<&CaptainDiveCapture> {
    data.fighters
        .get(holder)?
        .specials
        .as_ref()?
        .captain_dive_capture()
}

fn release(
    data: &MatchData,
    state: &mut MatchState,
    holder: usize,
    victim: usize,
) -> Result<(), Error> {
    let resource = captain_dive_capture(data, holder).cloned();
    state.fighters[holder].special_capture = State::default();
    state.fighters[victim].special_capture = State::default();

    if let Some(capture) = resource {
        let hit = capture.throw.hit;
        let hitbox = release_hitbox(hit)?;
        let staled = crate::game::staling::hit(
            &state.fighters[holder].staling,
            hit.damage,
            data.rules.staling.as_ref(),
        )?;
        let accepted = crate::game::hit_resolution::apply_hit(
            data,
            state,
            holder,
            &hitbox,
            staled,
            crate::fighter::damage::HurtHeight::Middle,
            crate::game::hit_resolution::HitDirection::Throw,
            false,
        )?;
        if accepted && data.rules.staling.is_some() {
            state.fighters[holder]
                .staling
                .queue
                .record(staled.identity, false);
        }
        return Ok(());
    }

    release_without_hit(state, holder, victim);
    Ok(())
}

fn release_hitbox(hit: super::script::resources::CaptainDiveHit) -> Result<Hitbox, Error> {
    let element = hit.semantic_element().map_err(Error::Data)?;
    Ok(Hitbox {
        clank: false,
        rebound: false,
        element,
        group: 0,
        bone: 0,
        center: [0.0; 3],
        radius: 0.0,
        damage: hit.damage,
        shield_damage: 0,
        angle_degrees: hit.angle_raw as f32,
        growth: hit.growth,
        fixed: hit.fixed,
        base: hit.base,
    })
}

fn release_without_hit(state: &mut MatchState, holder: usize, victim: usize) {
    state.fighters[holder].special_capture = State::default();
    state.fighters[victim].special_capture = State::default();

    let action = if state.fighters[victim].grounded {
        Action::Wait
    } else {
        Action::Fall
    };
    crate::game::simulation::enter(&mut state.fighters[victim], action);
}

pub(crate) fn valid_relationship(fighters: &[Fighter; 2], player: usize) -> bool {
    if player > 1 {
        return false;
    }
    let other = 1 - player;
    let fighter = &fighters[player];
    let partner = &fighters[other];
    match (
        fighter.special_capture.victim,
        fighter.special_capture.captor,
    ) {
        (None, None) => fighter.special_capture.is_empty(),
        (Some(victim), None) => {
            victim == other
                && fighter.action == Action::SpecialHiCatch
                && fighter.grab == super::grab::State::default()
                && partner.special_capture.captor == Some(player)
                && partner.special_capture.victim.is_none()
                && partner.action == Action::CaptureCaptain
                && partner.grab == super::grab::State::default()
                && fighter.special_capture.attachment == partner.special_capture.attachment
        }
        (None, Some(captor)) => {
            captor == other
                && fighter.action == Action::CaptureCaptain
                && fighter.grab == super::grab::State::default()
                && partner.special_capture.victim == Some(player)
                && partner.special_capture.captor.is_none()
                && partner.action == Action::SpecialHiCatch
                && partner.grab == super::grab::State::default()
                && fighter.special_capture.attachment == partner.special_capture.attachment
        }
        (Some(_), Some(_)) => false,
    }
}

pub(crate) fn holds_victim(fighter: &Fighter) -> bool {
    fighter.special_capture.holds_victim()
}

pub(crate) fn is_captured(fighter: &Fighter) -> bool {
    fighter.special_capture.is_captured()
}

/// Captain's dedicated path has no meaningful fallback constants. A Captain
/// resource must therefore carry the semantic capture payload before a match
/// can be constructed; other characters may omit the optional tree.
pub(crate) fn validate_resource(data: &super::data::FighterData) -> Result<(), Error> {
    let Some(specials) = &data.specials else {
        return Ok(());
    };
    if specials.character_key() != "captain-falcon" {
        return Ok(());
    }
    let capture = specials.captain_dive_capture().ok_or_else(|| {
        Error::Data("Captain Falcon specials.up.capture resource is required".into())
    })?;
    let attachment = capture.attachment;
    if attachment.holder_bone >= data.bones.len()
        || attachment.victim_bone >= data.bones.len()
        || attachment
            .holder_point
            .into_iter()
            .chain(attachment.victim_point)
            .any(|value| !value.is_finite() || value.abs() > 1_000_000.0)
    {
        return Err(Error::Data(
            "invalid Captain Falcon Dive attachment resource".into(),
        ));
    }
    let hit = capture.throw.hit;
    if capture.throw.release_frame >= 1_000_000
        || capture.damage.is_some_and(|damage| damage > 999)
        || hit.damage > 999
        || hit.angle_raw > 511
        || hit.growth > 1000
        || hit.fixed > 1000
        || hit.base > 1000
        || hit.semantic_element().is_err()
    {
        return Err(Error::Data(
            "invalid Captain Falcon Dive release resource".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        Attachment, State, break_for_player, capture, release_hitbox, update_pairs,
        valid_relationship,
    };
    use crate::game::data::HitElement;
    use crate::game::{Action, Match};

    fn fixture() -> (Match, crate::game::data::MatchData) {
        let data: crate::game::data::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration game fixture");
        let match_ = Match::new(data.clone(), 1).expect("synthetic match");
        (match_, data)
    }

    fn resource_fixture() -> (Match, crate::game::data::MatchData) {
        resource_fixture_with_damage(None)
    }

    fn capture_damage_fixture() -> (Match, crate::game::data::MatchData) {
        resource_fixture_with_damage(Some(5))
    }

    fn resource_fixture_with_damage(damage: Option<u32>) -> (Match, crate::game::data::MatchData) {
        let mut data: crate::game::data::MatchData = serde_json::from_str(include_str!(
            "../../tests/fixtures/game/integration-match.json"
        ))
        .expect("integration game fixture");
        let mut capture = serde_json::json!({
            "attachment": {
                "holder_bone": 0,
                "holder_point": [0.0, 0.0, 0.0],
                "victim_bone": 0,
                "victim_point": [0.0, 0.0, 0.0]
            },
            "throw": {
                "release_frame": 0,
                "hit": {
                    "damage": 12,
                    "angle_raw": 361,
                    "growth": 82,
                    "fixed": 0,
                    "base": 40,
                    "element": 1
                }
            }
        });
        if let Some(damage) = damage {
            capture["damage"] = serde_json::json!(damage);
        }
        data.fighters[0].specials = Some(
            serde_json::from_value(serde_json::json!({
                "character": "synthetic",
                "up": {
                    "capture": capture
                }
            }))
            .expect("Captain Dive resource"),
        );
        let match_ = Match::new(data.clone(), 1).expect("synthetic match");
        (match_, data)
    }

    #[test]
    fn relation_modes_are_checkpoint_stable_and_do_not_encode_bones() {
        let grounded = State {
            victim: Some(1),
            captor: None,
            attachment: Attachment::GroundedHolderToVictim,
        };
        let airborne = State {
            victim: Some(1),
            captor: None,
            attachment: Attachment::AirborneVictimToHolder,
        };
        assert_ne!(grounded, airborne);
        assert_eq!(grounded.victim, Some(1));
        assert_eq!(airborne.attachment, Attachment::AirborneVictimToHolder);
    }

    #[test]
    fn empty_relation_is_not_a_generic_grab() {
        let state = State::default();
        assert!(state.is_empty());
        assert!(!state.holds_victim());
        assert!(!state.is_captured());
    }

    #[test]
    fn capture_and_throw_release_are_one_dedicated_pair_lifecycle() {
        let (match_, data) = fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].grounded = true;
        let positions = state.fighters.each_ref().map(|fighter| fighter.position);

        capture(&data, &mut state, 0, 1, true).expect("grounded Dive capture");
        assert_eq!(
            state.fighters[0].special_capture.attachment,
            Attachment::GroundedHolderToVictim
        );
        assert_eq!(state.fighters[1].special_capture.captor, Some(0));
        assert_eq!(state.fighters[1].action, Action::CaptureCaptain);
        assert_eq!(
            state.fighters.each_ref().map(|fighter| fighter.position),
            positions
        );
        assert!(valid_relationship(&state.fighters, 0));
        assert!(valid_relationship(&state.fighters, 1));

        state.fighters[0].action = Action::SpecialHiThrow;
        update_pairs(&data, &mut state).expect("Catch -> Throw release");
        assert_eq!(state.fighters[0].special_capture, State::default());
        assert_eq!(state.fighters[1].special_capture, State::default());
        assert_eq!(state.fighters[1].action, Action::Wait);
        let released = state.fighters[1].action_frame;
        update_pairs(&data, &mut state).expect("released relation is inert");
        assert_eq!(state.fighters[1].action_frame, released);
        assert!(valid_relationship(&state.fighters, 0));
        assert!(valid_relationship(&state.fighters, 1));
    }

    #[test]
    fn capture_damage_adds_percent_without_leaving_capture_captain() {
        let (match_, data) = capture_damage_fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].percent = 17.0;
        state.fighters[1].grounded = false;
        state.fighters[0].hitlag = 0.0;
        state.fighters[1].hitlag = 0.0;

        capture(&data, &mut state, 0, 1, false).expect("Dive capture");

        assert_eq!(state.fighters[1].percent, 22.0);
        assert_eq!(state.fighters[1].action, Action::CaptureCaptain);
        assert_eq!(state.fighters[0].hitlag, 0.0);
        assert_eq!(state.fighters[1].hitlag, 0.0);
        assert_eq!(state.fighters[0].knockback, [0.0; 2]);
        assert_eq!(state.fighters[1].knockback, [0.0; 2]);
    }

    #[test]
    fn airborne_capture_records_the_anchor_attachment_mode() {
        let (match_, data) = fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].grounded = false;

        capture(&data, &mut state, 0, 1, false).expect("airborne Dive capture");
        assert_eq!(
            state.fighters[0].special_capture.attachment,
            Attachment::AirborneVictimToHolder
        );
        assert_eq!(
            state.fighters[1].special_capture.attachment,
            Attachment::AirborneVictimToHolder
        );
        assert!(valid_relationship(&state.fighters, 0));
        assert!(valid_relationship(&state.fighters, 1));
    }

    #[test]
    fn grounded_capture_moves_the_holder_with_the_victim_root() {
        let (match_, data) = resource_fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].grounded = true;
        state.fighters[0].position = [-4.0, 0.0];
        state.fighters[1].position = [3.0, 0.0];
        capture(&data, &mut state, 0, 1, true).expect("grounded Dive capture");
        let victim_position = state.fighters[1].position;

        super::attach_all(&data, &mut state).expect("grounded Dive attachment");

        assert_eq!(state.fighters[0].position, victim_position);
        assert_eq!(state.fighters[1].position, victim_position);
        assert_eq!(state.fighters[1].action, Action::CaptureCaptain);
    }

    #[test]
    fn airborne_capture_moves_the_victim_with_the_holder_anchors() {
        let (match_, data) = resource_fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].grounded = false;
        state.fighters[0].position = [-4.0, 0.0];
        state.fighters[1].position = [3.0, 0.0];
        capture(&data, &mut state, 0, 1, false).expect("airborne Dive capture");

        super::attach_all(&data, &mut state).expect("airborne Dive attachment");

        assert_eq!(state.fighters[1].position, state.fighters[0].position);
        assert_eq!(state.fighters[1].action, Action::CaptureCaptain);
    }

    #[test]
    fn resource_release_applies_the_exported_hit_once_through_damage_resolution() {
        let (match_, data) = resource_fixture();
        let hit = data.fighters[0]
            .specials
            .as_ref()
            .and_then(|specials| specials.captain_dive_capture())
            .expect("Captain Dive resource")
            .throw
            .hit;
        let hitbox = release_hitbox(hit).expect("supported release hit element");
        assert_eq!(hitbox.damage, 12);
        assert_eq!(hitbox.angle_degrees, 361.0);
        assert_eq!(hitbox.growth, 82);
        assert_eq!(hitbox.fixed, 0);
        assert_eq!(hitbox.base, 40);
        assert_eq!(hitbox.element, HitElement::Fire);

        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].action = Action::Fall;
        state.fighters[1].grounded = false;
        capture(&data, &mut state, 0, 1, false).expect("Dive capture");
        state.fighters[0].action = Action::SpecialHiThrow;

        update_pairs(&data, &mut state).expect("resource release");
        assert!(state.fighters[0].special_capture.is_empty());
        assert!(state.fighters[1].special_capture.is_empty());
        assert_eq!(state.fighters[1].percent, 12.0);
        let action = state.fighters[1].action;
        update_pairs(&data, &mut state).expect("released relation is inert");
        assert_eq!(state.fighters[1].percent, 12.0);
        assert_eq!(state.fighters[1].action, action);
    }

    #[test]
    fn release_rejects_unmapped_native_hit_elements() {
        let (_, data) = resource_fixture();
        let hit = data.fighters[0]
            .specials
            .as_ref()
            .and_then(|specials| specials.captain_dive_capture())
            .expect("Captain Dive resource")
            .throw
            .hit;
        let mut unsupported = hit;
        unsupported.element = 2;
        assert!(release_hitbox(unsupported).is_err());
    }

    #[test]
    fn stock_loss_tears_down_either_side_without_release_hit() {
        let (match_, data) = fixture();
        let mut state = match_.state().clone();
        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].grounded = false;
        capture(&data, &mut state, 0, 1, false).expect("airborne Dive capture");

        break_for_player(&mut state, 1);
        assert!(state.fighters[0].special_capture.is_empty());
        assert!(state.fighters[1].special_capture.is_empty());
        assert_eq!(state.fighters[1].action, Action::Fall);

        state.fighters[0].action = Action::SpecialHiCatch;
        state.fighters[1].grounded = true;
        capture(&data, &mut state, 0, 1, true).expect("grounded Dive capture");
        break_for_player(&mut state, 0);
        assert!(state.fighters[0].special_capture.is_empty());
        assert!(state.fighters[1].special_capture.is_empty());
        assert_eq!(state.fighters[1].action, Action::Wait);
    }
}
