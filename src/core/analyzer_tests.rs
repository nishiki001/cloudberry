use super::*;

fn sine(freq: f32, amp: f32, rate: f32) -> Vec<f32> {
    (0..8192)
        .map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / rate).sin() * amp)
        .collect()
}

fn argmax(v: &[f32]) -> usize {
    v.iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap()
        .0
}

#[test]
fn one_khz_sine_peaks_in_the_right_band() {
    let mut a = Analyzer::new(32, 44_100.0);
    a.update(&sine(1000.0, 0.9, 44_100.0), 0.033);
    let i = argmax(a.levels());
    let (lo, hi) = a.band_range_hz(i);
    assert!(
        lo <= 1000.0 + 50.0 && hi >= 1000.0 - 50.0,
        "band {i} covers {lo}..{hi}"
    );
    assert!(a.levels()[i] > 0.9, "level {}", a.levels()[i]);
    // far-away bands stay low
    assert!(a.levels()[0] < 0.4 && a.levels()[a.bands() - 1] < 0.4);
}

#[test]
fn tone_moves_with_frequency() {
    let mut a = Analyzer::new(32, 44_100.0);
    a.update(&sine(200.0, 0.8, 44_100.0), 0.033);
    let low = argmax(a.levels());
    let mut b = Analyzer::new(32, 44_100.0);
    b.update(&sine(8000.0, 0.8, 44_100.0), 0.033);
    assert!(argmax(b.levels()) > low + 10);
}

#[test]
fn silence_is_all_zero() {
    let mut a = Analyzer::new(24, 44_100.0);
    a.update(&vec![0.0; 4096], 0.033);
    assert!(a.levels().iter().all(|v| *v == 0.0));
    assert!(a.peaks().iter().all(|v| *v == 0.0));
    assert!(a.settled());
}

#[test]
fn bars_fall_then_settle_and_peaks_hold() {
    let mut a = Analyzer::new(32, 44_100.0);
    a.update(&sine(1000.0, 0.9, 44_100.0), 0.033);
    let i = argmax(a.levels());
    let top = a.levels()[i];
    // silence: bar drops, the peak cap holds for ~400 ms
    a.update(&[], 0.1);
    assert!(a.levels()[i] < top - 0.2);
    assert!(a.peaks()[i] >= top - 0.01, "peak should still be held");
    for _ in 0..8 {
        a.update(&[], 0.1);
    }
    assert!(a.peaks()[i] < top - 0.3, "peak should be falling by now");
    for _ in 0..30 {
        a.update(&[], 0.1);
    }
    assert!(
        a.settled(),
        "everything at rest after a few seconds of silence"
    );
}

#[test]
fn band_edges_are_log_spaced_and_cover_the_range() {
    let e = band_edges(24);
    assert!((e[0] - 40.0).abs() < 0.01 && (e[24] - 16_000.0).abs() < 1.0);
    let r1 = e[1] / e[0];
    let r2 = e[20] / e[19];
    assert!((r1 - r2).abs() < 1e-3);
}

#[test]
fn short_input_is_handled() {
    let mut a = Analyzer::new(16, 48_000.0);
    a.update(&[0.5; 100], 0.033);
    assert_eq!(a.levels().len(), 16);
}

#[test]
fn many_bands_use_a_bigger_fft_and_interpolate_the_lowest_ones() {
    assert_eq!(Analyzer::new(100, 44_100.0).fft_len(), 4096);
    let mut wide = Analyzer::new(400, 44_100.0);
    assert_eq!(wide.fft_len(), 8192);
    // broadband input (deterministic noise) so every band has energy
    let mut x = 12345u32;
    let noise: Vec<f32> = (0..8192)
        .map(|_| {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5
        })
        .collect();
    wide.update(&noise, 0.033);
    let low = &wide.levels()[..120];
    // neighbouring low columns must not repeat identical blocky steps
    let equal = low.windows(2).filter(|w| w[0] == w[1]).count();
    assert!(equal < 12, "{equal} identical neighbours among 119");
    wide = Analyzer::new(400, 44_100.0);
    wide.update(&sine(120.0, 0.8, 44_100.0), 0.033);
    // and the tone is still found in the right place
    let i = argmax(wide.levels());
    let (lo, hi) = wide.band_range_hz(i);
    assert!(lo <= 150.0 && hi >= 90.0, "band {i} covers {lo}..{hi}");
}
