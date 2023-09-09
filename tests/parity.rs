// skirmish is very easy to test. all we need to do is plug in a file, run simulation for a frame, and check
// if the game state is equal between skirmish and the file.

use peppi;
use peppi::{
	model::{
		frame::Frame,
		game::Frames,
	},
	serde::collect::{
		Opts,
		Rollback
	},
};

use skirmish;

type SnapshotCheck = fn(state: skirmish::Snapshot, next: skirmish::Snapshot, previous: skirmish::Snapshot, index: usize);

pub struct Context<const N: usize> {
	pub snapshot: skirmish::Snapshot<N>,
	pub inputs: skirmish::Inputs<N>, // inputs that create the next snapshot
}

fn parity(check: Fn) {
	let mut buf = io::BufReader::new(fs::File::open("game.slp").unwrap());
		
	let collect_opts = Opts { rollback: Rollback::First };

	let game = peppi::game(&mut buf, None, Some(&collect_opts)).unwrap();

	// map game.frames where each skirmish::Snapshot is created from a slice of all previous frames

	let contexts: Vec<Context<2>> = game.frames.downcast().unwrap().enumerate()
		.map(|(index, frame)| 
			Context { 
				snapshot: skirmish::Snapshot::game::<2>(frames[..index].to_vec(), game.start), 
				inputs: skirmish::Inputs::from::<2>(frame)
			}
		).collect();

	for [context, next] in contexts.windows(2) {
		let simulator = skirmish::Simulator::from::<2>(snapshot);
		
		// simulator.build(); // build state from frame
		simulator.step(context.inputs); // simulate one frame
		
		if let Some(end) = simulator.end {
			return assert_eq!(end, game.end, "Game end fails parity")
		}

		check(simulator.snapshot, next.snapshot, context.snapshot, index);
	}
}

#[test]
fn full_parity() {
	fn full_check(state: skirmish::Snapshot, next: skirmish::Snapshot, previous: skirmish::Snapshot, index: usize) {
		assert_eq!(state, next, "Frame {} fails full parity", index);
	}

	parity(full_check);
}

#[test]
fn action_state_parity() {
	fn action_state_check(state: skirmish::Snapshot, next: skirmish::Snapshot, previous: skirmish::Snapshot, index: usize) {
		assert_eq!(state, next, "Frame {}, agent {} fails action state parity. Action state {} actually transferred to {} but simulated {}", index);
	}

	parity(action_state_check);
}

#[test]
fn collision_parity() {
	fn collision_check(state: skirmish::Snapshot, next: skirmish::Snapshot, previous: skirmish::Snapshot, index: usize) {
		assert_eq!(state, next, "Frame {}, agent {} fails collision parity. Agent actually had collision {} but simulated {}", index);
	}

	parity(collision_check);
}

#[test]
fn physics_parity() {
	fn physics_check(state: skirmish::Snapshot, next: skirmish::Snapshot, previous: skirmish::Snapshot, index: usize) {
		assert_eq!(state, next, "Frame {}, agent {} fails physics parity. Agent actually had position {} but simulated {}", index);
	}

	parity(physics_check);
}