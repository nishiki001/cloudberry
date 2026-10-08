//! Draws analyzer frames into a plain RGBA image (the UI wraps it in a `slint::Image`).
//! Cells keep a constant size at any width; the number of bands follows the width
//! (`bands_for`). Styles: Block, Bars, Mirror, Dots (cells), Curve (spline of the bands) and
//! Wave (oscilloscope of the raw samples).
use super::analyzer_curve::{sample_curve, wave_points};
use image::{Rgba, RgbaImage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Block,
    Bars,
    Wave,
    Curve,
    Mirror,
    Dots,
}

use super::analyzer_layout::axis;
pub use super::analyzer_layout::{MAX_BANDS, MIN_BANDS};
const BLOCK_H: u32 = 2;
const GAP: u32 = 1;
const DOT: u32 = 4;
const DOT_GAP: u32 = 2;

impl Style {
    /// `None` for "off" and anything unknown.
    pub fn parse(s: &str) -> Option<Style> {
        Some(match s {
            "block" => Style::Block,
            "bars" => Style::Bars,
            "wave" => Style::Wave,
            "curve" => Style::Curve,
            "mirror" => Style::Mirror,
            "dots" => Style::Dots,
            _ => return None,
        })
    }

    /// Cell width and the gap after it (constant at any window size).
    pub fn cell_gap(self) -> (u32, u32) {
        match self {
            Style::Block => (6, 1),
            Style::Bars | Style::Mirror => (4, 1),
            Style::Dots => (DOT, DOT_GAP),
            Style::Wave | Style::Curve => (5, 1),
        }
    }

    /// Horizontal distance between two cells (cell + gap).
    pub fn pitch(self) -> u32 {
        let (c, g) = self.cell_gap();
        c + g
    }
}

/// Bands and margins for a `width` px wide analyzer (the one layout function of all styles).
pub fn layout(style: Style, width: u32) -> super::analyzer_layout::Axis {
    let (cell, gap) = style.cell_gap();
    axis(cell, gap, width, MIN_BANDS, MAX_BANDS)
}

/// Number of bands that fit `width` px at the style's constant cell size.
pub fn bands_for(style: Style, width: u32) -> usize {
    layout(style, width).count
}

/// Data for one frame.
pub struct Frame<'a> {
    pub levels: &'a [f32],
    pub peaks: &'a [f32],
    /// Recent raw samples (Wave style).
    pub wave: &'a [f32],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub bg: [u8; 4],
    /// Gradient stops, bottom → top.
    pub stops: [[u8; 4]; 3],
    pub peak: [u8; 4],
}

pub fn lerp(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    std::array::from_fn(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8)
}

/// Colour at `t` (0 = bottom, 1 = top) along the three stops.
fn ramp(c: &Colors, t: f32) -> [u8; 4] {
    if t < 0.6 {
        lerp(c.stops[0], c.stops[1], t / 0.6)
    } else {
        lerp(c.stops[1], c.stops[2], (t - 0.6) / 0.4)
    }
}

fn fill(img: &mut RgbaImage, x: u32, y: u32, w: u32, h: u32, c: [u8; 4]) {
    for yy in y..(y + h).min(img.height()) {
        for xx in x..(x + w).min(img.width()) {
            img.put_pixel(xx, yy, Rgba(c));
        }
    }
}

fn dot(img: &mut RgbaImage, x: u32, y: u32, c: [u8; 4]) {
    let r = DOT as f32 / 2.0;
    for dy in 0..DOT {
        for dx in 0..DOT {
            let (fx, fy) = (dx as f32 + 0.5 - r, dy as f32 + 0.5 - r);
            if fx * fx + fy * fy <= r * r + 0.3 && x + dx < img.width() && y + dy < img.height() {
                img.put_pixel(x + dx, y + dy, Rgba(c));
            }
        }
    }
}

pub fn render(f: &Frame, style: Style, w: u32, h: u32, c: &Colors) -> RgbaImage {
    let (w, h) = (w.max(1), h.max(1));
    let mut img = RgbaImage::from_pixel(w, h, Rgba(c.bg));
    if h < BLOCK_H + GAP {
        return img;
    }
    let n = f.levels.len().min(f.peaks.len());
    let pitch = style.pitch();
    // edge to edge when the band count matches the layout (it lags by one resize at most)
    let lay = layout(style, w);
    let x0 = if n == lay.count {
        lay.margin
    } else {
        w.saturating_sub(pitch * n as u32) / 2
    };
    let well = lerp(c.bg, c.stops[0], 0.07);
    let lvl = |i: usize| f.levels[i].clamp(0.0, 1.0);
    let pk = |i: usize| f.peaks[i].clamp(0.0, 1.0);
    match style {
        Style::Block => {
            let v = axis(BLOCK_H, GAP, h, 1, usize::MAX);
            let (rows, top) = (v.count as u32, v.margin);
            for i in 0..n {
                let x = x0 + i as u32 * pitch;
                let lit = (lvl(i) * rows as f32).round() as u32;
                let cap = (pk(i) * rows as f32).round() as u32;
                for r in 0..rows {
                    let y = top + (rows - 1 - r) * (BLOCK_H + GAP);
                    let color = if r < lit {
                        ramp(c, r as f32 / rows as f32)
                    } else if cap > 0 && r + 1 == cap {
                        c.peak
                    } else {
                        well
                    };
                    fill(&mut img, x, y, pitch - GAP, BLOCK_H, color);
                }
            }
        }
        Style::Bars | Style::Mirror => {
            let mirror = style == Style::Mirror;
            let half = if mirror { h / 2 } else { h };
            let base = if mirror { half } else { h }; // y of the baseline
            for i in 0..n {
                let x = x0 + i as u32 * pitch;
                let bw = pitch - GAP;
                let bh = (lvl(i) * half as f32).round() as u32;
                for dy in 0..bh {
                    let col = ramp(c, dy as f32 / half as f32);
                    fill(&mut img, x, base - 1 - dy, bw, 1, col);
                    if mirror {
                        fill(&mut img, x, base + dy, bw, 1, lerp(col, c.bg, 0.55));
                    }
                }
                let pc = (pk(i) * half as f32).round() as u32;
                if pk(i) > 0.01 && pc > bh {
                    fill(&mut img, x, base - pc, bw, 1, c.peak);
                    if mirror {
                        fill(&mut img, x, base + pc - 1, bw, 1, lerp(c.peak, c.bg, 0.55));
                    }
                }
            }
        }
        Style::Dots => {
            let v = axis(DOT, DOT_GAP, h, 1, usize::MAX);
            let (rows, top) = (v.count as u32, v.margin);
            for i in 0..n {
                let x = x0 + i as u32 * pitch;
                let lit = (lvl(i) * rows as f32).round() as u32;
                let cap = (pk(i) * rows as f32).round() as u32;
                for r in 0..rows {
                    let y = top + (rows - 1 - r) * (DOT + DOT_GAP);
                    let color = if r < lit {
                        ramp(c, r as f32 / rows as f32)
                    } else if cap > 0 && r + 1 == cap {
                        c.peak
                    } else {
                        well
                    };
                    dot(&mut img, x, y, color);
                }
            }
        }
        Style::Curve => {
            let line = sample_curve(&f.levels[..n], w as usize);
            let top = sample_curve(&f.peaks[..n], w as usize);
            for (x, (&v, &p)) in line.iter().zip(&top).enumerate() {
                if v < 0.01 {
                    continue; // silence draws nothing, so the frame settles blank
                }
                let y = h - ((v * (h - 1) as f32).round() as u32).min(h - 1) - 1;
                for yy in y..h {
                    let t = (h - 1 - yy) as f32 / h as f32;
                    fill(&mut img, x as u32, yy, 1, 1, lerp(c.bg, ramp(c, t), 0.45));
                }
                fill(&mut img, x as u32, y, 1, 2, ramp(c, v));
                let py = h - ((p * (h - 1) as f32).round() as u32).min(h - 1) - 1;
                if p > 0.02 && py + 2 < y {
                    fill(&mut img, x as u32, py, 1, 1, c.peak);
                }
            }
        }
        Style::Wave => {
            let pts = wave_points(f.wave, w as usize);
            let mid = h as f32 / 2.0;
            fill(&mut img, 0, h / 2, w, 1, lerp(c.bg, c.stops[0], 0.2));
            let ys: Vec<f32> = pts
                .iter()
                .map(|v| mid - (v * 1.5).clamp(-1.0, 1.0) * (mid - 2.0))
                .collect();
            for x in 0..w as usize {
                let prev = if x > 0 { ys[x - 1] } else { ys[x] };
                let (a, b) = (prev.min(ys[x]), prev.max(ys[x]));
                let col = ramp(c, ((ys[x] - mid).abs() / mid).clamp(0.0, 1.0));
                let (y0, y1) = (a.round() as u32, b.round() as u32 + 2);
                fill(&mut img, x as u32, y0, 1, (y1 - y0).max(2), col);
            }
        }
    }
    img
}

#[cfg(test)]
#[path = "analyzer_render_tests.rs"]
mod tests;
