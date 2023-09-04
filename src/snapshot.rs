use peppi::model::{
	buttons::Physical,
	enums::{action_state, character, ground, stage},
	frame::{
		self,
		Buttons,
		Frame,
		HurtboxState,
		PortData,
		StateFlags,
		Triggers,
		Velocities
	},
	game,
	item,
	primitives::{Direction, Port, Position, Velocity},
};

use crate::hits::Attack;

// histories
pub type History<T: Clone, const N: usize> = Vec<[T; N]>;

pub trait Transposable<T: Clone, const N: usize> {
	fn transpose(self) -> [Vec<T>; N];
}

impl<T: Clone, const N: usize> Transposable<T, N> for History<T, N> {
	fn transpose(self) -> [Vec<T>; N] {
		let mut transposed: [Vec<T>; N] = unsafe { // safe because N <= 4
            std::mem::MaybeUninit::uninit().assume_init()
        };

        for moment in self {
            for (i, &item) in moment.iter().enumerate() {
                transposed[i].push(item);
            }
        };

        transposed
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct Input { // input from one player on one frame
	pub joystick: Position,
	pub cstick: Position,
	pub trigger: f32, // 0 to 1 if analog, MAX if digital
	pub jump: bool,
	pub a: bool,
	pub b: bool,
	pub z: bool,
	pub taunt: bool,
}

impl From<PortData> for Input {
	fn from(pd: PortData) -> Self {
		let pre = pd.leader.pre;

		Self {
			joystick: pre.joystick,
			cstick: pre.cstick,
			trigger: f32::max(pre.triggers.physical.l, pre.triggers.physical.r),
			jump: (pre.buttons.physical.0 & (Physical::X.0 | Physical::Y.0)) != 0,
			a: (pre.buttons.physical.0 & Physical::A.0) != 0,
			b: (pre.buttons.physical.0 & Physical::B.0) != 0,
			z: (pre.buttons.physical.0 & Physical::Z.0) != 0,
			taunt: (pre.buttons.physical.0 & Physical::DPAD_UP.0) != 0,
		}
	}
}

pub struct Inputs<const N: usize>([Input; N]); // inputs from all players from one frame

impl<const N: usize> From<Frame<N>> for Inputs<N> {
	fn from(frame: Frame<N>) -> Self {
		Self(frame.ports.map(|port| Input::from(port)))
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionState {
	pub state: action_state::State,
	pub age: usize,
	pub counter: Option<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CharacterState {
	pub action: ActionState,

	pub hurtbox_state: Option<HurtboxState>,
	pub hurtbox_state_counter: Option<usize>,

	pub flags: Option<StateFlags>,

	pub hitlag: Option<usize>,
	pub hit_with: Option<Attack>,

	pub animation_index: usize,
}

/// e don't use pre and post, every snapshot is the result of processing the former snapshot plus new inputs
#[derive(Clone, Debug, PartialEq)]
pub struct Agent {
	/// all previous inputs
	pub inputs: Vec<Input>,

	/// in-game character (can only change for Zelda/Sheik)
	pub character: character::Internal,

	/// action state
	pub state: CharacterState,

	pub position: Position,

	pub direction: Direction,

	/// damage percent
	pub damage: f32,

	/// shield size
	pub shield: f32,

	/// stocks remaining
	pub stocks: u8,

	pub airborne: bool,

	pub weight: f32,

	/// ground the character is standing on, if any
	pub ground: Option<ground::Ground>,

	/// jumps remaining
	pub jumps: u8,

	/// true = successful L-Cancel
	pub l_cancel: Option<bool>,

	pub velocities: Velocities,

	pub attacks: [action_state::State; 9], // used for calculating stale moves

	pub follows: Option<usize>, // nana is her own agent (not supported at the moment)
}

impl From<Vec<PortData>> for Agent {
	fn from(history: Vec<PortData>) -> Self {
		let last = history.last().unwrap();
		let post = last.leader.post; // nana not supported at the moment
		
		// last frame with a different state to current
		let change = history.iter().rev().position(|data| data.leader.post.state != post.state).unwrap_or(history.len());
		let hurtbox_change = history.iter().rev().position(|data| data.leader.post.hurtbox_state != post.hurtbox_state).unwrap_or(history.len());

		Self {
			inputs: history.iter().map(|data| Input::from(*data)).collect(),
			character: post.character,
			state: CharacterState {
				action: ActionState {
					state: post.state,
					age: change,
					counter: history[change].counter(),
				},

				hurtbox_state: post.hurtbox_state,
				hurtbox_state_counter: history[hurtbox_change].hurtbox_counter(),

				flags: post.flags,

				hitlag: post.hitlag,

				animation_index: post.animation_index.unwrap(),
			},

			position: post.position,

			direction: post.direction,

			damage: post.damage,

			shield: post.shield,

			stocks: post.stocks,

			airborne: post.airborne.unwrap(),

			ground: post.ground,

			jumps: post.jumps.unwrap(),

			l_cancel: post.l_cancel.unwrap(),

			velocities: post.velocities.unwrap(),

			follows: None,
		}
	}
}

/// A single frame of the simulator. `N` is the number of agents in the game.
// This differs from a normal peppi Frame by adding extra history that allows
// the simulator continue from any given snapshot.
#[derive(PartialEq, Debug)]
pub struct Snapshot<const N: usize> {
	/// Frame index (starts at `peppi::game::FIRST_FRAME_INDEX`).
	///
	/// Indexes should never skip values, but may decrease if rollbacks
	/// are enabled (see `peppi::serde::collect::Opts`).
	/// TODO: implement rollback in simulator
	pub index: i32,

	/// Frame data for each agent. The agent with the lowest port is always at index 0.
	pub agents: [Agent; N],

	// Start/end frame data is not relevant to the simulator

	pub items: Option<Vec<item::Item>>,

	// Normally within game Start
	pub stage: stage::Stage,

	pub random_seed: u32,
	// We don't need everything because we assume tournament settings: no spawning items, PS is frozen, damage ratio is 1.0, etc.
}

impl<const N: usize> Snapshot<N> {
	pub fn game(frames: Vec<Frame<N>>, start: game::Start) -> Self {
		// convert frame history into snapshot history

		// get last frame in history
		let frame = frames.last().unwrap();
		let inputs = Inputs::from(*frame);
		let ports: History<PortData, N> = frames.iter().map(|frame| frame.ports).collect();

		let agents = ports.transpose().map(|history| Agent::from(history));

        Self { 
			index: frame.index,
			agents,
			items: frame.items,
			stage: start.stage,
			random_seed: start.random_seed,
		}
    }
}