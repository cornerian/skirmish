//! Fighter action-instance state from `ft_800895E0` and `ft_80089824`.
use super::instance::Counter;
use crate::game::{Action, damage::ProneOrientation};
use serde::Serialize;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct State {
    /// Low byte of the original motion flags (`x2070.x2073`).
    pub motion_identity: u8,
    /// Slippi `post.instance_id` (`x2074.x2088`).
    pub id: u16,
    pending: Vec<u8>,
}

pub fn queue(state: &mut State, identity: u8) {
    state.pending.push(identity);
}

pub fn flush(state: &mut State, counter: &mut Counter) {
    for identity in state.pending.drain(..) {
        change(state.motion_identity, &mut state.id, identity, counter);
        state.motion_identity = identity;
    }
}

pub fn restart(state: &mut State, counter: &mut Counter) {
    state.id = counter.allocate();
}

fn change(previous: u8, id: &mut u16, identity: u8, counter: &mut Counter) {
    if identity == 0 || identity != previous {
        *id = counter.allocate();
    }
}

/// Low bytes of the common motion flags retained by Skirmish's actions. Fox's
/// current neutral-special shell uses the character table's startup identity.
pub fn motion_identity(action: Action, prone: Option<ProneOrientation>, slow_ledge: bool) -> u8 {
    use Action::*;
    match action {
        Walk | Dash => 102,
        Run => 104,
        Turn | RunTurn | Squat | SquatWait | SquatRv => 100,
        Jump => 105,
        JumpAerial => 106,
        Jab => 1,
        Attack12 => 2,
        Attack13 => 3,
        Attack100Start | Attack100Loop | Attack100End => 4,
        AttackS3Hi | AttackS3HiS | AttackS3S | AttackS3LwS | AttackS3Lw => 6,
        AttackHi3 => 7,
        AttackLw3 => 8,
        AttackS4Hi | AttackS4HiS | AttackS4S | AttackS4LwS | AttackS4Lw => 9,
        AttackHi4 => 10,
        AttackLw4 => 11,
        AttackAirN | LandingAirN => 12,
        AttackAirF | LandingAirF => 13,
        AttackAirB | LandingAirB => 14,
        AttackAirHi | LandingAirHi => 15,
        AttackAirLw | LandingAirLw => 16,
        SpecialN | SpecialAirN => 17,
        Pass => 111,
        FlyReflectWall => 107,
        Passive | PassiveStandF | PassiveStandB => 110,
        DownAttack => {
            if prone == Some(ProneOrientation::FaceDown) {
                50
            } else {
                49
            }
        }
        DownBound | DownWait | DownDamage | DownForward | DownBack | DownStand => {
            if prone == Some(ProneOrientation::FaceDown) {
                109
            } else {
                108
            }
        }
        Catch | CatchDash => 51,
        CatchAttack => 52,
        ThrowF => 53,
        ThrowB => 54,
        ThrowHi => 55,
        ThrowLw => 56,
        CliffCatch => 112,
        CliffAttack => {
            if slow_ledge {
                62
            } else {
                63
            }
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action::*;

    #[test]
    fn zero_changes_and_restarts_allocate_but_equal_nonzero_retains() {
        let mut counter = Counter::default();
        let mut state = State::default();
        for identity in [0, 0, 12, 12, 13] {
            queue(&mut state, identity);
        }
        flush(&mut state, &mut counter);
        assert_eq!(
            (state.motion_identity, state.id, counter.next_value()),
            (13, 4, 5)
        );
        restart(&mut state, &mut counter);
        assert_eq!((state.id, counter.next_value()), (5, 6));
    }

    #[test]
    fn native_common_motion_families_share_their_low_flag_byte() {
        let id = |action, prone| motion_identity(action, prone, false);
        for (action, expected) in [
            (Walk, 102),
            (Run, 104),
            (Jump, 105),
            (JumpAerial, 106),
            (Jab, 1),
            (AttackAirN, 12),
            (AttackAirF, 13),
            (AttackAirB, 14),
            (AttackAirHi, 15),
            (AttackAirLw, 16),
            (SpecialN, 17),
            (Pass, 111),
            (FlyReflectWall, 107),
            (Passive, 110),
            (Catch, 51),
            (CatchAttack, 52),
            (ThrowF, 53),
            (ThrowB, 54),
            (ThrowHi, 55),
            (ThrowLw, 56),
            (CliffCatch, 112),
        ] {
            assert_eq!(id(action, None), expected, "{action:?}");
        }
        assert_eq!(id(AttackAirN, None), id(LandingAirN, None));
        assert_eq!(id(Squat, None), id(SquatWait, None));
        assert_eq!(id(SpecialN, None), id(SpecialAirN, None));
        assert_ne!(id(AttackAirF, None), id(AttackAirB, None));
        assert_ne!(
            id(DownBound, Some(ProneOrientation::FaceUp)),
            id(DownBound, Some(ProneOrientation::FaceDown))
        );
        assert_eq!(motion_identity(CliffAttack, None, true), 62);
        assert_eq!(motion_identity(CliffAttack, None, false), 63);
    }
}
