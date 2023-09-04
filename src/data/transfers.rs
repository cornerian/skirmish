use peppi::model::enums::action_state::{Common, State};

use crate::snapshot::Input;

pub trait Transferrable {
	fn transfer(&self, input: Input) -> State;
}

// HUBS
mod hubs {
	mod actionable {
		pub fn grounded(input: crate::snapshot::Input, default: peppi::model::enums::action_state::State) {
		}

		pub fn aerial(input: crate::snapshot::Input, default: peppi::model::enums::action_state::State) {
		}
	}

	mod dash {
		pub fn attacks(input: crate::snapshot::Input) {
		}
	}

	mod grab {
		pub fn throws(input: crate::snapshot::Input) {
		}
	}

	mod ledge {
		pub fn getups(input: crate::snapshot::Input) {
		}
	}
}

// COMMON TRANSFERS

// death transfers
impl Transferrable for Common {
	fn transfer(&self, input: Input) -> State {
		match *self {
			// Death transfers
			Common::DEAD_DOWN | Common::DEAD_LEFT | Common::DEAD_RIGHT | Common::DEAD_UP_STAR | Common::DEAD_UP_FALL_HIT_CAMERA | Common::DEAD_UP_FALL_HIT_CAMERA_FLAT => State::Common(Common::REBIRTH),

			// Respawn transfers
			Common::REBIRTH => State::Common(Common::REBIRTH_WAIT),
			Common::REBIRTH_WAIT => State::Common(Common::WAIT),
		}
	}
}