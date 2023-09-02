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

pub struct Context<const N: usize> {
	pub snapshot: skirmish::Snapshot<N>,
	pub inputs: skirmish::Inputs<N>, // inputs that create the next snapshot
}

#[test]
fn has_parity() {
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

		assert_eq!(simulator.snapshot, next.snapshot, "Frame {} fails parity", index);
	}
}