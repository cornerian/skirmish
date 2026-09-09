use super::*;
use melee_physics::Movement;
use melee_runtime::random::HsdRng;

// This is an explicit script of real translated helpers, not a game frame loop.
// Complete fixture state includes movement, RNG and an otherwise hidden counter.
#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeState {
    movement: Movement,
    rng: HsdRng,
    draw_count: u32,
}

#[derive(Clone, Debug)]
enum Input {
    Fall { gravity: f32, terminal: f32 },
    FrictionAir(f32),
    ProjectGround,
    Fail,
}

#[derive(Clone, Debug, PartialEq)]
struct Observation {
    velocity: [f32; 3],
    animation: [f32; 3],
    seed: u32,
    draw: i32,
    draw_count: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
enum StepError {
    #[error("checkpoint unavailable")]
    Restore,
    #[error("requested step failure")]
    Advance,
}

#[derive(Clone)]
struct ScriptedStepper {
    state: NativeState,
    restore_calls: usize,
    advance_calls: usize,
    fail_restore: bool,
}

impl FrameStepper for ScriptedStepper {
    type Checkpoint = NativeState;
    type Input = Input;
    type Observation = Observation;
    type Error = StepError;

    fn restore(&mut self, checkpoint: &NativeState) -> Result<(), StepError> {
        self.restore_calls += 1;
        if self.fail_restore {
            return Err(StepError::Restore);
        }
        self.state = *checkpoint;
        self.advance_calls = 0;
        Ok(())
    }

    fn advance(&mut self, input: &Input) -> Result<Observation, StepError> {
        self.advance_calls += 1;
        match *input {
            Input::Fall { gravity, terminal } => self.state.movement.fall(gravity, terminal),
            Input::FrictionAir(friction) => self.state.movement.friction_air(friction),
            Input::ProjectGround => self.state.movement.project_ground(),
            Input::Fail => return Err(StepError::Advance),
        }
        let draw = self.state.rng.rand();
        self.state.draw_count += 1;
        Ok(Observation {
            velocity: self.state.movement.self_velocity,
            animation: self.state.movement.animation_velocity,
            seed: self.state.rng.seed(),
            draw,
            draw_count: self.state.draw_count,
        })
    }
}

fn checkpoint() -> Checkpoint<NativeState> {
    Checkpoint {
        next_frame: -123,
        state: NativeState {
            movement: Movement {
                self_velocity: [2.0, 3.0, 0.0],
                ground_velocity: 2.0,
                ground_acceleration: 0.5,
                floor_normal: [0.0, 1.0, 0.0],
                ..Movement::default()
            },
            rng: HsdRng::new(1),
            draw_count: 40,
        },
    }
}

fn stepper() -> ScriptedStepper {
    let mut state = checkpoint().state;
    state.movement.self_velocity = [99.0; 3];
    state.rng = HsdRng::new(999);
    state.draw_count = 0;
    ScriptedStepper {
        state,
        restore_calls: 0,
        advance_calls: 0,
        fail_restore: false,
    }
}

fn transitions() -> [Transition<Input, Observation>; 3] {
    [
        Transition {
            frame: -123,
            input: Input::Fall {
                gravity: 0.5,
                terminal: 10.0,
            },
            expected: Observation {
                velocity: [2.0, 2.5, 0.0],
                animation: [0.0; 3],
                seed: 2_745_024,
                draw: 41,
                draw_count: 41,
            },
        },
        Transition {
            frame: -122,
            input: Input::FrictionAir(0.25),
            expected: Observation {
                velocity: [2.0, 2.5, 0.0],
                animation: [-0.25, 0.0, 0.0],
                seed: 3_357_800_067,
                draw: 51_235,
                draw_count: 42,
            },
        },
        Transition {
            frame: -121,
            input: Input::ProjectGround,
            expected: Observation {
                velocity: [2.0, -0.0, 0.0],
                animation: [0.5, -0.0, 0.0],
                seed: 415_139_642,
                draw: 6334,
                draw_count: 43,
            },
        },
    ]
}

fn compare_observations(expected: &Observation, actual: &Observation) -> Option<String> {
    for (field, expected, actual) in [
        ("velocity", &expected.velocity, &actual.velocity),
        ("animation", &expected.animation, &actual.animation),
    ] {
        for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
            if let Some(difference) = compare_f32_bits(expected, actual) {
                return Some(format!("{field}[{index}]: {difference}"));
            }
        }
    }
    if (expected.seed, expected.draw, expected.draw_count)
        != (actual.seed, actual.draw, actual.draw_count)
    {
        return Some(format!("RNG state: expected {expected:?}, got {actual:?}"));
    }
    None
}

#[test]
fn restores_complete_native_state_and_checks_negative_frames() {
    let mut stepper = stepper();
    let report = validate(
        &mut stepper,
        &checkpoint(),
        transitions(),
        compare_observations,
    )
    .unwrap();
    assert_eq!(
        report,
        ValidationReport {
            first_frame: -123,
            last_frame: -121,
            checked_frames: 3
        }
    );
    assert_eq!(stepper.restore_calls, 1);
    assert_eq!(stepper.advance_calls, 3);
    assert_eq!(stepper.state.rng.seed(), 415_139_642);
    assert_eq!(stepper.state.draw_count, 43);

    let mut checkpoint = checkpoint();
    checkpoint.next_frame = -1;
    let transitions = transitions()
        .into_iter()
        .enumerate()
        .map(|(i, mut transition)| {
            transition.frame = i as i32 - 1;
            transition
        });
    let report = validate(&mut stepper, &checkpoint, transitions, compare_observations).unwrap();
    assert_eq!((report.first_frame, report.last_frame), (-1, 1));
}

#[test]
fn reports_first_mismatch_without_consuming_the_rest_of_the_stream() {
    let mut stepper = stepper();
    let mut records = transitions();
    records[1].expected.velocity[1] = 9.5;
    let stream = records.into_iter().take(2).chain(std::iter::once_with(|| {
        panic!("validator consumed past the first mismatch")
    }));
    let error = validate(&mut stepper, &checkpoint(), stream, compare_observations).unwrap_err();
    match error {
        ValidationError::Mismatch {
            frame,
            checked_frames,
            difference,
        } => {
            assert_eq!((frame, checked_frames), (-122, 1));
            assert!(difference.contains("velocity[1]"), "{difference}");
            assert!(difference.contains("expected bits"), "{difference}");
        }
        other => panic!("unexpected error: {other}"),
    }
    assert_eq!(stepper.advance_calls, 2);
}

#[test]
fn empty_gapped_duplicate_and_out_of_order_streams_fail() {
    let mut stepper = stepper();
    let error = validate(&mut stepper, &checkpoint(), [], compare_observations).unwrap_err();
    assert!(matches!(error, ValidationError::Empty));
    assert_eq!(stepper.restore_calls, 0);
    for (index, frame, expected, checked) in [
        (0, -122, -123, 0),
        (1, -121, -122, 1),
        (1, -123, -122, 1),
        (1, -124, -122, 1),
    ] {
        let mut records = transitions();
        records[index].frame = frame;
        let error =
            validate(&mut stepper, &checkpoint(), records, compare_observations).unwrap_err();
        assert!(matches!(error, ValidationError::Sequence {
            expected: e, actual: a, checked_frames: c,
        } if (e, a, c) == (expected, frame, checked)));
        assert_eq!(stepper.advance_calls as u64, checked);
    }
}

#[test]
fn final_maximum_frame_is_valid_but_wrapping_is_rejected() {
    let mut stepper = stepper();
    let mut checkpoint = checkpoint();
    checkpoint.next_frame = i32::MAX;
    let mut records = transitions();
    records[0].frame = i32::MAX;
    records[1].frame = i32::MIN;
    let report = validate(
        &mut stepper,
        &checkpoint,
        [records[0].clone()],
        compare_observations,
    )
    .unwrap();
    assert_eq!(report.last_frame, i32::MAX);
    let error = validate(&mut stepper, &checkpoint, records, compare_observations).unwrap_err();
    assert!(matches!(
        error,
        ValidationError::FrameOverflow {
            after: i32::MAX,
            checked_frames: 1
        }
    ));
    assert_eq!(stepper.advance_calls, 1);
}

#[test]
fn restore_and_step_failures_preserve_the_underlying_error() {
    let mut stepper = stepper();
    stepper.fail_restore = true;
    let error = validate(
        &mut stepper,
        &checkpoint(),
        transitions(),
        compare_observations,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ValidationError::Restore {
            source: StepError::Restore
        }
    ));
    assert_eq!(stepper.advance_calls, 0);
    assert_eq!(
        error.source().unwrap().to_string(),
        "checkpoint unavailable"
    );
    stepper.fail_restore = false;
    let mut records = transitions();
    records[1].input = Input::Fail;
    let error = validate(&mut stepper, &checkpoint(), records, compare_observations).unwrap_err();
    assert!(matches!(
        error,
        ValidationError::Advance {
            frame: -122,
            checked_frames: 1,
            source: StepError::Advance,
        }
    ));
    assert_eq!(stepper.advance_calls, 2);
    assert_eq!(
        error.source().unwrap().to_string(),
        "requested step failure"
    );
}

#[test]
fn fallible_stream_errors_never_pass_as_a_matching_prefix() {
    let mut stepper = stepper();
    let read_failure = || {
        std::io::Error::new(
            std::io::ErrorKind::UnexpectedEof,
            "incomplete native record",
        )
    };
    let error = validate_fallible(
        &mut stepper,
        &checkpoint(),
        [Err(read_failure())],
        compare_observations,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ValidationError::Read {
            checked_frames: 0,
            ..
        }
    ));
    assert_eq!(stepper.restore_calls, 0);
    let stream = [
        Ok(transitions()[0].clone()),
        Err(read_failure()),
        Ok(transitions()[1].clone()),
    ];
    let error =
        validate_fallible(&mut stepper, &checkpoint(), stream, compare_observations).unwrap_err();
    assert!(matches!(
        error,
        ValidationError::Read {
            checked_frames: 1,
            ..
        }
    ));
    assert_eq!(stepper.advance_calls, 1);
    assert_eq!(
        error.source().unwrap().to_string(),
        "incomplete native record"
    );
}

#[test]
fn checkpoint_branches_are_independent_of_each_other_and_the_original() {
    let mut original = stepper();
    validate(
        &mut original,
        &checkpoint(),
        transitions(),
        compare_observations,
    )
    .unwrap();
    let saved = original.state;
    let mut left = branch_from(&original, &checkpoint()).unwrap();
    let mut right = branch_from(&original, &checkpoint()).unwrap();
    let left_result = left
        .advance(&Input::Fall {
            gravity: 1.0,
            terminal: 10.0,
        })
        .unwrap();
    let right_result = right.advance(&Input::FrictionAir(0.5)).unwrap();
    assert_eq!(left_result.velocity[1], 2.0);
    assert_eq!(right_result.velocity[1], 3.0);
    assert_eq!(left_result.seed, right_result.seed);
    assert_eq!(left_result.draw_count, 41);
    assert_eq!(original.state, saved);
    left.advance(&Input::ProjectGround).unwrap();
    assert_ne!(left.state.rng.seed(), right.state.rng.seed());
    assert_eq!(original.state, saved);
}

#[test]
fn bit_comparators_preserve_signed_zero_and_nan_payloads() {
    assert!(compare_f32_bits(&-0.0, &0.0).is_some());
    assert!(compare_f64_bits(&-0.0, &0.0).is_some());
    for bits in [0x7fc0_0001, 0x7f80_0001, 0xffc0_0001] {
        let value = f32::from_bits(bits);
        assert_eq!(compare_f32_bits(&value, &value), None);
        assert_eq!(
            compare_f32_bits(&value, &f32::from_bits(bits + 1)),
            Some(BitDifference {
                expected: bits,
                actual: bits + 1,
            })
        );
    }
    let nan = f64::from_bits(0x7ff8_0000_0000_0001);
    assert_eq!(compare_f64_bits(&nan, &nan), None);
    assert!(compare_f64_bits(&nan, &f64::from_bits(nan.to_bits() + 1)).is_some());
}
