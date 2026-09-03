pub mod fmt;

pub use fmt::play_count;

/// Format a duration in milliseconds as M:SS or H:MM:SS.
pub fn fmt_duration_ms(ms: u64) -> String {
    fmt::duration_ms(ms)
}
