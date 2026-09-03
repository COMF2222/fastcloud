#![allow(dead_code)]

use anyhow::{Context, Result};
use symphonia::core::audio::GenericAudioBufferRef;
use symphonia::core::codecs::CodecParameters;
use symphonia::core::codecs::audio::AudioDecoder;
use symphonia::core::codecs::registry::CodecRegistry;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, Track, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Decode a complete in-memory audio file (mp3 preview, cached full stream)
/// to interleaved stereo f32 at the source sample rate.
pub fn decode_all(bytes: Vec<u8>, mime: Option<&str>) -> Result<(Vec<f32>, u32)> {
    let (mut reader, track, mut decoder) = open(bytes, mime)?;
    let rate = track
        .codec_params
        .as_ref()
        .and_then(|p| match p {
            CodecParameters::Audio(a) => a.sample_rate,
            _ => None,
        })
        .unwrap_or(44_100);
    let mut out = Vec::new();
    while let Ok(Some(packet)) = reader.next_packet() {
        if packet.track_id != track.id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(d) => push_stereo(d, &mut out),
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    Ok((out, rate))
}

type OpenedStream = (Box<dyn FormatReader>, Track, Box<dyn AudioDecoder>);

fn open(bytes: Vec<u8>, mime: Option<&str>) -> Result<OpenedStream> {
    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if let Some(m) = mime {
        hint.mime_type(m);
        if m.contains("mpegurl") || m.contains("mp3") || m.contains("mpeg") {
            hint.with_extension("mp3");
        } else if m.contains("aac") {
            hint.with_extension("aac");
        } else if m.contains("mp4") {
            hint.with_extension("m4a");
        }
    } else {
        hint.with_extension("mp3");
    }
    let reader = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .context("probe audio stream")?;
    let track = reader
        .default_track(TrackType::Audio)
        .cloned()
        .context("no audio track")?;
    let params = match &track.codec_params {
        Some(CodecParameters::Audio(a)) => a.clone(),
        _ => anyhow::bail!("track is not audio"),
    };
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &Default::default())
        .context("init decoder")?;
    Ok((reader, track, decoder))
}

/// Append a decoded buffer to `out` as interleaved stereo f32.
fn push_stereo(decoded: GenericAudioBufferRef<'_>, out: &mut Vec<f32>) {
    let frames = decoded.frames();
    let chans = decoded.spec().channels().count();
    match chans {
        0 => {}
        1 => {
            let mut m = vec![0f32; frames];
            decoded.copy_to_vec_interleaved(&mut m);
            for v in m {
                out.push(v);
                out.push(v);
            }
        }
        _ => {
            let mut interleaved = vec![0f32; frames * chans];
            decoded.copy_to_vec_interleaved(&mut interleaved);
            let mut i = 0;
            while i + chans <= interleaved.len() {
                out.push(interleaved[i]);
                out.push(interleaved[i + 1]);
                i += chans;
            }
        }
    }
}

/// Concatenated HLS segment decoder: append bytes, pull decoded packets.
///
/// symphonia needs a full re-probe when new bytes arrive, so appended data is
/// buffered and the reader is rebuilt on exhaustion.
pub struct SegmentDecoder {
    bytes: Vec<u8>,
    mime: Option<String>,
    reader: Option<Box<dyn FormatReader>>,
    decoder: Option<Box<dyn AudioDecoder>>,
    track: Option<Track>,
    decoded_frames: u64,
    rate: u32,
    /// Bytes consumed by the previous reader generation; new readers skip them.
    consumed: usize,
    exhausted_current: bool,
}

impl SegmentDecoder {
    pub fn new(mime: Option<String>) -> Self {
        Self {
            bytes: Vec::new(),
            mime,
            reader: None,
            decoder: None,
            track: None,
            decoded_frames: 0,
            rate: 44_100,
            consumed: 0,
            exhausted_current: false,
        }
    }

    /// Append raw segment bytes (init segment first for fMP4).
    pub fn append(&mut self, data: &[u8]) {
        self.bytes.extend_from_slice(data);
        // Drop the previous reader; it will be rebuilt with more data on demand.
        self.reader = None;
        self.decoder = None;
        self.track = None;
        self.exhausted_current = false;
    }

    pub fn rate(&self) -> u32 {
        self.rate
    }

    pub fn decoded_frames(&self) -> u64 {
        self.decoded_frames
    }

    pub fn decoded_ms(&self) -> u64 {
        if self.rate == 0 {
            0
        } else {
            self.decoded_frames * 1000 / self.rate as u64
        }
    }

    fn ensure_open(&mut self) -> Result<()> {
        if self.reader.is_some() {
            return Ok(());
        }
        let remaining = self.bytes[self.consumed.min(self.bytes.len())..].to_vec();
        if remaining.is_empty() {
            anyhow::bail!("no data to decode");
        }
        let (reader, track, decoder) = open(remaining, self.mime.as_deref())?;
        self.rate = match &track.codec_params {
            Some(CodecParameters::Audio(a)) => a.sample_rate.unwrap_or(44_100),
            _ => 44_100,
        };
        self.reader = Some(reader);
        self.decoder = Some(decoder);
        self.track = Some(track);
        Ok(())
    }

    /// Pull one packet of interleaved stereo f32 samples.
    /// Ok(false) = out of appended data for now.
    pub fn next_packet(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        if self.exhausted_current {
            // Nothing more in the current reader; caller must append more.
            self.exhausted_current = false;
            return Ok(false);
        }
        loop {
            self.ensure_open()?;
            let reader = self.reader.as_mut().expect("open reader");
            let packet = match reader.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) | Err(symphonia::core::errors::Error::IoError(_)) => {
                    // Reached end of the appended window.
                    if let Some(r) = self.reader.take() {
                        // Track how much we've decoded: assume full window consumed
                        // for ADTS/MP3 streams (byte-exact for concatenated segments).
                        self.consumed = self.bytes.len();
                        let _ = r;
                    }
                    self.decoder = None;
                    self.track = None;
                    return Ok(false);
                }
                Err(e) => return Err(e.into()),
            };
            let Some(track) = self.track.clone() else {
                return Ok(false);
            };
            if packet.track_id != track.id {
                continue;
            }
            let Some(decoder) = self.decoder.as_mut() else {
                return Ok(false);
            };
            match decoder.decode(&packet) {
                Ok(d) => {
                    let frames = d.frames() as u64;
                    push_stereo(d, out);
                    self.decoded_frames += frames;
                    return Ok(true);
                }
                Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
                Err(e) => return Err(e.into()),
            }
        }
    }
}

#[allow(dead_code)]
fn _assert_send_sync(registry: &CodecRegistry) {
    fn is_send_sync<T: Send + Sync>() {}
    is_send_sync::<CodecRegistry>();
    let _ = registry;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_all_garbage_errors() {
        assert!(decode_all(vec![1, 2, 3, 4], None).is_err());
    }

    #[test]
    fn segment_decoder_empty_eof() {
        let mut dec = SegmentDecoder::new(None);
        let mut out = Vec::new();
        assert!(dec.next_packet(&mut out).is_err() || out.is_empty());
    }

    #[test]
    fn segment_decoder_reports_rate() {
        let dec = SegmentDecoder::new(Some("audio/mpeg".into()));
        assert_eq!(dec.rate(), 44_100);
        assert_eq!(dec.decoded_ms(), 0);
    }
}
