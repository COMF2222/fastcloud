#![allow(dead_code)]

use super::dsp::{Eq10, Limiter};
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::{Condvar, Mutex};
use std::sync::Arc;

/// Shared playback state consumed by the cpal audio callback.
pub struct OutputState {
    /// Interleaved stereo f32 samples at `source_rate`, produced by the decode thread.
    pub buffer: Vec<f32>,
    pub read_pos: usize,
    pub source_rate: u32,
    pub playing: bool,
    pub volume: f32,
    pub eq_gains: [f32; 10],
    pub eq_enabled: bool,
    pub limiter_threshold_db: f32,
}

impl Default for OutputState {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            read_pos: 0,
            source_rate: 44_100,
            playing: false,
            volume: 0.8,
            eq_gains: [0.0; 10],
            eq_enabled: false,
            limiter_threshold_db: -1.0,
        }
    }
}

/// Handle for pushing decoded samples into the cpal callback.
pub struct AudioOutput {
    state: Arc<Mutex<OutputState>>,
    cond: Arc<Condvar>,
    _stream: cpal::Stream,
    device_sample_rate: u32,
}

impl AudioOutput {
    pub fn open(default_gains: [f32; 10], default_volume: f32) -> Result<Self> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("no audio output device")?;
        let ranges: Vec<_> = device
            .supported_output_configs()
            .context("query output configs")?
            .collect();
        let range = *ranges
            .iter()
            .filter(|c| c.channels() >= 2)
            .find(|c| c.sample_format().is_float())
            .or_else(|| ranges.iter().find(|c| c.channels() >= 2))
            .context("no stereo output config")?;
        let device_rate = range.max_sample_rate().min(48_000);
        let config = range.with_sample_rate(device_rate);

        let state = Arc::new(Mutex::new(OutputState {
            eq_gains: default_gains,
            volume: default_volume,
            ..Default::default()
        }));
        let cond = Arc::new(Condvar::new());

        let st = state.clone();
        let config_rate = config.sample_rate();
        let mut eq = Eq10::new(config_rate as f32, default_gains);
        let mut limiter = Limiter::new(config_rate as f32, -1.0);
        let mut resample_pos = 0.0f64;

        let stream_config = config.config();
        let stream = device.build_output_stream(
            stream_config,
            move |data: &mut [f32], _| {
                let mut state = st.lock();
                let sr_ratio = state.source_rate as f64 / config_rate as f64;
                for frame in data.chunks_mut(2) {
                    let (l, r) = next_frame(&mut state, &mut resample_pos, sr_ratio);
                    let (mut l, mut r) = if state.eq_enabled {
                        eq.process_stereo(l, r)
                    } else {
                        (l, r)
                    };
                    let vol = state.volume;
                    l *= vol;
                    r *= vol;
                    let (l, r) = limiter.process(l, r);
                    if frame.len() == 2 {
                        frame[0] = l;
                        frame[1] = r;
                    } else if frame.len() == 1 {
                        frame[0] = (l + r) * 0.5;
                    }
                }
            },
            |err| log::error!("audio output error: {err}"),
            None,
        )?;
        stream.play()?;

        Ok(Self {
            state,
            cond,
            _stream: stream,
            device_sample_rate: config_rate,
        })
    }

    /// Replace the sample buffer (new track). `samples` are interleaved stereo at `rate`.
    pub fn start_track(&self, samples: Vec<f32>, rate: u32, playing: bool) {
        let mut s = self.state.lock();
        s.buffer = samples;
        s.read_pos = 0;
        s.source_rate = rate.max(1);
        s.playing = playing;
        self.cond.notify_all();
    }

    /// Append decoded samples for gapless continuation.
    pub fn append_samples(&self, samples: &[f32], rate: u32) {
        let mut s = self.state.lock();
        if s.source_rate == 0 {
            s.source_rate = rate.max(1);
        }
        s.buffer.extend_from_slice(samples);
        self.cond.notify_all();
    }

    pub fn set_playing(&self, playing: bool) {
        self.state.lock().playing = playing;
        self.cond.notify_all();
    }

    pub fn set_volume(&self, volume: f32) {
        self.state.lock().volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_eq(&self, enabled: bool, gains: [f32; 10]) {
        let mut s = self.state.lock();
        s.eq_enabled = enabled;
        s.eq_gains = gains;
    }

    /// Seek to a position given in track milliseconds.
    pub fn seek_ms(&self, ms: u64) {
        let mut s = self.state.lock();
        let frame = ms * s.source_rate as u64 / 1000 * 2;
        s.read_pos = frame.min(s.buffer.len() as u64) as usize;
    }

    /// Current playback position in ms (at source rate).
    pub fn position_ms(&self) -> u64 {
        let s = self.state.lock();
        (s.read_pos as u64 / 2) * 1000 / s.source_rate.max(1) as u64
    }

    pub fn buffered_ms(&self) -> u64 {
        let s = self.state.lock();
        let frames = (s.buffer.len().saturating_sub(s.read_pos)) / 2;
        frames as u64 * 1000 / s.source_rate.max(1) as u64
    }

    pub fn is_playing(&self) -> bool {
        self.state.lock().playing
    }

    /// Block until data is available or the timeout elapses.
    pub fn underrun_wait(&self, timeout: std::time::Duration) -> bool {
        let mut s = self.state.lock();
        loop {
            let avail = s.buffer.len().saturating_sub(s.read_pos);
            if avail > 0 || !s.playing {
                return true;
            }
            let result = self.cond.wait_for(&mut s, timeout);
            if result.timed_out() {
                return false;
            }
        }
    }

    pub fn device_sample_rate(&self) -> u32 {
        self.device_sample_rate
    }
}

/// Linear-interpolation resampling frame pull.
#[inline]
fn next_frame(state: &mut OutputState, pos: &mut f64, sr_ratio: f64) -> (f32, f32) {
    let len = state.buffer.len();
    if state.source_rate == 0 || len < 4 {
        return (0.0, 0.0);
    }
    let idx = *pos as usize;
    if idx + 3 >= len {
        // Starved: output silence (decode thread will append more).
        return (0.0, 0.0);
    }
    let frac = (*pos - (*pos as usize) as f64) as f32;
    let l = state.buffer[idx] + (state.buffer[idx + 2] - state.buffer[idx]) * frac;
    let r = state.buffer[idx + 1] + (state.buffer[idx + 3] - state.buffer[idx + 1]) * frac;
    *pos += sr_ratio;
    (l, r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_defaults() {
        let s = OutputState::default();
        assert_eq!(s.eq_gains.len(), 10);
        assert_eq!(s.source_rate, 44_100);
        assert!(!s.playing);
    }
}
