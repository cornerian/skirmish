use peppi::model::enums::action_state::{Common, State};
use peppi::model::primitives::Position;

use crate::snapshot::Agent;
use crate::math::Sphere;

pub struct StateData {
	pub effect: Box<dyn Fn(Agent) -> Agent>,
	pub counter: Option<usize>,
	pub spheres: Vec<Box<(Sphere, Position)>>,
}

pub trait Statable {
	fn crouching(&self) -> bool;
}

impl Statable for Agent {
	fn crouching(&self) -> bool {
		match self.state.action.state {
			State::Common(common) => {
				match common {
					Common::SQUAT | Common::SQUAT_WAIT | Common::SQUAT_RV => true,
					_ => false,
				}
			},
			_ => false,
		}

	}
}