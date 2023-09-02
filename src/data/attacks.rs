use phf;

use peppi::model::enums::{action_state::State, character};

use crate::snapshot::Attack;

type CharacterAttacks = phf::Map<&'static State, Attack>;

pub static CAPTAIN_FALCON: CharacterAttacks = phf::phf_map! {
	State::ATTACK_11 => Attack {
		code: action_state::State::ATTACK_11,
		damage: 25.0,
		angle: 361.0,
		knockback: 0.0,
		knockback_scaling: 0.0,
		blockable: true,
		shield_damage: 0.0,
		induces: action_state::State::DAMAGE_FALL,
	},
	State::ATTACK_12 => Attack {
		code: action_state::State::ATTACK_12,
		damage: 25.0,
		angle: 361.0,
		knockback: 0.0,
		knockback_scaling: 0.0,
		blockable: true,
		shield_damage: 0.0,
		induces: action_state::State::DAMAGE_FALL,
	},
};

pub static ATTACKS: phf::Map<&'static character::Internal, CharacterAttacks> = phf::phf_map! {
    Internal::CAPTAIN_FALCON => &CAPTAIN_FALCON,
};