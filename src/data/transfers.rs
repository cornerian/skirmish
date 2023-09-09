use peppi::model::enums::action_state::{Common, State};

use crate::snapshot::{ActionState, Agent, Input};

// We call the transfer function on the current state either when its counter is up OR 
// on every frame ONLY if the state is interruptible (which all states with infinite counters are)
pub trait Transferrable {
	fn transfer(&self, input: Input, agent: Agent) -> ActionState;
}

// COMMON TRANSFERS
impl Transferrable for Common {
	fn transfer(&self, input: Input, agent: Agent) -> State {
		macro_rules! transfers {
			() => { State::Common(*self) }; // base case to end recursion

			// munch the "condition" syntax
			({ $cond:expr => $res:path } $(, $rest:tt)*) => {
				if $cond {
					State::Common($res)
				} else {
					transfers!($($rest),*)
				}
			};

			// munch the "interrupt" syntax
			({ interrupt!($( $default:ident )::+) } $(, $rest:tt)*) => {{
				let res = interrupts!($( $default )::+);
				if res != State::Common(*self) {
					res
				} else {
					transfers!($($rest),*)
				}
			}};

			// munch the "over" syntax
			({ over!($res:path) } $(, $rest:tt)*) => {
				if agent.state.action.over() {
					State::Common($res)
				} else {
					transfers!($($rest),*)
				}
			};
		}

		macro_rules! interrupts {
			(actionable::grounded) => { transfers! {
				{ input.b => Common::DASH },
				{ input.trigger > 0. => Common::DASH }
			} };

			(actionable::aerial) => { transfers! {
				{ input.b => Common::DASH },
				{ input.trigger > 0. => Common::DASH }
			} };

			(dash::attacks) => { transfers! {
				{ input.a => Common::DASH }
			} };
		}

		match *self {
			// --- Death transfers ---
				Common::DEAD_DOWN | Common::DEAD_LEFT | Common::DEAD_RIGHT | Common::DEAD_UP_STAR | Common::DEAD_UP_FALL_HIT_CAMERA | Common::DEAD_UP_FALL_HIT_CAMERA_FLAT => transfers! {{ over!(Common::REBIRTH) }},
			// ------

			// --- Respawn transfers ---
				Common::REBIRTH => transfers! {{ over!(Common::REBIRTH_WAIT) }},
				Common::REBIRTH_WAIT => transfers! {{ interrupt!(actionable::aerial) }},
			// ------

			// --- Ambulation transfers ---
				Common::WAIT => transfers! {{ interrupt!(actionable::grounded) }},
				
				// --- Walking ---
					Common::WALK_SLOW => transfers! {{ interrupt!(actionable::grounded) }},
					Common::WALK_MIDDLE => transfers! {{ interrupt!(actionable::grounded) }},
					Common::WALK_FAST => transfers! {{ interrupt!(actionable::grounded) }},

					Common::TURN => transfers! { { interrupt!(actionable::grounded) }, { over!(Common::WAIT) } },
				// ------

				// --- Run cycle ---
					Common::DASH => transfers! {
						{ input.jump => Common::KNEE_BEND },
						{ agent.collisions.wall => Common::STOP_WALL },
						{ interrupt!(dash::attacks) },
						{ over!(Common::RUN) }
					},
					Common::RUN => transfers! {
						{ input.jump => Common::KNEE_BEND },
						{ input.trigger.activated() => Common::GUARD_ON },
						{ agent.collisions.wall => Common::STOP_WALL },
						{ input.joystick.backwards() => Common::TURN_RUN },
						{ input.joystick.neutral() => Common::RUN_BRAKE },
						{ interrupt!(dash::attacks) }
					},
					Common::RUN_BRAKE => transfers! {
						{ input.jump => Common::KNEE_BEND },
						{ input.joystick.backwards() => Common::TURN_RUN }
					},
					Common::TURN_RUN => transfers! {
						{ input.jump => Common::KNEE_BEND }
					},
				// ------

				Common::KNEE_BEND => transfers! {
					{ input.joystick.backwards() => Common::JUMP_B },
					{ over!(Common::JUMP_F) }
				},

				// --- Squat cycle ---
					Common::SQUAT => transfers! {
						{ input.a => Common::ATTACK_LW_3 },
						{ interrupt!(actionable::grounded) },
						{ over!(Common::SQUAT_WAIT) }
					},
					Common::SQUAT_WAIT => transfers! {
						{ input.a => Common::ATTACK_LW_3 },
						{ interrupt!(actionable::grounded) },
						{ !input.joystick.downwards() => Common::SQUAT_RV }
					},
					Common::SQUAT_RV => transfers! {{ interrupt!(actionable::grounded) }},
				// ------

				Common::PASS => transfers! {{ interrupt!(actionable::aerial) }},

				// --- Ledge teeter cycle ---
					Common::OTTOTTO => transfers! {
						{ interrupt!(actionable::grounded) },
						{ over!(Common::OTTOTTO_WAIT) }
					},
					Common::OTTOTTO_WAIT => transfers! {{ interrupt!(actionable::grounded) }},
				// ------

				// --- Bumping ---
					Common::STOP_WALL => transfers! {{ interrupt!(actionable::grounded) }},
				// ------
			// ------

			// --- Jump transfers ---
				// --- Grounded ---
					Common::JUMP_F => transfers! {{ interrupt!(actionable::aerial) }},
					Common::JUMP_B => transfers! {{ interrupt!(actionable::aerial) }},
				// ------
		
				// --- Airborne ---
					Common::JUMP_AERIAL_F => transfers! {{ interrupt!(actionable::aerial) }},
					Common::JUMP_AERIAL_B => transfers! {{ interrupt!(actionable::aerial) }},
				// ------
			// ------
		}
	}
}