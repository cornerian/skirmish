//! Developer helper for recording known-image fingerprints. Not needed by players.
use std::{fs::File, io::Read, time::Instant};

fn main() -> anyhow::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("provide an ISO path"))?;
    let mut file = File::open(path)?;
    let mut hash = xxhash_rust::xxh3::Xxh3::new();
    let mut buffer = vec![0; 1024 * 1024];
    let start = Instant::now();
    let mut total = 0u64;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        hash.update(&buffer[..count]);
    }
    println!(
        "XXH3-128 {:032x}; {} bytes; {:.3}s",
        hash.digest128(),
        total,
        start.elapsed().as_secs_f64()
    );
    Ok(())
}
