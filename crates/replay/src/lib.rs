//! Streaming validation of explicit input-to-next-observation transitions.
//!
//! A caller supplies a complete native simulation checkpoint separately from replay
//! observations. Observations cannot generally reconstruct hidden game state.
//! `peppi-adapter` supplies the Peppi importer separately; this crate has no
//! dependency on the parser or on a particular game stepper.
#![forbid(unsafe_code)]

use std::{convert::Infallible, error::Error, fmt};

/// State immediately before advancing the input whose observation is labeled
/// `next_frame`. Signed indices retain pre-game replay frames without rebasing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint<C> {
    pub next_frame: i32,
    pub state: C,
}

/// The observation expected *after* applying this input for `frame`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition<I, O> {
    pub frame: i32,
    pub input: I,
    pub expected: O,
}

pub trait FrameStepper {
    type Checkpoint;
    type Input;
    type Observation;
    type Error: Error + 'static;

    /// Restore all state affecting later transitions, including RNGs and hidden
    /// counters. The caller must provide a complete native checkpoint, not a partial replay
    /// observation. On failure, the stepper's state may have changed.
    fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<(), Self::Error>;

    /// Advance exactly one simulation step and observe its resulting state.
    fn advance(&mut self, input: &Self::Input) -> Result<Self::Observation, Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationReport {
    pub first_frame: i32,
    pub last_frame: i32,
    pub checked_frames: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError<E, D, R = Infallible> {
    #[error("no expected transitions were supplied")]
    Empty,
    #[error("checkpoint restoration failed: {source}")]
    Restore { source: E },
    #[error("transition stream failed after {checked_frames} matched frames: {source}")]
    Read { checked_frames: u64, source: R },
    #[error("expected frame {expected}, found {actual} after {checked_frames} matched frames")]
    Sequence {
        expected: i32,
        actual: i32,
        checked_frames: u64,
    },
    #[error("frame index overflow after frame {after} ({checked_frames} matched frames)")]
    FrameOverflow { after: i32, checked_frames: u64 },
    #[error("advancing frame {frame} failed after {checked_frames} matched frames: {source}")]
    Advance {
        frame: i32,
        checked_frames: u64,
        source: E,
    },
    #[error("frame {frame} differs after {checked_frames} matched frames: {difference}")]
    Mismatch {
        frame: i32,
        checked_frames: u64,
        difference: D,
    },
}

/// Validate a streaming sequence, stopping at the first error or differing frame.
/// The comparator receives `(expected, actual)` and returns a caller-defined
/// difference. Use bit comparisons for floats; ordinary float equality loses
/// signed-zero and NaN-payload information. A matching prefix is counted only
/// up to the first mismatch. The stepper remains available for diagnosis.
pub fn validate<S, T, F, D>(
    stepper: &mut S,
    checkpoint: &Checkpoint<S::Checkpoint>,
    transitions: T,
    compare: F,
) -> Result<ValidationReport, ValidationError<S::Error, D>>
where
    S: FrameStepper,
    T: IntoIterator<Item = Transition<S::Input, S::Observation>>,
    F: FnMut(&S::Observation, &S::Observation) -> Option<D>,
{
    validate_fallible(
        stepper,
        checkpoint,
        transitions.into_iter().map(Ok),
        compare,
    )
}

/// Fallible-stream variant for future parsers: a read failure after a matching
/// prefix is an error, never a successful end of replay. This function retains
/// only the current transition; the caller is responsible for stream completeness
/// and selecting finalized replay frames before validation.
pub fn validate_fallible<S, T, F, D, R>(
    stepper: &mut S,
    checkpoint: &Checkpoint<S::Checkpoint>,
    transitions: T,
    mut compare: F,
) -> Result<ValidationReport, ValidationError<S::Error, D, R>>
where
    S: FrameStepper,
    T: IntoIterator<Item = Result<Transition<S::Input, S::Observation>, R>>,
    F: FnMut(&S::Observation, &S::Observation) -> Option<D>,
    R: Error + 'static,
{
    let mut transitions = transitions.into_iter();
    let first = transitions
        .next()
        .ok_or(ValidationError::Empty)?
        .map_err(|source| ValidationError::Read {
            checked_frames: 0,
            source,
        })?;
    stepper
        .restore(&checkpoint.state)
        .map_err(|source| ValidationError::Restore { source })?;
    let mut checked_frames = 0;
    let mut previous: Option<i32> = None;
    for transition in std::iter::once(Ok(first)).chain(transitions) {
        let transition = transition.map_err(|source| ValidationError::Read {
            checked_frames,
            source,
        })?;
        let expected = match previous {
            None => checkpoint.next_frame,
            Some(after) => after.checked_add(1).ok_or(ValidationError::FrameOverflow {
                after,
                checked_frames,
            })?,
        };
        if transition.frame != expected {
            return Err(ValidationError::Sequence {
                expected,
                actual: transition.frame,
                checked_frames,
            });
        }
        let actual =
            stepper
                .advance(&transition.input)
                .map_err(|source| ValidationError::Advance {
                    frame: transition.frame,
                    checked_frames,
                    source,
                })?;
        if let Some(difference) = compare(&transition.expected, &actual) {
            return Err(ValidationError::Mismatch {
                frame: transition.frame,
                checked_frames,
                difference,
            });
        }
        checked_frames += 1; // At most 2^32 contiguous i32 frame labels can match.
        previous = Some(transition.frame);
    }
    Ok(ValidationReport {
        first_frame: checkpoint.next_frame,
        last_frame: previous.expect("the nonempty stream matched at least one frame"),
        checked_frames,
    })
}

/// Clone a stepper and restore a checkpoint for an independent counterfactual
/// branch. The original is untouched. Implementors must ensure `Clone` does not
/// share mutable simulation state across branches.
pub fn branch_from<S: FrameStepper + Clone>(
    original: &S,
    checkpoint: &Checkpoint<S::Checkpoint>,
) -> Result<S, S::Error> {
    let mut branch = original.clone();
    branch.restore(&checkpoint.state)?;
    Ok(branch)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BitDifference<B> {
    pub expected: B,
    pub actual: B,
}

impl<B: fmt::LowerHex> fmt::Display for BitDifference<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "expected bits {:#x}, got {:#x}",
            self.expected, self.actual
        )
    }
}

pub fn compare_f32_bits(expected: &f32, actual: &f32) -> Option<BitDifference<u32>> {
    let (expected, actual) = (expected.to_bits(), actual.to_bits());
    (expected != actual).then_some(BitDifference { expected, actual })
}

pub fn compare_f64_bits(expected: &f64, actual: &f64) -> Option<BitDifference<u64>> {
    let (expected, actual) = (expected.to_bits(), actual.to_bits());
    (expected != actual).then_some(BitDifference { expected, actual })
}
