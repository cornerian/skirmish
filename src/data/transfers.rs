use peppi::model::enums::action_state::{Common, State};

use crate::snapshot::{ActionState, Agent, Input};

pub trait Transferrable {
	fn transfer(&self, input: Input) -> ActionState;
}

// HUBS
pub mod hubs {
	pub mod actionable {
		pub fn grounded(input: crate::snapshot::Input, default: crate::snapshot::ActionState) -> crate::snapshot::ActionState {

		}

		pub fn aerial(input: crate::snapshot::Input, default: crate::snapshot::ActionState) -> crate::snapshot::ActionState {
		}
	}

	pub mod dash {
		pub fn attacks(input: crate::snapshot::Input, default: crate::snapshot::ActionState) -> crate::snapshot::ActionState {
		}
	}

	pub mod grab {
		pub fn throws(input: crate::snapshot::Input, default: crate::snapshot::ActionState) -> crate::snapshot::ActionState {
		}
	}

	pub mod ledge {
		pub fn getups(input: crate::snapshot::Input, default: crate::snapshot::ActionState) -> crate::snapshot::ActionState {
		}
	}
}

// COMMON TRANSFERS

// death transfers
impl Transferrable for Common {
	fn transfer(&self, input: Input, agent: Agent) -> State {
		match *self {
			// --- Death transfers ---
				Common::DEAD_DOWN | Common::DEAD_LEFT | Common::DEAD_RIGHT | Common::DEAD_UP_STAR | Common::DEAD_UP_FALL_HIT_CAMERA | Common::DEAD_UP_FALL_HIT_CAMERA_FLAT => State::Common(Common::REBIRTH),
			// ------

			// --- Respawn transfers ---
				Common::REBIRTH => State::Common(Common::REBIRTH_WAIT),
				Common::REBIRTH_WAIT => hubs::actionable::aerial(input, State::Common(Common::REBIRTH_WAIT)),
			// ------

			// --- Ambulation transfers ---
				Common::WAIT => hubs::actionable::grounded(input, State::Common(Common::WAIT)),
				
				// --- Walking ---
					Common::WALK_SLOW => hubs::actionable::grounded(input, State::Common(Common::WAIT)),
					Common::WALK_MIDDLE => hubs::actionable::grounded(input, State::Common(Common::WAIT)),
					Common::WALK_FAST => hubs::actionable::grounded(input, State::Common(Common::WAIT)),

					Common::TURN => hubs::actionable::grounded(input, State::Common(Common::WAIT)),
				// ------

				// --- Run cycle ---
					Common::DASH => 
						if input.jump { State::Common(Common::KNEE_BEND) }
						else if agent.collisions.wall { State::Common(Common::STOP_WALL) }
						else { hubs::dash::attacks(input, State::Common(Common::RUN)) },
					Common::RUN =>
						if input.jump { State::Common(Common::KNEE_BEND) }
						else if agent.collisions.wall { State::Common(Common::STOP_WALL) }
						else { hubs::dash::attacks(input, State::Common(Common::RUN)) },
					Common::RUN => State::Common(Common::RUN_BRAKE),
					Common::RUN => State::Common(Common::TURN_RUN),

				// ------
			// ------
		}
	}
}