//! One track: symphonia demux + decode to interleaved stereo f32.
use anyhow::{Context, Result, anyhow};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo, TrackType};
use symphonia::core::io::{MediaSource, MediaSourceStream};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::units::Time;

pub struct TrackDecoder {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    pub sample_rate: u32,
    pub duration_secs: Option<f64>,
    scratch: Vec<f32>,
    /// Frames still to drop after an accurate seek.
    skip: usize,
    /// AAC (m4a) tracks start/end with encoder priming and padding: trim digital silence at
    /// both edges so consecutive tracks join without a gap.
    trim_edges: bool,
    lead_left: usize,
    /// Last few blocks, held back so the tail's padding silence can be trimmed at EOS.
    held: std::collections::VecDeque<Vec<f32>>,
}

const SILENT: f32 = 1e-4;
const MAX_LEAD_TRIM: usize = 3600;
const MAX_TAIL_TRIM: usize = 3600;
/// 4 × ~1024-frame blocks cover `MAX_TAIL_TRIM`.
const HOLD_BLOCKS: usize = 4;

fn silent_prefix(b: &[f32]) -> usize {
    b.chunks(2)
        .take_while(|f| f[0].abs() < SILENT && f[1].abs() < SILENT)
        .count()
}

fn silent_suffix(b: &[f32]) -> usize {
    b.chunks(2)
        .rev()
        .take_while(|f| f[0].abs() < SILENT && f[1].abs() < SILENT)
        .count()
}

impl TrackDecoder {
    pub fn open(src: Box<dyn MediaSource>, ext: &str) -> Result<Self> {
        let mss = MediaSourceStream::new(src, Default::default());
        let mut hint = Hint::new();
        if !ext.is_empty() {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                mss,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(|e| anyhow!("unsupported stream: {e}"))?;
        let track = format
            .default_track(TrackType::Audio)
            .context("no audio track")?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .context("no audio parameters")?;
        let sample_rate = params.sample_rate.context("unknown sample rate")?;
        let duration_secs = track
            .num_frames
            .map(|n| n as f64 / sample_rate as f64)
            .or_else(|| track.duration.map(|d| d.get() as f64 / sample_rate as f64));
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(|e| anyhow!("no decoder: {e}"))?;
        Ok(Self {
            track_id: track.id,
            format,
            decoder,
            sample_rate,
            duration_secs,
            scratch: Vec::new(),
            skip: 0,
            trim_edges: matches!(ext, "m4a" | "mp4" | "aac"),
            lead_left: if matches!(ext, "m4a" | "mp4" | "aac") {
                MAX_LEAD_TRIM
            } else {
                0
            },
            held: Default::default(),
        })
    }

    /// Seek to `secs`; the next blocks start exactly there (leading frames are dropped).
    pub fn seek(&mut self, secs: f64) -> Result<()> {
        let time = Time::try_from_secs_f64(secs.max(0.0)).context("bad seek time")?;
        let seeked = self
            .format
            .seek(
                SeekMode::Accurate,
                SeekTo::Time {
                    time,
                    track_id: Some(self.track_id),
                },
            )
            .map_err(|e| anyhow!("seek failed: {e}"))?;
        self.decoder.reset();
        self.held.clear();
        self.lead_left = 0;
        // required_ts - actual_ts is in the track's time base; for m4a that is 1/sample_rate
        let delta = seeked
            .required_ts
            .get()
            .saturating_sub(seeked.actual_ts.get())
            .max(0) as usize;
        self.skip = delta;
        Ok(())
    }

    /// Next block of decoded audio, interleaved stereo. `None` = end of stream.
    pub fn next_block(&mut self) -> Result<Option<Vec<f32>>> {
        loop {
            let Some(mut b) = self.read_block()? else {
                // end of stream: the held-back last block loses its padding silence
                let mut tail: Vec<f32> = self.held.drain(..).flatten().collect();
                let cut = silent_suffix(&tail).min(MAX_TAIL_TRIM);
                tail.truncate(tail.len() - cut * 2);
                return Ok((!tail.is_empty()).then_some(tail));
            };
            if self.lead_left > 0 {
                let n = silent_prefix(&b).min(self.lead_left);
                b.drain(..n * 2);
                // sound reached (block not fully silent): priming is over
                self.lead_left = if b.is_empty() { self.lead_left - n } else { 0 };
                if b.is_empty() {
                    continue;
                }
            }
            if !self.trim_edges {
                return Ok(Some(b));
            }
            self.held.push_back(b);
            if self.held.len() > HOLD_BLOCKS {
                return Ok(self.held.pop_front());
            }
        }
    }

    fn read_block(&mut self) -> Result<Option<Vec<f32>>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(p)) => p,
                Ok(None) => return Ok(None),
                Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    return Ok(None);
                }
                Err(e) => return Err(anyhow!("read error: {e}")),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            let audio = match self.decoder.decode(&packet) {
                Ok(a) => a,
                Err(SymError::DecodeError(_)) => continue, // skip a corrupt packet
                Err(e) => return Err(anyhow!("decode error: {e}")),
            };
            let channels = audio.spec().channels().count().max(1);
            self.scratch.resize(audio.samples_interleaved(), 0.0);
            audio.copy_to_slice_interleaved(&mut self.scratch);
            let frames = self.scratch.len() / channels;
            let mut out = Vec::with_capacity(frames * 2);
            for f in 0..frames {
                let l = self.scratch[f * channels];
                let r = if channels > 1 {
                    self.scratch[f * channels + 1]
                } else {
                    l
                };
                out.push(l);
                out.push(r);
            }
            if self.skip > 0 {
                let drop = self.skip.min(frames);
                self.skip -= drop;
                out.drain(..drop * 2);
                if out.is_empty() {
                    continue;
                }
            }
            return Ok(Some(out));
        }
    }
}

#[cfg(test)]
pub fn wav_bytes(rate: u32, secs: f64, freq: f64, amp: f32) -> Vec<u8> {
    let n = (rate as f64 * secs) as usize;
    let mut pcm = Vec::with_capacity(n * 4);
    for i in 0..n {
        let v = ((2.0 * std::f64::consts::PI * freq * i as f64 / rate as f64).sin() as f32
            * amp
            * 32767.0) as i16;
        pcm.extend_from_slice(&v.to_le_bytes());
        pcm.extend_from_slice(&v.to_le_bytes());
    }
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&2u16.to_le_bytes()); // stereo
    w.extend_from_slice(&rate.to_le_bytes());
    w.extend_from_slice(&(rate * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(&pcm);
    w
}

#[cfg(test)]
pub struct MemSource(pub std::io::Cursor<Vec<u8>>);

#[cfg(test)]
impl std::io::Read for MemSource {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(b)
    }
}
#[cfg(test)]
impl std::io::Seek for MemSource {
    fn seek(&mut self, p: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(p)
    }
}
#[cfg(test)]
impl MediaSource for MemSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.0.get_ref().len() as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(secs: f64) -> TrackDecoder {
        TrackDecoder::open(
            Box::new(MemSource(std::io::Cursor::new(wav_bytes(
                44100, secs, 440.0, 0.5,
            )))),
            "wav",
        )
        .unwrap()
    }

    #[test]
    fn decodes_whole_track() {
        let mut d = open(1.0);
        assert_eq!(d.sample_rate, 44100);
        let mut frames = 0;
        while let Some(b) = d.next_block().unwrap() {
            frames += b.len() / 2;
        }
        assert!((frames as i64 - 44100).abs() < 50, "frames={frames}");
        assert!((d.duration_secs.unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn silence_helpers_count_stereo_frames() {
        let v = [0.0, 0.0, 0.0, 0.00001, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0];
        assert_eq!(silent_prefix(&v), 2);
        assert_eq!(silent_suffix(&v), 2);
        assert_eq!(silent_prefix(&[0.5, 0.5]), 0);
    }

    #[test]
    fn seek_skips_to_position() {
        let mut d = open(2.0);
        d.seek(1.5).unwrap();
        let mut frames = 0;
        while let Some(b) = d.next_block().unwrap() {
            frames += b.len() / 2;
        }
        assert!((frames as i64 - 22050).abs() < 100, "frames={frames}");
    }
}
