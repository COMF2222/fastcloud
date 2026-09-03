#![allow(dead_code)]

use crate::api::models::{Me, Track, UserLite};

/// Offline demo library used when `--demo` is passed (no credentials needed).
pub fn demo_tracks() -> Vec<Track> {
    let artist = UserLite {
        id: 1,
        username: "SoundCloud Demo".into(),
        permalink: Some("demo".into()),
        avatar_url: None,
    };
    let titles = [
        "Northern Lights",
        "Sunset Drive",
        "Rain on Glass",
        "Neon District",
        "Paper Planes",
        "Golden Hour",
        "Static Fields",
        "Low Tide",
        "Concrete Garden",
        "Afterglow",
    ];
    titles
        .iter()
        .enumerate()
        .map(|(i, title)| Track {
            id: 1000 + i as u64,
            title: (*title).into(),
            duration_ms: Some(120_000 + i as u64 * 13_000),
            full_duration_ms: Some(120_000 + i as u64 * 13_000),
            artwork: None,
            user: Some(artist.clone()),
            playback_count: Some(12_345 * (i as u64 + 1)),
            favoritings_count: Some(100 * (i as u64 + 1)),
            likes_count: Some(100 * (i as u64 + 1)),
            comment_count: Some(12),
            streamable: true,
            downloadable: false,
            preview_start_ms: None,
            preview_end_ms: None,
            genre: Some("ambient".into()),
            description: Some("Demo track generated offline.".into()),
            created_at: Some("2025-01-01T00:00:00Z".into()),
            permalink_url: None,
        })
        .collect()
}

pub fn demo_me() -> Me {
    Me {
        id: 1,
        username: "Demo Listener".into(),
        permalink: Some("demo-listener".into()),
        avatar_url: None,
        followers_count: Some(0),
    }
}

/// Synthesize a chord pad at 44.1kHz stereo for demo playback.
/// Duration matches the requested track so seek/progress are consistent.
pub fn demo_pcm_for(track: &crate::api::models::Track) -> Vec<f32> {
    let rate = 44_100f32;
    let secs = (track.effective_duration_ms() as f32 / 1000.0).clamp(2.0, 400.0);
    let n = (rate * secs) as usize;
    let base = 220.0 * 2f32.powf((track.id % 5) as f32 / 12.0 * 3.0);
    let chord = [1.0, 1.25, 1.5, 2.0];
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let t = i as f32 / rate;
        let mut sample = 0.0f32;
        for (ci, &ratio) in chord.iter().enumerate() {
            let f = base * ratio;
            let lfo = 0.5 + 0.5 * (2.0 * std::f32::consts::PI * (0.1 + 0.03 * ci as f32) * t).sin();
            sample += lfo * (2.0 * std::f32::consts::PI * f * t).sin() / chord.len() as f32;
        }
        let fade = (t / 2.0).min(1.0).min((secs - t) / 2.0).clamp(0.0, 1.0);
        let v = sample * fade * 0.5;
        out.push(v);
        out.push(v);
    }
    out
}

/// Legacy helper: 30s pad for a track id.
pub fn demo_pcm(track_id: u64) -> Vec<f32> {
    let mut t = demo_track_stub();
    t.id = track_id;
    demo_pcm_for(&t)
}

fn demo_track_stub() -> crate::api::models::Track {
    use crate::api::models::*;
    Track {
        id: 0,
        title: String::new(),
        duration_ms: Some(30_000),
        full_duration_ms: Some(30_000),
        artwork: None,
        user: None,
        playback_count: None,
        favoritings_count: None,
        likes_count: None,
        comment_count: None,
        streamable: false,
        downloadable: false,
        preview_start_ms: None,
        preview_end_ms: None,
        genre: None,
        description: None,
        created_at: None,
        permalink_url: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ten_demo_tracks() {
        let tracks = demo_tracks();
        assert_eq!(tracks.len(), 10);
        assert!(tracks[0].streamable);
    }

    #[test]
    fn demo_pcm_is_stereo_and_bounded() {
        let pcm = demo_pcm(1000);
        assert_eq!(pcm.len() % 2, 0);
        assert!(pcm.iter().all(|&v| v.abs() <= 1.0));
    }

    #[test]
    fn demo_pcm_has_energy() {
        let pcm = demo_pcm(1002);
        let peak = pcm.iter().fold(0.0f32, |a, &v| a.max(v.abs()));
        assert!(peak > 0.1, "demo audio must be audible: {peak}");
    }
}
