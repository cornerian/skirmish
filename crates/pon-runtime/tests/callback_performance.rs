//! Manual performance comparison for the cached Pon callback bridge.
//!
//! Run explicitly in release mode. This is deliberately ignored in normal
//! test runs because machine load makes timing unsuitable for CI assertions.

use std::{hint::black_box, time::Instant};

use skirmish_pon_runtime::{Program, Value};

const ITERATIONS: usize = 20_000;
const SAMPLES: usize = 7;
const WARMUP: usize = 2_000;

#[test]
#[ignore = "manual release benchmark; timing is machine-dependent"]
fn cached_pon_callback_vs_rust_integer_event_transform() {
    let source = "class Fighter:\n    def action(self, value):\n        return value + 1\nfighter = Fighter()\naction = fighter.action\n";
    let mut pon = Program::new(source, "callback_performance.py", ["action"])
        .prepare_for_thread()
        .expect("compile cached Pon callback once");

    let mut pon_checksum = 0i64;
    let mut rust_checksum = 0i64;
    for index in 0..WARMUP {
        let input = (index % 97) as i64;
        pon_checksum += match pon.invoke("action", &[Value::Int(input)]) {
            Ok(Value::Int(value)) => value,
            other => panic!("Pon callback warmup returned {other:?}"),
        };
        rust_checksum += black_box(input + 1);
    }
    assert_eq!(pon_checksum, rust_checksum);

    let mut pon_samples = Vec::with_capacity(SAMPLES);
    let mut rust_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for index in 0..ITERATIONS {
            let input = black_box((index % 97) as i64);
            let output = match pon.invoke("action", &[Value::Int(input)]) {
                Ok(Value::Int(value)) => value,
                other => panic!("Pon callback returned {other:?}"),
            };
            pon_checksum = pon_checksum.wrapping_add(black_box(output));
        }
        pon_samples.push(start.elapsed());

        let start = Instant::now();
        for index in 0..ITERATIONS {
            let input = black_box((index % 97) as i64);
            rust_checksum = rust_checksum.wrapping_add(black_box(input + 1));
        }
        rust_samples.push(start.elapsed());
    }
    assert_eq!(pon_checksum, rust_checksum);

    let pon_median = median_nanos(&pon_samples);
    let rust_median = median_nanos(&rust_samples);
    let ratio = pon_median as f64 / rust_median.max(1) as f64;
    eprintln!(
        concat!(
            "pon-performance pin=ab9067dbd2899c64c4d67a4bc27b8ad49472b126 config=release ",
            "iterations={} samples={} warmup={} pon_ns={} rust_ns={} ratio={:.2}x ",
            "pon_samples_ns={:?} rust_samples_ns={:?} checksum={}"
        ),
        ITERATIONS,
        SAMPLES,
        WARMUP,
        pon_median,
        rust_median,
        ratio,
        pon_samples
            .iter()
            .map(|sample| sample.as_nanos())
            .collect::<Vec<_>>(),
        rust_samples
            .iter()
            .map(|sample| sample.as_nanos())
            .collect::<Vec<_>>(),
        pon_checksum
    );
}

fn median_nanos(samples: &[std::time::Duration]) -> u128 {
    let mut values: Vec<_> = samples.iter().map(std::time::Duration::as_nanos).collect();
    values.sort_unstable();
    values[values.len() / 2]
}
