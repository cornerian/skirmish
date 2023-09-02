use crate::hits::Hittable;
use crate::snapshot::{Agent, Snapshot};

pub trait Step {
	fn step(&self, inputs: Inputs) -> Self;
}

impl Step for Agent {
	fn step(&self, inputs: Inputs) -> Self {
		self
	}
}

impl Step for Snapshot {
	fn step(&self, inputs: Inputs) -> Self {
		// check if any agents were hit by other agents
		for (i, agent) in self.agents.enumerate() {
			let hits = agent.hits(self.agents.enumerate().filter(|oi, agent| oi != i));

			agents.map(|agent| agent.apply_hits(hits))
		}
	}
}