/// Format a duration in milliseconds as `M:SS` or `H:MM:SS`.
pub fn duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// Format a play count like "1.2K" / "3.4M".
pub fn play_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration_ms(0), "0:00");
        assert_eq!(duration_ms(61_000), "1:01");
        assert_eq!(duration_ms(3_661_000), "1:01:01");
    }

    #[test]
    fn counts() {
        assert_eq!(play_count(999), "999");
        assert_eq!(play_count(1_500), "1.5K");
        assert_eq!(play_count(2_500_000), "2.5M");
    }
}
