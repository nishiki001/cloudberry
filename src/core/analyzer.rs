//! Spectrum analyzer maths: Hann window → real FFT → log-spaced bands → dB → smoothing with
//! peak hold. Pure (no Slint, no audio device): feed it samples, read levels in 0..=1.
use realfft::{RealFftPlanner, RealToComplex};
use std::sync::Arc;

/// Samples kept for the Wave style.
pub const WAVE_LEN: usize = 2048;
/// FFT size: 4096, or 8192 when there are more than 256 bands (the lowest bands need the
/// frequency resolution).
pub fn fft_len_for(bands: usize) -> usize {
    if bands > 256 { 8192 } else { 4096 }
}
const F_MIN: f32 = 40.0;
const F_MAX: f32 = 16_000.0;
/// Level mapping: `DB_FLOOR` dBFS → 0.0, 0 dBFS → 1.0.
const DB_FLOOR: f32 = -62.0;
/// Seconds for a bar to fall from full scale to zero.
const FALL_SECS: f32 = 0.30;
/// Peak caps hold this long, then fall at `PEAK_FALL_PER_SEC`.
const WAVE_FADE_SECS: f32 = 0.12;
const PEAK_HOLD_SECS: f32 = 0.40;
const PEAK_FALL_PER_SEC: f32 = 0.9;

pub struct Analyzer {
    fft_len: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    window: Vec<f32>,
    input: Vec<f32>,
    spectrum: Vec<realfft::num_complex::Complex<f32>>,
    scratch: Vec<realfft::num_complex::Complex<f32>>,
    bins: Vec<Bin>,
    levels: Vec<f32>,
    peaks: Vec<f32>,
    hold: Vec<f32>,
    /// The newest raw samples (fading to silence when nothing plays), for the Wave style.
    wave: Vec<f32>,
}

/// Where a band reads the spectrum: the peak over its bins, or - when the band is narrower than
/// one bin (low bands of a very wide analyzer) - a linear interpolation between two bins, so
/// neighbouring columns do not show identical steps.
#[derive(Clone, Copy, Debug)]
enum Bin {
    Range(usize, usize),
    Interp(usize, f32),
}

/// Band edges in Hz, log-spaced between `F_MIN` and `F_MAX`.
pub fn band_edges(bands: usize) -> Vec<f32> {
    let n = bands.max(1);
    (0..=n)
        .map(|i| F_MIN * (F_MAX / F_MIN).powf(i as f32 / n as f32))
        .collect()
}

impl Analyzer {
    pub fn new(bands: usize, sample_rate: f32) -> Self {
        let bands = bands.clamp(
            super::analyzer_layout::MIN_BANDS,
            super::analyzer_layout::MAX_BANDS,
        );
        let fft_len = fft_len_for(bands);
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_len);
        let window = (0..fft_len)
            .map(|i| {
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (fft_len - 1) as f32).cos()
            })
            .collect();
        let bin_hz = sample_rate / fft_len as f32;
        let edges = band_edges(bands);
        let max_bin = fft_len / 2;
        let bins: Vec<Bin> = edges
            .windows(2)
            .map(|w| {
                let (fl, fh) = (w[0] / bin_hz, w[1] / bin_hz);
                if fh - fl >= 1.0 {
                    let lo = (fl.round() as usize).min(max_bin);
                    Bin::Range(lo, (fh.round() as usize).clamp(lo, max_bin))
                } else {
                    let c = ((fl + fh) / 2.0).min((max_bin - 1) as f32);
                    Bin::Interp(c.floor() as usize, c.fract())
                }
            })
            .collect();
        Self {
            fft_len,
            input: fft.make_input_vec(),
            spectrum: fft.make_output_vec(),
            scratch: fft.make_scratch_vec(),
            fft,
            window,
            bins,
            levels: vec![0.0; bands],
            peaks: vec![0.0; bands],
            hold: vec![0.0; bands],
            wave: Vec::new(),
        }
    }

    /// Samples per analysis window; feed the newest this many.
    pub fn fft_len(&self) -> usize {
        self.fft_len
    }
    pub fn bands(&self) -> usize {
        self.levels.len()
    }
    pub fn levels(&self) -> &[f32] {
        &self.levels
    }
    pub fn wave(&self) -> &[f32] {
        &self.wave
    }
    pub fn peaks(&self) -> &[f32] {
        &self.peaks
    }
    /// Frequency range covered by band `i`.
    #[cfg(test)]
    pub fn band_range_hz(&self, i: usize) -> (f32, f32) {
        let e = band_edges(self.levels.len());
        (e[i], e[i + 1])
    }
    /// All bars and caps are at rest: nothing left to animate.
    pub fn settled(&self) -> bool {
        self.levels.iter().chain(&self.peaks).all(|v| *v < 0.002)
            && self.wave.iter().all(|v| v.abs() < 0.002)
    }

    /// Instantaneous band levels (0..=1, no smoothing) for the newest `fft_len` samples.
    fn raw_levels(&mut self, samples: &[f32]) -> Vec<f32> {
        let n = self.fft_len;
        let start = samples.len().saturating_sub(n);
        let tail = &samples[start..];
        self.input.fill(0.0);
        let off = n - tail.len();
        for (i, s) in tail.iter().enumerate() {
            self.input[off + i] = s * self.window[off + i];
        }
        if self
            .fft
            .process_with_scratch(&mut self.input, &mut self.spectrum, &mut self.scratch)
            .is_err()
        {
            return vec![0.0; self.bands()];
        }
        // a full-scale sine peaks at N/4 with a Hann window
        let norm = 4.0 / n as f32;
        self.bins
            .iter()
            .map(|&bin| {
                let m = match bin {
                    Bin::Range(lo, hi) => (lo..=hi)
                        .map(|k| self.spectrum[k].norm())
                        .fold(0.0f32, f32::max),
                    Bin::Interp(i, f) => {
                        self.spectrum[i].norm() * (1.0 - f) + self.spectrum[i + 1].norm() * f
                    }
                } * norm;
                let db = 20.0 * m.max(1e-9).log10();
                ((db - DB_FLOOR) / -DB_FLOOR).clamp(0.0, 1.0)
            })
            .collect()
    }

    /// Advance by `dt` seconds with the newest samples (pass an empty slice for silence).
    pub fn update(&mut self, samples: &[f32], dt: f32) {
        let raw = if samples.is_empty() {
            vec![0.0; self.bands()]
        } else {
            self.raw_levels(samples)
        };
        if samples.is_empty() {
            let keep = (-dt / WAVE_FADE_SECS).exp();
            self.wave.iter_mut().for_each(|v| *v *= keep);
        } else {
            self.wave.clear();
            self.wave
                .extend_from_slice(&samples[samples.len().saturating_sub(WAVE_LEN)..]);
        }
        let fall = dt / FALL_SECS;
        for (i, &raw_i) in raw.iter().enumerate().take(self.levels.len()) {
            // fast attack, ~300 ms decay
            self.levels[i] = if raw_i >= self.levels[i] {
                raw_i
            } else {
                (self.levels[i] - fall).max(raw_i)
            };
            if self.levels[i] >= self.peaks[i] {
                self.peaks[i] = self.levels[i];
                self.hold[i] = PEAK_HOLD_SECS;
            } else if self.hold[i] > 0.0 {
                self.hold[i] -= dt;
            } else {
                self.peaks[i] = (self.peaks[i] - PEAK_FALL_PER_SEC * dt).max(self.levels[i]);
            }
        }
    }
}

#[cfg(test)]
#[path = "analyzer_tests.rs"]
mod tests;
