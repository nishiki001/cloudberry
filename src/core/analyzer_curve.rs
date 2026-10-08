//! Sample maths for the Wave and Curve analyzer styles.

/// Reduce raw time-domain samples to `n` points in -1..=1 for the oscilloscope line: the
/// sample with the largest magnitude per bucket (so peaks stay visible), then a light 3-tap
/// smoothing. Silence or no input gives a flat line.
pub fn wave_points(samples: &[f32], n: usize) -> Vec<f32> {
    let n = n.max(2);
    if samples.is_empty() {
        return vec![0.0; n];
    }
    let picked: Vec<f32> = (0..n)
        .map(|i| {
            let lo = i * samples.len() / n;
            let hi = ((i + 1) * samples.len() / n).clamp(lo + 1, samples.len());
            samples[lo..hi]
                .iter()
                .copied()
                .max_by(|a, b| a.abs().total_cmp(&b.abs()))
                .unwrap_or(0.0)
        })
        .collect();
    (0..n)
        .map(|i| {
            let a = picked[i.saturating_sub(1)];
            let c = picked[(i + 1).min(n - 1)];
            (0.25 * a + 0.5 * picked[i] + 0.25 * c).clamp(-1.0, 1.0)
        })
        .collect()
}

/// Catmull-Rom spline through `p1` (u = 0) and `p2` (u = 1), tangents from the neighbours.
pub fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, u: f32) -> f32 {
    let (u2, u3) = (u * u, u * u * u);
    0.5 * (2.0 * p1
        + (p2 - p0) * u
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * u2
        + (3.0 * p1 - 3.0 * p2 + p3 - p0) * u3)
}

/// The spline through `points` evaluated at position `x` (0 = first point, n-1 = last).
pub fn curve_at(points: &[f32], x: f32) -> f32 {
    match points.len() {
        0 => return 0.0,
        1 => return points[0],
        _ => {}
    }
    let last = points.len() - 1;
    let x = x.clamp(0.0, last as f32);
    let i = (x.floor() as usize).min(last - 1);
    // beyond the ends the data is extrapolated along the end slope, so a straight line stays straight
    let at = |k: isize| match k {
        k if k < 0 => 2.0 * points[0] - points[1],
        k if k as usize > last => 2.0 * points[last] - points[last - 1],
        k => points[k as usize],
    };
    let i = i as isize;
    catmull_rom(at(i - 1), at(i), at(i + 1), at(i + 2), x - i as f32)
}

/// One value per pixel column, 0..=1 (the spline may overshoot between points).
pub fn sample_curve(points: &[f32], width: usize) -> Vec<f32> {
    let last = points.len().saturating_sub(1) as f32;
    (0..width)
        .map(|px| {
            let x = if width > 1 {
                px as f32 / (width - 1) as f32 * last
            } else {
                0.0
            };
            curve_at(points, x).clamp(0.0, 1.0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_follows_a_sine_and_silence_is_flat() {
        let s: Vec<f32> = (0..2048)
            .map(|i| (i as f32 / 2048.0 * std::f32::consts::TAU).sin() * 0.8)
            .collect();
        let w = wave_points(&s, 64);
        assert_eq!(w.len(), 64);
        for (i, v) in w.iter().enumerate() {
            let want = ((i as f32 + 0.5) / 64.0 * std::f32::consts::TAU).sin() * 0.8;
            assert!((v - want).abs() < 0.2, "{i}: {v} vs {want}");
        }
        assert!(wave_points(&[], 16).iter().all(|v| *v == 0.0));
        assert!(wave_points(&[0.0; 100], 16).iter().all(|v| *v == 0.0));
        // never leaves -1..1 even for clipped input
        assert!(
            wave_points(&[5.0, -5.0, 5.0, -5.0], 4)
                .iter()
                .all(|v| v.abs() <= 1.0)
        );
    }

    #[test]
    fn spline_passes_through_the_control_points() {
        let p = [0.1, 0.9, 0.2, 0.6, 0.3];
        for (i, v) in p.iter().enumerate() {
            assert!((curve_at(&p, i as f32) - v).abs() < 1e-5, "{i}");
        }
        // between two points it stays continuous
        let a = curve_at(&p, 1.999);
        let b = curve_at(&p, 2.001);
        assert!((a - b).abs() < 0.01);
    }

    #[test]
    fn straight_data_gives_a_straight_curve_and_output_is_clamped() {
        let line = [0.0, 0.25, 0.5, 0.75, 1.0];
        for x in [0.3f32, 1.5, 2.7, 3.9] {
            assert!((curve_at(&line, x) - x / 4.0).abs() < 1e-4, "{x}");
        }
        let spiky = [0.0, 1.0, 0.0, 1.0, 0.0];
        assert!(
            sample_curve(&spiky, 50)
                .iter()
                .all(|v| (0.0..=1.0).contains(v))
        );
        assert_eq!(sample_curve(&spiky, 50).len(), 50);
        assert_eq!(sample_curve(&[], 4), vec![0.0; 4]);
        assert_eq!(curve_at(&[0.7], 3.0), 0.7);
    }
}
