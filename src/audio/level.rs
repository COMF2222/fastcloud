/// Estimate a stable per-recording gain from the decoded opening. Silence is
/// never boosted; the gain is capped at +6 dB and leaves peak headroom.
pub fn recording_gain(samples: &[f32]) -> f32 {
    if samples.is_empty() { return 1.0; }
    let mut energy = 0.0f64;
    let mut peak = 0.0f32;
    let mut count = 0;
    for &sample in samples {
        if sample.is_finite() { energy += f64::from(sample) * f64::from(sample); peak = peak.max(sample.abs()); count += 1; }
    }
    if count == 0 || peak < 0.001 { return 1.0; }
    let rms = (energy / f64::from(count)).sqrt() as f32;
    (0.12 / rms.max(0.001)).clamp(0.25, 2.0).min(0.9 / peak.max(0.001))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quiet_and_loud_recordings_move_toward_the_same_level_without_clipping() {
        let quiet = vec![0.08; 500]; let loud = vec![0.4; 500];
        assert!((quiet[0] * recording_gain(&quiet) - loud[0] * recording_gain(&loud)).abs() < 0.001);
        assert!(recording_gain(&vec![0.01; 500]) <= 2.0);
        assert_eq!(recording_gain(&vec![0.0; 500]), 1.0);
        assert!(recording_gain(&[1.0, -0.1]) <= 0.9);
    }
}
