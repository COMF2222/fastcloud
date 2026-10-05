use serde::Serialize;

// Reports are deliberately constructed from counts and states. Raw errors,
// URLs, paths, user profiles and audio metadata never enter this structure.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub version: &'static str,
    pub os: &'static str,
    pub architecture: &'static str,
    pub connection: String,
    pub queue_length: usize,
    pub playing: bool,
    pub loading: bool,
    pub error_stage: &'static str,
    pub sample_rate: u32,
    pub bitrate_kbps: u32,
    pub settings_version: u8,
}

pub fn error_stage(error: Option<&str>) -> &'static str {
    let Some(error) = error else { return "none"; };
    let error = error.to_lowercase();
    if error.contains("session") || error.contains("401") || error.contains("unauthorized") { "authentication" }
    else if error.contains("403") || error.contains("blocked") || error.contains("no stream") { "availability" }
    else if error.contains("429") || error.contains("rate limit") { "rate_limit" }
    else if error.contains("probe") || error.contains("decode") { "audio_decode" }
    else if error.contains("fetch streams") { "stream_lookup" }
    else if error.contains("stream") || error.contains("timeout") || error.contains("connect") { "audio_network" }
    else { "unknown" }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn classification_never_echoes_sensitive_errors() {
        let error = "fetch streams https://private/?token=SECRET Bearer PRIVATE C:\\Users\\owner";
        assert_eq!(error_stage(Some(error)), "stream_lookup");
        assert_eq!(error_stage(None), "none");
    }
}
