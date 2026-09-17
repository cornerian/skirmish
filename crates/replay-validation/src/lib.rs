//! Simulator-agnostic streaming validation of input-to-observation transitions.
//!
//! A caller supplies a complete native simulation checkpoint separately from replay
//! observations. Observations cannot generally reconstruct hidden game state.
//! `peppi-adapter` supplies the Peppi importer separately; this crate has no
//! dependency on the parser or on a particular game stepper.
#![forbid(unsafe_code)]

use std::{cell::Cell, convert::Infallible, error::Error, fmt};

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

/// The first differing observation found while a diagnostic validation run
/// continued stepping the native simulation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstMismatch<D> {
    pub frame: i32,
    pub checked_frames: u64,
    pub difference: D,
}

/// Complete result metadata for a diagnostic run. Unlike [`ValidationReport`],
/// `checked_frames` remains the length of the exact matching prefix while
/// `simulated_frames` records every successful native step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContinuedValidationReport<D> {
    pub first_frame: i32,
    pub last_frame: i32,
    pub checked_frames: u64,
    pub simulated_frames: u64,
    pub first_mismatch: Option<FirstMismatch<D>>,
}

/// A diagnostic run can encounter a real stream, sequence, or simulation
/// error after already observing a mismatch. Preserve both pieces of
/// information so callers do not mistake the partial run for a clean result.
#[derive(Debug)]
pub struct ContinuedValidationError<E, D, R> {
    pub error: ValidationError<E, D, R>,
    pub first_mismatch: Option<FirstMismatch<D>>,
    pub last_simulated_frame: Option<i32>,
    pub simulated_frames: u64,
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

/// Validate a streaming sequence to its end while retaining the first
/// mismatch. This is intended for diagnosis: it always advances through
/// later inputs after a difference, but it never treats a differing run as a
/// successful validation. A genuine stream, sequence, or simulation error
/// still terminates the run and is returned with the mismatch metadata seen
/// so far.
pub fn validate_fallible_continue<S, T, F, D, R>(
    stepper: &mut S,
    checkpoint: &Checkpoint<S::Checkpoint>,
    transitions: T,
    mut compare: F,
) -> Result<ContinuedValidationReport<D>, ContinuedValidationError<S::Error, D, R>>
where
    S: FrameStepper,
    T: IntoIterator<Item = Result<Transition<S::Input, S::Observation>, R>>,
    F: FnMut(&S::Observation, &S::Observation) -> Option<D>,
    R: Error + 'static,
{
    // Preserve the original compatibility behavior: once the first
    // mismatch is found, this wrapper keeps simulating but no longer invokes
    // the comparator. Callers that need the complete mismatch stream use
    // `validate_fallible_continue_with` below.
    let compare_after_mismatch = Cell::new(true);
    validate_fallible_continue_with(
        stepper,
        checkpoint,
        transitions,
        |expected, actual| {
            compare_after_mismatch
                .get()
                .then(|| compare(expected, actual))
                .flatten()
        },
        |_| compare_after_mismatch.set(false),
    )
}

/// Validate a streaming sequence to its end while retaining the first
/// mismatch and notifying the caller about every mismatch. The callback is
/// invoked in frame order after each successful native step whose comparison
/// differs. The callback borrows the mismatch, so the validation result still
/// owns the first difference without requiring `D: Clone`.
pub fn validate_fallible_continue_with<S, T, F, D, R, M>(
    stepper: &mut S,
    checkpoint: &Checkpoint<S::Checkpoint>,
    transitions: T,
    mut compare: F,
    mut on_mismatch: M,
) -> Result<ContinuedValidationReport<D>, ContinuedValidationError<S::Error, D, R>>
where
    S: FrameStepper,
    T: IntoIterator<Item = Result<Transition<S::Input, S::Observation>, R>>,
    F: FnMut(&S::Observation, &S::Observation) -> Option<D>,
    R: Error + 'static,
    M: FnMut(&FirstMismatch<D>),
{
    let mut transitions = transitions.into_iter();
    let first = match transitions.next() {
        None => {
            return Err(ContinuedValidationError {
                error: ValidationError::Empty,
                first_mismatch: None,
                last_simulated_frame: None,
                simulated_frames: 0,
            });
        }
        Some(Err(source)) => {
            return Err(ContinuedValidationError {
                error: ValidationError::Read {
                    checked_frames: 0,
                    source,
                },
                first_mismatch: None,
                last_simulated_frame: None,
                simulated_frames: 0,
            });
        }
        Some(Ok(first)) => first,
    };

    if let Err(source) = stepper.restore(&checkpoint.state) {
        return Err(ContinuedValidationError {
            error: ValidationError::Restore { source },
            first_mismatch: None,
            last_simulated_frame: None,
            simulated_frames: 0,
        });
    }

    let mut checked_frames = 0;
    let mut simulated_frames = 0;
    let mut last_simulated_frame = None;
    let mut previous: Option<i32> = None;
    let mut first_mismatch = None;

    for transition in std::iter::once(Ok(first)).chain(transitions) {
        let transition = match transition {
            Ok(transition) => transition,
            Err(source) => {
                return Err(ContinuedValidationError {
                    error: ValidationError::Read {
                        checked_frames,
                        source,
                    },
                    first_mismatch,
                    last_simulated_frame,
                    simulated_frames,
                });
            }
        };
        let expected = match previous {
            None => checkpoint.next_frame,
            Some(after) => match after.checked_add(1) {
                Some(expected) => expected,
                None => {
                    return Err(ContinuedValidationError {
                        error: ValidationError::FrameOverflow {
                            after,
                            checked_frames,
                        },
                        first_mismatch,
                        last_simulated_frame,
                        simulated_frames,
                    });
                }
            },
        };
        if transition.frame != expected {
            return Err(ContinuedValidationError {
                error: ValidationError::Sequence {
                    expected,
                    actual: transition.frame,
                    checked_frames,
                },
                first_mismatch,
                last_simulated_frame,
                simulated_frames,
            });
        }

        let actual = match stepper.advance(&transition.input) {
            Ok(actual) => actual,
            Err(source) => {
                return Err(ContinuedValidationError {
                    error: ValidationError::Advance {
                        frame: transition.frame,
                        checked_frames,
                        source,
                    },
                    first_mismatch,
                    last_simulated_frame,
                    simulated_frames,
                });
            }
        };
        simulated_frames += 1;
        last_simulated_frame = Some(transition.frame);
        if let Some(difference) = compare(&transition.expected, &actual) {
            let mismatch = FirstMismatch {
                frame: transition.frame,
                checked_frames,
                difference,
            };
            on_mismatch(&mismatch);
            if first_mismatch.is_none() {
                first_mismatch = Some(mismatch);
            }
        }
        if first_mismatch.is_none() {
            checked_frames += 1;
        }
        previous = Some(transition.frame);
    }

    Ok(ContinuedValidationReport {
        first_frame: checkpoint.next_frame,
        last_frame: previous.expect("the nonempty stream simulated at least one frame"),
        checked_frames,
        simulated_frames,
        first_mismatch,
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Stepper {
        value: i32,
        advances: usize,
    }

    impl FrameStepper for Stepper {
        type Checkpoint = i32;
        type Input = i32;
        type Observation = i32;
        type Error = std::io::Error;

        fn restore(&mut self, checkpoint: &Self::Checkpoint) -> Result<(), Self::Error> {
            self.value = *checkpoint;
            Ok(())
        }

        fn advance(&mut self, input: &Self::Input) -> Result<Self::Observation, Self::Error> {
            self.value += *input;
            self.advances += 1;
            Ok(self.value)
        }
    }

    fn transitions() -> Vec<Transition<i32, i32>> {
        vec![
            Transition {
                frame: 10,
                input: 1,
                expected: 99,
            },
            Transition {
                frame: 11,
                input: 1,
                expected: 2,
            },
            Transition {
                frame: 12,
                input: 1,
                expected: 3,
            },
        ]
    }

    #[test]
    fn default_validation_stops_at_the_first_mismatch() {
        let mut stepper = Stepper {
            value: -1,
            advances: 0,
        };
        let result = validate(
            &mut stepper,
            &Checkpoint {
                next_frame: 10,
                state: 0,
            },
            transitions(),
            |expected, actual| (*expected != *actual).then_some((*expected, *actual)),
        );
        assert!(matches!(
            result,
            Err(ValidationError::Mismatch {
                frame: 10,
                checked_frames: 0,
                difference: (99, 1),
            })
        ));
        assert_eq!(stepper.advances, 1);
    }

    #[test]
    fn continued_validation_advances_past_the_first_mismatch() {
        let mut stepper = Stepper {
            value: -1,
            advances: 0,
        };
        let report = validate_fallible_continue(
            &mut stepper,
            &Checkpoint {
                next_frame: 10,
                state: 0,
            },
            transitions().into_iter().map(Ok::<_, std::io::Error>),
            |expected, actual| (*expected != *actual).then_some((*expected, *actual)),
        )
        .unwrap();
        assert_eq!(stepper.advances, 3);
        assert_eq!(report.last_frame, 12);
        assert_eq!(report.simulated_frames, 3);
        assert_eq!(report.checked_frames, 0);
        assert_eq!(
            report.first_mismatch,
            Some(FirstMismatch {
                frame: 10,
                checked_frames: 0,
                difference: (99, 1),
            })
        );
    }

    #[test]
    fn diagnostic_continuation_compares_and_reports_separated_mismatches() {
        let mut stepper = Stepper {
            value: -1,
            advances: 0,
        };
        let mut records = transitions();
        records[2].expected = 8;
        let mut mismatches = Vec::new();
        let report = validate_fallible_continue_with(
            &mut stepper,
            &Checkpoint {
                next_frame: 10,
                state: 0,
            },
            records.into_iter().map(Ok::<_, std::io::Error>),
            |expected, actual| (*expected != *actual).then_some((*expected, *actual)),
            |mismatch| mismatches.push((mismatch.frame, mismatch.difference)),
        )
        .unwrap();

        assert_eq!(stepper.advances, 3);
        assert_eq!(report.checked_frames, 0);
        assert_eq!(
            report.first_mismatch,
            Some(FirstMismatch {
                frame: 10,
                checked_frames: 0,
                difference: (99, 1),
            })
        );
        assert_eq!(mismatches, [(10, (99, 1)), (12, (8, 3))]);
    }
}
