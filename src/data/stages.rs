use phf;

use peppi::model::enums::stage::Stage;
use peppi::model::primitives::Position;

#[derive(Clone, Debug)]
pub struct PlatformData {
	pub position: Position,
	pub length: f32,
}

#[derive(Clone, Debug)]
pub struct BlastData {
	pub top: f32,
	pub bottom: f32,
	pub left: f32,
	pub right: f32,
}

#[derive(Clone, Debug)]
pub struct StageData {
	// pub polygons: [Position],
	pub length: f32,
	pub blasts: BlastData, // top, bottom, left, right
	pub platforms: &'static [PlatformData],
}

pub static STAGES: phf::Map<&'static Stage, StageData> = phf::phf_map! {
    Stage::FOUNTAIN_OF_DREAMS => StageData {
		length: 128.85448,
		blasts: BlastData {
			top: 202.50, 
			bottom: -146.25,
			left: -198.75, 
			right: 198.75,
		},
		platforms: &[],
	},
    Stage::POKEMON_STADIUM => StageData {
		length: 175.5,
		blasts: BlastData {
			top: 180.00, 
			bottom: -111.00,
			left: -230.00, 
			right: 230.00,
		},
		platforms: &[],
	},
    Stage::YOSHIS_STORY => StageData {
		length: 112.6327,
		blasts: BlastData {
			top: 168.00, 
			bottom: -91.00,
			left: -175.70, 
			right: 173.60,
		},
		platforms: &[], // No.
	},
    Stage::DREAM_LAND_N64 => StageData {
		length: 128.85448,
		blasts: BlastData {
			top: 250.00, 
			bottom: -123.00,
			left: -255.00, 
			right: 255.00,
		},
		platforms: &[],
	},
    Stage::BATTLEFIELD => StageData {
		length: 136.8,
		blasts: BlastData {
			top: 200.00, 
			bottom: -108.80,
			left: -224.00, 
			right: 224.00,
		},
		platforms: &[],
	},
	Stage::FINAL_DESTINATION => StageData {
		length: 171.1314,
		blasts: BlastData {
			top: 188.00, 
			bottom: -140.00,
			left: -246.00, 
			right: 246.00,
		},
		platforms: &[],
	},
};