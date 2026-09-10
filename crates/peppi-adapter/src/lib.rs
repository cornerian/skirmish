//! Peppi-backed replay import. Original recorded fields remain available through
//! Peppi's types; they are observations, not complete simulator checkpoints.
#![forbid(unsafe_code)]

pub use peppi;
pub use peppi::{frame::transpose as row, game::Port, io::slippi::Version};

mod envelope;
mod frames;

use peppi::{frame::FIRST_INDEX, game::immutable::Game};
use replay_validation::Transition;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};

pub const PARSER: &str = "peppi 2.1.2";
/// Bound one file, not a corpus. Parsed Arrow columns also occupy memory.
pub const MAX_REPLAY_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Timeline {
    /// Last surviving recording of each frame in a completed replay.
    #[default]
    LastRecorded,
    /// Only the prefix explicitly finalized by frame bookends (Slippi >=3.7).
    FinalizedOnly,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("replay I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Peppi could not parse replay: {0}")]
    Parse(#[from] peppi::io::Error),
    #[error("invalid or unsupported replay: {0}")]
    Invalid(String),
    #[error("unsupported Slippi version {0}; supported range is 2.0.0 through 3.18.0")]
    UnsupportedVersion(Version),
    #[error("Peppi rejected malformed replay data with a panic")]
    ParserPanic,
}

/// Peppi supplies the payload types. Absent actors have no record; nullable
/// Arrow rows must never turn into invented zero-valued fighters.
#[derive(Clone, Debug, PartialEq)]
pub struct Actor {
    pub port: Port,
    pub follower: bool,
    pub pre: row::Pre,
    pub post: row::Post,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub id: i32,
    pub actors: Vec<Actor>,
    pub start: Option<row::Start>,
    pub end: Option<row::End>,
    pub items: Option<Vec<row::Item>>,
    pub fod_platforms: Option<Vec<row::FodPlatform>>,
    pub dreamland_whispys: Option<Vec<row::DreamlandWhispy>>,
    pub stadium_transformations: Option<Vec<row::StadiumTransformation>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActorInput {
    pub port: Port,
    pub follower: bool,
    /// Includes both controller input and observed pre-state. An eventual
    /// simulation adapter must select input fields explicitly, not overwrite
    /// simulated positions, state or RNG with this reference observation.
    pub pre: row::Pre,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub actors: Vec<ActorInput>,
    pub start: Option<row::Start>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Summary {
    pub parser: &'static str,
    pub sha256: String,
    pub bytes: usize,
    pub version: Version,
    pub stage: u16,
    pub ports: Vec<Port>,
    pub physical_frames: usize,
    pub surviving_frames: usize,
    pub discarded_frames: usize,
    pub selected_frames: usize,
    pub first_frame: Option<i32>,
    pub last_frame: Option<i32>,
    pub latest_finalized_frame: Option<i32>,
    pub timeline: Timeline,
    pub end_method: peppi::game::EndMethod,
}

/// One immutable Peppi game plus a compact index into its surviving timeline.
/// No duplicated corpus/row table, simulator state or inferred asset data.
#[derive(Debug)]
pub struct Replay {
    game: Game,
    indices: Vec<usize>,
    latest_finalized: Option<i32>,
    digest: [u8; 32],
    bytes: usize,
}

impl Replay {
    pub fn read(reader: impl Read) -> Result<Self, Error> {
        let mut bytes = Vec::new();
        reader.take(MAX_REPLAY_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_REPLAY_BYTES {
            return Err(Error::Invalid(
                "file exceeds the 512 MiB replay limit".into(),
            ));
        }
        envelope::check(&bytes)?;
        let mut cursor = Cursor::new(&bytes);
        // Peppi uses assertions for some malformed event sequences. Convert
        // those failures to an import error without changing global panic hooks.
        let game = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            peppi::io::slippi::read(&mut cursor, None)
        }))
        .map_err(|_| Error::ParserPanic)??;
        if cursor.position() != bytes.len() as u64 {
            return Err(Error::Invalid("content after the Slippi wrapper".into()));
        }
        let version = game.start.slippi.version;
        if version < Version(2, 0, 0) || version > peppi::io::slippi::MAX_SUPPORTED_VERSION {
            return Err(Error::UnsupportedVersion(version));
        }
        if game.end.is_none() {
            return Err(Error::Invalid("missing GameEnd".into()));
        }
        frames::validate(&game)?;
        let (indices, latest_finalized) = timeline(&game)?;
        Ok(Self {
            game,
            indices,
            latest_finalized,
            digest: Sha256::digest(&bytes).into(),
            bytes: bytes.len(),
        })
    }

    /// Retain Peppi's columnar arrays, match settings, metadata and Gecko bytes
    /// for analysis and future compatibility checks. This is read-only access.
    pub fn game(&self) -> &Game {
        &self.game
    }
    pub fn sha256(&self) -> [u8; 32] {
        self.digest
    }

    pub fn frame_indices(&self, policy: Timeline) -> Result<&[usize], Error> {
        let count = match policy {
            Timeline::LastRecorded => self.indices.len(),
            Timeline::FinalizedOnly => {
                if !self.game.start.slippi.version.gte(3, 7) {
                    return Err(Error::Invalid(
                        "finalized-only requires Slippi 3.7 frame bookends".into(),
                    ));
                }
                self.latest_finalized.map_or(0, |last| {
                    self.indices
                        .partition_point(|&i| self.game.frames.id.values()[i] <= last)
                })
            }
        };
        Ok(&self.indices[..count])
    }

    pub fn frame(&self, physical_index: usize) -> Result<Frame, Error> {
        frames::read(&self.game, physical_index)
    }

    /// Pre/post events with the same Slippi frame ID form one transition: apply
    /// that frame's controller inputs, then compare its post-frame observation.
    /// This does not initialize or claim compatibility with the experimental match.
    pub fn transitions(
        &self,
        policy: Timeline,
    ) -> Result<impl Iterator<Item = Result<Transition<Inputs, Frame>, Error>> + '_, Error> {
        Ok(self.frame_indices(policy)?.iter().map(|&index| {
            let expected = self.frame(index)?;
            let input = Inputs {
                start: expected.start,
                actors: expected
                    .actors
                    .iter()
                    .map(|a| ActorInput {
                        port: a.port,
                        follower: a.follower,
                        pre: a.pre,
                    })
                    .collect(),
            };
            Ok(Transition {
                frame: expected.id,
                input,
                expected,
            })
        }))
    }

    pub fn summary(&self, policy: Timeline) -> Result<Summary, Error> {
        let selected = self.frame_indices(policy)?;
        let ids = self.game.frames.id.values();
        Ok(Summary {
            parser: PARSER,
            sha256: self
                .digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
            bytes: self.bytes,
            version: self.game.start.slippi.version,
            stage: self.game.start.stage,
            ports: self.game.start.players.iter().map(|p| p.port).collect(),
            physical_frames: self.game.frames.len(),
            surviving_frames: self.indices.len(),
            discarded_frames: self.game.frames.len() - self.indices.len(),
            selected_frames: selected.len(),
            first_frame: selected.first().map(|&i| ids[i]),
            last_frame: selected.last().map(|&i| ids[i]),
            latest_finalized_frame: self.latest_finalized,
            timeline: policy,
            end_method: self.game.end.as_ref().expect("validated GameEnd").method,
        })
    }
}

fn timeline(game: &Game) -> Result<(Vec<usize>, Option<i32>), Error> {
    let mut indices = Vec::new();
    let mut finalized = None;
    for (index, &id) in game.frames.id.values().iter().enumerate() {
        if id < FIRST_INDEX || finalized.is_some_and(|last| id <= last) {
            return Err(Error::Invalid(format!(
                "frame {id} precedes the start or rewrites finalized history"
            )));
        }
        let offset = usize::try_from(i64::from(id) - i64::from(FIRST_INDEX)).unwrap();
        if offset > indices.len() {
            return Err(Error::Invalid(format!("gap before frame {id}")));
        }
        indices.truncate(offset);
        indices.push(index);
        if let Some(bookends) = &game.frames.end
            && let Some(watermarks) = &bookends.latest_finalized_frame
        {
            let watermark = watermarks.values()[index];
            if watermark > id {
                return Err(Error::Invalid(format!(
                    "frame {id} finalizes unrecorded frame {watermark}"
                )));
            }
            if watermark >= FIRST_INDEX {
                finalized =
                    Some(finalized.map_or(watermark, |previous: i32| previous.max(watermark)));
            }
        }
    }
    if indices.is_empty() {
        return Err(Error::Invalid("no recorded frames".into()));
    }
    Ok((indices, finalized))
}
