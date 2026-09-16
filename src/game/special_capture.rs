//! Captain Falcon's Falcon Dive capture relation.
//!
//! Native source gives this interaction its own victim motion (`CaptureCaptain`)
//! and does not route it through the ordinary grab pair scheduler. The exact
//! release hit (`ftCo_800DE7C0`, sourced from `xDF4[1]`) and the resource-level
//! XRotN/TransN2 attachment transforms are not present in Skirmish's current
//! data model, so this module records only the source-supported relation and
//! action/freeze lifecycle. It deliberately does not invent damage, knockback,
//! or numeric bone identities.

use super::{Action, Error, Fighter, MatchData, State as MatchState};
use serde::Serialize;

/// Which native side of the relation owns the supported attachment behavior.
///
/// Grounded Dive contact calls the native XRotN-to-TransN2 helper for the
/// holder/victim pair. Airborne contact follows a distinct no-attachment path
/// in the supported model; the missing pose resource is never approximated by
/// a guessed bone index or offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Attachment {
    #[default]
    None,
    GroundedHolderToVictim,
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
    _data: &MatchData,
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

    let attachment = if victim_was_grounded {
        Attachment::GroundedHolderToVictim
    } else {
        Attachment::None
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
    Ok(())
}

/// Process the dedicated catch-to-throw transition. The Captain animation
/// callback enters `SpecialHiThrow`; this function clears the relation before
/// any later generic grab/collision work can observe it. A cleared relation is
/// never released a second time.
pub(crate) fn update_pairs(state: &mut MatchState) -> Result<(), Error> {
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
            release(state, holder, victim);
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
        if state.fighters[holder].action != Action::SpecialHiCatch
            || state.fighters[victim].action != Action::CaptureCaptain
        {
            release(state, holder, victim);
        }
    }
}

fn release(state: &mut MatchState, holder: usize, victim: usize) {
    state.fighters[holder].special_capture = State::default();
    state.fighters[victim].special_capture = State::default();

    // The exact native release hit is intentionally unavailable: xDF4[1]
    // cannot be reconstructed from current resources without inventing its
    // damage/knockback. Returning the victim to ordinary collision keeps the
    // relation finite and makes this gap observable in focused tests.
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
    match (fighter.special_capture.victim, fighter.special_capture.captor) {
        (None, None) => fighter.special_capture.is_empty(),
        (Some(victim), None) => {
            victim == other
                && fighter.action == Action::SpecialHiCatch
                && fighter.grab == super::grab::State::default()
                && partner.special_capture.captor == Some(player)
                && partner.special_capture.victim.is_none()
                && partner.action == Action::CaptureCaptain
                && partner.grab == super::grab::State::default()
                && fighter.special_capture.attachment
                    == partner.special_capture.attachment
        }
        (None, Some(captor)) => {
            captor == other
                && fighter.action == Action::CaptureCaptain
                && fighter.grab == super::grab::State::default()
                && partner.special_capture.victim == Some(player)
                && partner.special_capture.captor.is_none()
                && partner.action == Action::SpecialHiCatch
                && partner.grab == super::grab::State::default()
                && fighter.special_capture.attachment
                    == partner.special_capture.attachment
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

#[cfg(test)]
mod tests {
    use super::{Attachment, State};

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
            attachment: Attachment::None,
        };
        assert_ne!(grounded, airborne);
        assert_eq!(grounded.victim, Some(1));
        assert_eq!(airborne.attachment, Attachment::None);
    }

    #[test]
    fn empty_relation_is_not_a_generic_grab() {
        let state = State::default();
        assert!(state.is_empty());
        assert!(!state.holds_victim());
        assert!(!state.is_captured());
    }
}
