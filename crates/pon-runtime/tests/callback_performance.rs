//! Manual performance comparison for the cached Pon callback bridge.
//!
//! Run explicitly in release mode. This is deliberately ignored in normal
//! test runs because machine load makes timing unsuitable for CI assertions.

use std::{hint::black_box, time::Instant};

use skirmish_pon_runtime::{Program, SourceBundle, Value};

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

#[test]
#[ignore = "manual release benchmark; timing is machine-dependent"]
fn bundled_module_root_recapture_profile() {
    let root = tempfile::tempdir().expect("temporary bundle root");
    let bundle = SourceBundle::new("callback-root-profile")
        .with_file("known.py", "value = 0\n")
        .expect("bundle source")
        .materialize(root.path())
        .expect("materialize bundle");
    let source =
        "import known\ndef touch(value):\n    known.value = [value]\n    return known.value[0]\n";
    let mut pon = Program::new(source, "callback-root-profile-main.py", ["touch"])
        .prepare_for_thread_in_bundle(&bundle)
        .expect("compile bundled callback");
    let callback = pon.callback_index("touch").expect("touch callback");

    for index in 0..WARMUP {
        assert_eq!(
            pon.invoke_index(callback, &[Value::Int(index as i64)])
                .expect("bundled callback warmup"),
            Value::Int(index as i64)
        );
    }
    let mut samples = Vec::with_capacity(SAMPLES);
    let mut checksum = 0i64;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for index in 0..ITERATIONS {
            let value = black_box((index % 97) as i64);
            let result = pon
                .invoke_index(callback, &[Value::Int(value)])
                .expect("bundled callback");
            checksum = checksum.wrapping_add(match result {
                Value::Int(value) => value,
                other => panic!("bundled callback returned {other:?}"),
            });
        }
        samples.push(start.elapsed());
    }
    eprintln!(
        "pon-bundled-root-profile pin=ab9067dbd2899c64c4d67a4bc27b8ad49472b126 iterations={} samples={} warmup={} median_ns={} samples_ns={:?} checksum={}",
        ITERATIONS,
        SAMPLES,
        WARMUP,
        median_nanos(&samples),
        samples
            .iter()
            .map(|sample| sample.as_nanos())
            .collect::<Vec<_>>(),
        checksum
    );

    // Isolate the changed operation from callback dispatch itself. Both arms
    // visit the same prepared `known` module and perform the same number of
    // recaptures. The snapshot arm retains the old Vec-producing API as the
    // A/B baseline; the visitor arm is the zero-temporary-vector path.
    let module = pon_runtime::import::cached_module(pon_runtime::intern("known"))
        .expect("prepared bundle must retain known module");
    const RECAPTURES: usize = 20_000;
    let mut snapshot_samples = Vec::with_capacity(SAMPLES);
    let mut visitor_samples = Vec::with_capacity(SAMPLES);
    let mut snapshot_checksum = 0usize;
    let mut visitor_checksum = 0usize;
    for _ in 0..SAMPLES {
        let start = Instant::now();
        for _ in 0..RECAPTURES {
            let values = pon_runtime::import::module_object_attr_values(module)
                .expect("known module attributes");
            snapshot_checksum = snapshot_checksum.wrapping_add(black_box(values.len()));
        }
        snapshot_samples.push(start.elapsed());

        let start = Instant::now();
        for _ in 0..RECAPTURES {
            let mut count = 0usize;
            // SAFETY: this visitor only increments a local counter and does
            // not re-enter the import or module mutation APIs.
            assert!(
                unsafe { pon_runtime::import::for_each_module_object_attr(module, |_| count += 1) }
                    .is_some()
            );
            visitor_checksum = visitor_checksum.wrapping_add(black_box(count));
        }
        visitor_samples.push(start.elapsed());
    }
    assert_eq!(snapshot_checksum, visitor_checksum);
    eprintln!(
        "pon-root-recapture-ab pin=ab9067dbd2899c64c4d67a4bc27b8ad49472b126 recaptures={} samples={} snapshot_median_ns={} snapshot_p95_ns={} visitor_median_ns={} visitor_p95_ns={} snapshot_samples_ns={:?} visitor_samples_ns={:?} scoped_allocations=unavailable",
        RECAPTURES,
        SAMPLES,
        median_nanos(&snapshot_samples),
        percentile_nanos(&snapshot_samples, 95),
        median_nanos(&visitor_samples),
        percentile_nanos(&visitor_samples, 95),
        snapshot_samples
            .iter()
            .map(|sample| sample.as_nanos())
            .collect::<Vec<_>>(),
        visitor_samples
            .iter()
            .map(|sample| sample.as_nanos())
            .collect::<Vec<_>>(),
    );
}

fn median_nanos(samples: &[std::time::Duration]) -> u128 {
    let mut values: Vec<_> = samples.iter().map(std::time::Duration::as_nanos).collect();
    values.sort_unstable();
    values[values.len() / 2]
}

fn percentile_nanos(samples: &[std::time::Duration], percentile: usize) -> u128 {
    assert!((1..=100).contains(&percentile));
    let mut values: Vec<_> = samples.iter().map(std::time::Duration::as_nanos).collect();
    values.sort_unstable();
    let index = (values.len() * percentile).div_ceil(100).saturating_sub(1);
    values[index]
}
