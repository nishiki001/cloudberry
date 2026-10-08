//! Audio output: a lock-free ring (rtrb) feeds the cpal callback (or a null/capture sink).
//! The callback applies volume, honors pause and flush requests, counts played frames and
//! writes a mono tap for the analyzer.
use anyhow::{Context, Result, anyhow};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

pub const TAP_LEN: usize = 8192;
/// ~3 s of stereo audio at 48 kHz.
const RING_SAMPLES: usize = 48_000 * 2 * 3;

/// Last `TAP_LEN` mono samples (pre-volume), readable without locks.
pub struct Tap {
    buf: Vec<AtomicU32>,
    head: AtomicUsize,
}

impl Tap {
    fn new() -> Self {
        Self {
            buf: (0..TAP_LEN).map(|_| AtomicU32::new(0)).collect(),
            head: AtomicUsize::new(0),
        }
    }
    fn push(&self, v: f32) {
        let h = self.head.load(Ordering::Relaxed);
        self.buf[h % TAP_LEN].store(v.to_bits(), Ordering::Relaxed);
        self.head.store(h.wrapping_add(1), Ordering::Release);
    }
    /// Oldest → newest. Slightly torn reads are harmless for visualisation.
    pub fn snapshot(&self, out: &mut [f32; TAP_LEN]) {
        let h = self.head.load(Ordering::Acquire);
        for (i, o) in out.iter_mut().enumerate() {
            *o = f32::from_bits(self.buf[(h.wrapping_add(i)) % TAP_LEN].load(Ordering::Relaxed));
        }
    }
}

pub struct Shared {
    pub rate: u32,
    /// Frames consumed by the sink (flushed frames count as consumed).
    pub played: AtomicU64,
    pub paused: AtomicBool,
    volume: AtomicU32,
    /// Id of the track whose Start mark the sink has reached (set by the position thread).
    pub audible: AtomicU64,
    pub flush_req: AtomicU64,
    flush_seen: AtomicU64,
    pub tap: Tap,
    /// Test sink: everything the "device" consumed.
    pub capture: Mutex<Option<Vec<f32>>>,
}

impl Shared {
    fn new(rate: u32) -> Arc<Self> {
        Arc::new(Self {
            rate,
            played: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
            audible: AtomicU64::new(0),
            flush_req: AtomicU64::new(0),
            flush_seen: AtomicU64::new(0),
            tap: Tap::new(),
            capture: Mutex::new(None),
        })
    }
    pub fn set_volume(&self, v: f32) {
        self.volume
            .store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    /// Ask the sink to drop everything buffered; returns when it has (or after a timeout).
    pub fn flush(&self) {
        let req = self.flush_req.fetch_add(1, Ordering::SeqCst) + 1;
        for _ in 0..400 {
            if self.flush_seen.load(Ordering::SeqCst) >= req {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
}

/// Sink side of the ring: shared by the cpal callback and the null/capture thread.
struct Drain {
    cons: Consumer<f32>,
    shared: Arc<Shared>,
    /// Test sink: record everything (the device callback never takes this lock).
    capture: bool,
}

impl Drain {
    /// Fill `out` (interleaved, `channels` per frame) from the ring.
    fn fill(&mut self, out: &mut [f32], channels: usize) {
        let sh = &self.shared;
        let req = sh.flush_req.load(Ordering::SeqCst);
        if req != sh.flush_seen.load(Ordering::SeqCst) {
            let mut n = 0u64;
            while self.cons.pop().is_ok() {
                n += 1;
            }
            sh.played.fetch_add(n / 2, Ordering::SeqCst);
            sh.flush_seen.store(req, Ordering::SeqCst);
        }
        if sh.paused.load(Ordering::Relaxed) {
            out.fill(0.0);
            return;
        }
        let vol = f32::from_bits(sh.volume.load(Ordering::Relaxed));
        let frames_wanted = out.len() / channels;
        let avail = self.cons.slots() / 2;
        let frames = frames_wanted.min(avail);
        let mut cap = if self.capture {
            sh.capture.lock().ok()
        } else {
            None
        };
        for f in 0..frames {
            let l = self.cons.pop().unwrap_or(0.0);
            let r = self.cons.pop().unwrap_or(0.0);
            sh.tap.push((l + r) * 0.5);
            let o = &mut out[f * channels..(f + 1) * channels];
            o[0] = if channels > 1 {
                l * vol
            } else {
                (l + r) * 0.5 * vol
            };
            if channels > 1 {
                o[1] = r * vol;
                o[2..].fill(0.0);
            }
            if let Some(Some(c)) = cap.as_deref_mut() {
                c.push(l);
                c.push(r);
            }
        }
        out[frames * channels..].fill(0.0);
        if frames > 0 {
            sh.played.fetch_add(frames as u64, Ordering::SeqCst);
        }
    }
}

pub enum SinkKind {
    /// The default audio device.
    Device,
    /// Real-time pace, discards audio (APP_AO=null).
    Null,
    /// As fast as possible, records everything (tests).
    Capture,
}

/// Keeps the output alive; dropping it stops the stream / thread.
pub struct OutputGuard {
    _stream: Option<cpal::Stream>,
    stop: Arc<AtomicBool>,
}

impl Drop for OutputGuard {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

pub fn open(kind: SinkKind) -> Result<(Producer<f32>, Arc<Shared>, OutputGuard)> {
    let (prod, cons) = RingBuffer::<f32>::new(RING_SAMPLES);
    let stop = Arc::new(AtomicBool::new(false));
    match kind {
        SinkKind::Device => {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .context("no audio output device")?;
            let cfg = device
                .default_output_config()
                .map_err(|e| anyhow!("output config: {e}"))?;
            let channels = cfg.channels() as usize;
            let rate = cfg.sample_rate();
            let shared = Shared::new(rate);
            let mut drain = Drain {
                cons,
                shared: shared.clone(),
                capture: false,
            };
            let stream_cfg: cpal::StreamConfig = cfg.config();
            let stream = match cfg.sample_format() {
                cpal::SampleFormat::F32 => {
                    let mut tmp: Vec<f32> = Vec::new();
                    device.build_output_stream(
                        stream_cfg,
                        move |out: &mut [f32], _| {
                            let _ = &mut tmp;
                            drain.fill(out, channels);
                        },
                        |e| tracing::warn!("audio stream error: {e}"),
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let mut tmp: Vec<f32> = Vec::new();
                    device.build_output_stream(
                        stream_cfg,
                        move |out: &mut [i16], _| {
                            tmp.resize(out.len(), 0.0);
                            drain.fill(&mut tmp, channels);
                            for (o, s) in out.iter_mut().zip(&tmp) {
                                *o = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                            }
                        },
                        |e| tracing::warn!("audio stream error: {e}"),
                        None,
                    )
                }
                other => return Err(anyhow!("unsupported output sample format {other:?}")),
            }
            .map_err(|e| anyhow!("open audio stream: {e}"))?;
            stream
                .play()
                .map_err(|e| anyhow!("start audio stream: {e}"))?;
            Ok((
                prod,
                shared,
                OutputGuard {
                    _stream: Some(stream),
                    stop,
                },
            ))
        }
        SinkKind::Null | SinkKind::Capture => {
            let capture = matches!(kind, SinkKind::Capture);
            let shared = Shared::new(44_100);
            if capture {
                *shared.capture.lock().unwrap() = Some(Vec::new());
            }
            let mut drain = Drain {
                cons,
                shared: shared.clone(),
                capture,
            };
            let stop2 = stop.clone();
            std::thread::Builder::new()
                .name("null-sink".into())
                .spawn(move || {
                    let mut buf = vec![0.0f32; 441 * 2]; // 10 ms
                    let mut last = std::time::Instant::now();
                    while !stop2.load(Ordering::Relaxed) {
                        if capture {
                            // fast: take whatever is there, then yield
                            drain.fill(&mut buf, 2);
                            std::thread::sleep(std::time::Duration::from_micros(200));
                        } else {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            let dt = last.elapsed().as_secs_f64();
                            last = std::time::Instant::now();
                            let frames = ((dt * 44_100.0) as usize).clamp(1, 44_100);
                            buf.resize(frames * 2, 0.0);
                            drain.fill(&mut buf, 2);
                        }
                    }
                })
                .context("spawn null sink")?;
            Ok((
                prod,
                shared,
                OutputGuard {
                    _stream: None,
                    stop,
                },
            ))
        }
    }
}
