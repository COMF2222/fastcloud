use crate::{AppState, auth};
use parking_lot::Mutex;
use serde::Deserialize;
use std::time::Duration;
use tauri::Emitter;

#[derive(Default)]
pub struct Subscription {
    task: Mutex<Option<(String, tokio::task::JoinHandle<()>)>>,
}

#[tauri::command]
pub fn subscribe_updates(
    server_url: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    subscription: tauri::State<'_, Subscription>,
) -> Result<(), String> {
    let origin = auth::checked_server_url(&server_url).map_err(|error| error.to_string())?;
    let mut task = subscription.task.lock();
    if let Some((previous, handle)) = task.as_ref()
        && previous == &origin
        && !handle.is_finished()
    {
        return Ok(());
    }
    if let Some((_, handle)) = task.take() {
        handle.abort();
    }
    let handle = state
        .rt
        .spawn(listen(app, format!("{origin}/v1/updates/events")));
    *task = Some((origin, handle));
    Ok(())
}

async fn listen(app: tauri::AppHandle, endpoint: String) {
    let Ok(client) = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .build()
    else {
        log::warn!("Could not start release notification connection");
        return;
    };
    let mut last_version = String::new();
    let mut delay = 3_u64;
    loop {
        let request = client.get(&endpoint).header("Accept", "text/event-stream");
        if let Ok(Ok(mut response)) =
            tokio::time::timeout(Duration::from_secs(15), request.send()).await
            && response.status().is_success()
            && response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|value| value.starts_with("text/event-stream"))
        {
            delay = 3;
            let mut decoder = Decoder::default();
            loop {
                let Ok(Ok(Some(chunk))) =
                    tokio::time::timeout(Duration::from_secs(45), response.chunk()).await
                else {
                    break;
                };
                if decoder
                    .feed(&chunk, |version| {
                        if version != last_version {
                            // The frontend still verifies the signed update manifest.
                            if app.emit_to("main", "release-published", &version).is_ok() {
                                last_version = version;
                            }
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        }
        // Heartbeats detect dead connections; reconnects never query GitHub.
        tokio::time::sleep(Duration::from_secs(delay + rand::random_range(0..3))).await;
        delay = (delay * 2).min(60);
    }
}

#[derive(Deserialize)]
struct Release {
    version: String,
}

#[derive(Default)]
struct Decoder {
    pending: Vec<u8>,
    event: String,
    data: String,
}

impl Decoder {
    fn feed(&mut self, bytes: &[u8], mut release: impl FnMut(String)) -> Result<(), &'static str> {
        self.pending.extend_from_slice(bytes);
        if self.pending.len() > 8192 {
            return Err("release event is too large");
        }
        while let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8(self.pending.drain(..=end).collect())
                .map_err(|_| "release event is not UTF-8")?;
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if self.event == "release"
                    && let Ok(value) = serde_json::from_str::<Release>(&self.data)
                    && release_version(&value.version)
                {
                    release(value.version);
                }
                self.event.clear();
                self.data.clear();
            } else if let Some(value) = line.strip_prefix("event:") {
                self.event = value.trim().to_owned();
            } else if let Some(value) = line.strip_prefix("data:") {
                if self.data.len() + value.len() > 8192 {
                    return Err("release event is too large");
                }
                self.data.push_str(value.trim_start());
                self.data.push('\n');
            }
        }
        Ok(())
    }
}

fn release_version(version: &str) -> bool {
    let (core, suffix) = version
        .split_once('-')
        .map_or((version, None), |(core, suffix)| (core, Some(suffix)));
    version.len() <= 32
        && suffix.is_none_or(|value| value.len() == 1 && value.as_bytes()[0].is_ascii_lowercase())
        && core.split('.').count() == 3
        && core.split('.').all(|part| {
            !part.is_empty()
                && part.bytes().all(|byte| byte.is_ascii_digit())
                && (part == "0" || !part.starts_with('0'))
                && part.parse::<u32>().is_ok()
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_survives_fragmented_chunks_and_heartbeats() {
        let mut decoder = Decoder::default();
        let mut received = Vec::new();
        for chunk in b": heartbeat\r\n\r\nevent: release\r\ndata: {\"version\":\"0.2.1-a\"}\r\n\r\n"
            .chunks(3)
        {
            decoder
                .feed(chunk, |version| received.push(version))
                .unwrap();
        }
        assert_eq!(received, vec!["0.2.1-a"]);
    }

    #[test]
    fn invalid_payloads_do_not_become_update_notifications() {
        let mut decoder = Decoder::default();
        let mut received = Vec::new();
        decoder.feed(b"event: release\ndata: {\"version\":\"bad\"}\n\nevent: other\ndata: {\"version\":\"0.2.1\"}\n\n", |version| received.push(version)).unwrap();
        assert!(received.is_empty());
    }

    #[test]
    fn unterminated_events_cannot_grow_without_bound() {
        assert!(Decoder::default().feed(&[b'x'; 8193], |_| {}).is_err());
    }
}
