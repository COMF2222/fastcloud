pub mod fmt;

pub use fmt::play_count;

/// Format a duration in milliseconds as M:SS or H:MM:SS.
pub fn fmt_duration_ms(ms: u64) -> String {
    fmt::duration_ms(ms)
}

/// Format bytes like "12.4 MiB".
pub fn fmt_bytes(n: u64) -> String {
    fmt::bytes(n)
}
