//! Isolated metadata memory probe, not a full UI/GPU benchmark.
//! Run: cargo run --release --manifest-path tools/collection-memory/Cargo.toml --locked

#[path = "../../../src/api/models.rs"]
mod models;

use std::hint::black_box;
use std::sync::Arc;

fn fixture(id: u64) -> serde_json::Result<models::Track> {
    serde_json::from_value(serde_json::json!({
        "id": id,
        "title": format!("Collection probe track {id:05} — evening session"),
        "duration": 240000,
        "artwork_url": format!("https://example.invalid/artwork/{id:05}-large.jpg"),
        "waveform_url": format!("https://example.invalid/waveforms/{id:05}.json"),
        "genre": "Electronic",
        "description": "Synthetic metadata only. No network requests. ".repeat(8),
        "user": {"id": id % 100, "username": "Collection probe artist"},
        "streamable": true
    }))
}

#[cfg(any(windows, test))]
fn parse_sample(output: &str) -> anyhow::Result<(u64, u64)> {
    let values: Vec<u64> = output
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    anyhow::ensure!(
        values.len() == 2,
        "expected working-set and private-byte counters"
    );
    Ok((values[0], values[1]))
}

#[cfg(windows)]
fn sample() -> anyhow::Result<(u64, u64)> {
    use std::os::windows::process::CommandExt;
    let script = format!(
        "$probe = Get-Process -Id {}; $probe.WorkingSet64; $probe.PrivateMemorySize64",
        std::process::id()
    );
    let output = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW: the sampler is not interactive.
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "memory sampler failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    parse_sample(std::str::from_utf8(&output.stdout)?)
}

#[cfg(not(windows))]
fn sample() -> anyhow::Result<(u64, u64)> {
    anyhow::bail!("This probe currently samples Windows process counters only")
}

fn report(stage: &str) -> anyhow::Result<()> {
    let (working_set, private_bytes) = sample()?;
    println!("{stage},{working_set},{private_bytes}");
    Ok(())
}

fn main() -> anyhow::Result<()> {
    anyhow::ensure!(!cfg!(debug_assertions), "run this probe with --release");
    // Warm up JSON parsing and process sampling before taking the baseline.
    black_box(fixture(0)?);
    sample()?;
    println!("stage,working_set_bytes,private_bytes");
    report("baseline")?;
    let mut tracks = Vec::with_capacity(10_000);
    for id in 0..10_000 {
        tracks.push(fixture(id)?);
    }
    black_box(&tracks);
    report("10000_tracks")?;
    let shared = Arc::new(tracks);
    let readers: Vec<_> = (0..100).map(|_| Arc::clone(&shared)).collect();
    black_box(&readers);
    report("100_arc_readers")?;
    let copied = shared.as_ref().clone();
    black_box(&copied);
    report("one_deep_copy")?;
    drop(copied);
    drop(readers);
    drop(shared);
    report("released")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_counters() {
        assert_eq!(parse_sample("1234\r\n5678\r\n").unwrap(), (1234, 5678));
    }

    #[test]
    fn rejects_incomplete_sample() {
        assert!(parse_sample("1234").is_err());
    }

    #[test]
    fn fixture_keeps_unique_track_ids() {
        assert_eq!(fixture(9999).unwrap().id, 9999);
    }
}
