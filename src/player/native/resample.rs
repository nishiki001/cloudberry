//! Streaming stereo resampler (rubato FFT, fixed input chunk) with delay compensation.
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

const CHUNK: usize = 1024;

pub struct Resample {
    rs: Fft<f32>,
    pending: Vec<f32>,
    out_buf: Vec<f32>,
    /// Leading output frames to drop (filter delay).
    skip: usize,
}

impl Resample {
    pub fn new(from: u32, to: u32) -> Option<Self> {
        let rs = Fft::<f32>::new(from as usize, to as usize, CHUNK, 2, FixedSync::Input).ok()?;
        let out_buf = vec![0.0; rs.output_frames_max() * 2];
        let skip = rs.output_delay();
        Some(Self {
            rs,
            pending: Vec::new(),
            out_buf,
            skip,
        })
    }

    fn run(&mut self, out: &mut Vec<f32>) {
        loop {
            let need = self.rs.input_frames_next();
            if self.pending.len() < need * 2 {
                return;
            }
            let frames_out = self.out_buf.len() / 2;
            let (Ok(inp), Ok(mut outp)) = (
                InterleavedSlice::new(&self.pending[..need * 2], 2, need),
                InterleavedSlice::new_mut(&mut self.out_buf[..], 2, frames_out),
            ) else {
                return;
            };
            let Ok((used, produced)) = self.rs.process_into_buffer(&inp, &mut outp, None) else {
                self.pending.clear();
                return;
            };
            self.pending.drain(..used * 2);
            let mut start = 0;
            if self.skip > 0 {
                let d = self.skip.min(produced);
                self.skip -= d;
                start = d;
            }
            out.extend_from_slice(&self.out_buf[start * 2..produced * 2]);
        }
    }

    /// Feed interleaved stereo input; resampled output is appended to `out`.
    pub fn process(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.pending.extend_from_slice(input);
        self.run(out);
    }

    /// End of track: push zeros through so the tail comes out, then trim to the true length.
    pub fn finish(
        &mut self,
        total_in_frames: u64,
        from: u32,
        to: u32,
        produced_so_far: u64,
        out: &mut Vec<f32>,
    ) {
        let expect = (total_in_frames as f64 * to as f64 / from as f64).round() as u64;
        let zeros = vec![0.0f32; CHUNK * 2];
        let mut guard = 0;
        let before = out.len();
        while produced_so_far + ((out.len() - before) / 2) as u64 <= expect && guard < 8 {
            self.pending.extend_from_slice(&zeros);
            self.run(out);
            guard += 1;
        }
        let have = produced_so_far + ((out.len() - before) / 2) as u64;
        if have > expect {
            let extra = (have - expect) as usize;
            let keep = out.len().saturating_sub(extra * 2).max(before);
            out.truncate(keep);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_and_pitch_preserved() {
        let (from, to) = (44_100u32, 48_000u32);
        let frames = 44_100usize;
        let mut inp = Vec::new();
        for i in 0..frames {
            let v = (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / from as f32).sin() * 0.5;
            inp.push(v);
            inp.push(v);
        }
        let mut r = Resample::new(from, to).unwrap();
        let mut out = Vec::new();
        for c in inp.chunks(4096) {
            r.process(c, &mut out);
        }
        let produced = (out.len() / 2) as u64;
        r.finish(frames as u64, from, to, produced, &mut out);
        let got = out.len() / 2;
        assert!((got as i64 - 48_000).abs() <= 2, "got {got}");
        // 1 kHz tone: ~1000 zero crossings (positive-going) in one second
        let mut crossings = 0;
        for w in out.chunks(2).collect::<Vec<_>>().windows(2) {
            if w[0][0] <= 0.0 && w[1][0] > 0.0 {
                crossings += 1;
            }
        }
        assert!(
            (crossings as i64 - 1000).abs() <= 3,
            "crossings {crossings}"
        );
    }
}
