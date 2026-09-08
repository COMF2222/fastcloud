//! Lazy loader for SoundCloud's real waveform sample JSON.

use eframe::egui;
use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Waveforms are small, but search can expose a new URL on every row and the
/// old cache never released any of them. This keeps long sessions bounded.
const MAX_WAVEFORMS: usize = 128;

#[derive(Clone)]
pub struct WaveformLoader {
    inner: Arc<Inner>,
}

struct Inner {
    states: Mutex<HashMap<String, State>>,
    http: reqwest::Client,
    rt: tokio::runtime::Handle,
}

#[derive(Clone)]
enum State {
    Pending,
    Ready {
        samples: Arc<[f32]>,
        last_used: Instant,
    },
    Failed,
}

#[derive(Deserialize)]
struct WireWaveform {
    #[serde(default)]
    samples: Vec<f32>,
}

fn waveform_json_url(url: &str) -> String {
    let (path, query) = url
        .split_once('?')
        .map_or((url, None), |(path, query)| (path, Some(query)));
    let path = path
        .strip_suffix(".png")
        .map_or_else(|| path.to_owned(), |base| format!("{base}.json"));
    query.map_or(path.clone(), |query| format!("{path}?{query}"))
}

impl WaveformLoader {
    pub fn new(rt: tokio::runtime::Handle) -> Self {
        Self {
            inner: Arc::new(Inner {
                states: Mutex::new(HashMap::new()),
                http: reqwest::Client::new(),
                rt,
            }),
        }
    }

    /// Return cached samples, or start one background fetch and repaint when
    /// it completes. Bad/missing waveforms keep the deterministic fallback.
    pub fn get(&self, ctx: &egui::Context, url: &str) -> Option<Arc<[f32]>> {
        if url.is_empty() {
            return None;
        }
        {
            let mut states = self.inner.states.lock();
            match states.get_mut(url) {
                Some(State::Ready { samples, last_used }) => {
                    *last_used = Instant::now();
                    return Some(samples.clone());
                }
                Some(State::Pending | State::Failed) => return None,
                None => {
                    make_room(&mut states);
                    // Do not grow without a bound when every existing request
                    // is still in flight; a later frame retries naturally.
                    if states.len() >= MAX_WAVEFORMS {
                        return None;
                    }
                    states.insert(url.to_owned(), State::Pending);
                }
            }
        }

        let inner = self.inner.clone();
        let cache_key = url.to_owned();
        // The public track object still returns the legacy PNG URL. The same
        // CDN object with a `.json` suffix contains the 1,800 real samples.
        let request_url = waveform_json_url(url);
        let ctx = ctx.clone();
        self.inner.rt.spawn(async move {
            let state = match inner.http.get(&request_url).send().await {
                Ok(response) => match response.error_for_status() {
                    Ok(response) => match response.json::<WireWaveform>().await {
                        Ok(wire) if !wire.samples.is_empty() => {
                            let peak = wire
                                .samples
                                .iter()
                                .copied()
                                .map(f32::abs)
                                .fold(0.0_f32, f32::max);
                            let scale = peak.max(1.0);
                            let samples: Arc<[f32]> = wire
                                .samples
                                .into_iter()
                                .map(|sample| (sample.abs() / scale).clamp(0.01, 1.0))
                                .collect::<Vec<_>>()
                                .into();
                            State::Ready {
                                samples,
                                last_used: Instant::now(),
                            }
                        }
                        _ => State::Failed,
                    },
                    Err(_) => State::Failed,
                },
                Err(_) => State::Failed,
            };
            inner.states.lock().insert(cache_key, state);
            ctx.request_repaint();
        });
        None
    }
}

fn make_room(states: &mut HashMap<String, State>) {
    while states.len() >= MAX_WAVEFORMS {
        let victim = states
            .iter()
            .find_map(|(url, state)| matches!(state, State::Failed).then(|| url.clone()))
            .or_else(|| {
                states
                    .iter()
                    .filter_map(|(url, state)| match state {
                        State::Ready { last_used, .. } => Some((url.clone(), *last_used)),
                        State::Pending | State::Failed => None,
                    })
                    .min_by_key(|(_, last_used)| *last_used)
                    .map(|(url, _)| url)
            });
        let Some(victim) = victim else {
            break;
        };
        states.remove(&victim);
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_WAVEFORMS, State, make_room, waveform_json_url};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    #[test]
    fn upgrades_legacy_png_waveform_url() {
        assert_eq!(
            waveform_json_url("https://wave.sndcdn.com/example.png"),
            "https://wave.sndcdn.com/example.json"
        );
    }

    #[test]
    fn preserves_query_while_upgrading_waveform_url() {
        assert_eq!(
            waveform_json_url("https://wave.sndcdn.com/example.png?token=abc"),
            "https://wave.sndcdn.com/example.json?token=abc"
        );
    }

    #[test]
    fn leaves_modern_waveform_url_unchanged() {
        assert_eq!(
            waveform_json_url("https://wave.sndcdn.com/example.json"),
            "https://wave.sndcdn.com/example.json"
        );
    }

    #[test]
    fn waveform_cache_evicts_the_oldest_ready_entry() {
        let mut states = HashMap::new();
        for index in 0..MAX_WAVEFORMS {
            states.insert(
                index.to_string(),
                State::Ready {
                    samples: Arc::from([0.5]),
                    last_used: Instant::now() - Duration::from_secs((index + 1) as u64),
                },
            );
        }

        make_room(&mut states);

        assert_eq!(states.len(), MAX_WAVEFORMS - 1);
        assert!(!states.contains_key(&(MAX_WAVEFORMS - 1).to_string()));
    }
}
