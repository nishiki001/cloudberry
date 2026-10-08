//! Procedural artwork (original): disc sheen, blank disc, circular cover masking.
//! Done in Rust so the look is identical on every Slint renderer (the software renderer
//! ignores border-radius for gradients and images).
use image::{Rgba, RgbaImage};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

pub fn to_slint(img: &RgbaImage) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        img.as_raw(),
        img.width(),
        img.height(),
    ))
}

/// Anti-aliased circle coverage (0..1) of pixel (x, y) in a `size` square.
fn coverage(x: u32, y: u32, size: u32) -> f32 {
    let c = size as f32 / 2.0;
    let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
    (c - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0)
}

/// Square-crop is the caller's job; this clips an already square image to a circle.
pub fn circle_mask(img: &mut RgbaImage) {
    let s = img.width().min(img.height());
    for (x, y, p) in img.enumerate_pixels_mut() {
        p.0[3] = (p.0[3] as f32 * coverage(x, y, s)) as u8;
    }
}

fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}

/// Fixed rainbow reflection: translucent conic wedges, with the hub hole cut out.
pub fn sheen(size: u32) -> RgbaImage {
    const STOPS: [(f32, [f32; 4]); 9] = [
        (0.0, [255.0, 122.0, 200.0, 0.30]),
        (40.0, [255.0, 255.0, 255.0, 0.0]),
        (90.0, [58.0, 141.0, 255.0, 0.26]),
        (130.0, [255.0, 255.0, 255.0, 0.0]),
        (180.0, [255.0, 233.0, 122.0, 0.28]),
        (220.0, [255.0, 122.0, 200.0, 0.30]),
        (260.0, [255.0, 255.0, 255.0, 0.0]),
        (310.0, [41.0, 227.0, 255.0, 0.26]),
        (360.0, [255.0, 122.0, 200.0, 0.30]),
    ];
    let c = size as f32 / 2.0;
    RgbaImage::from_fn(size, size, |x, y| {
        let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
        let deg = (dy.atan2(dx).to_degrees() + 90.0).rem_euclid(360.0);
        let i = STOPS
            .windows(2)
            .position(|w| deg >= w[0].0 && deg <= w[1].0)
            .unwrap_or(0);
        let (a, b) = (STOPS[i], STOPS[i + 1]);
        let col = lerp(a.1, b.1, (deg - a.0) / (b.0 - a.0));
        // soft specular streak across the top-left
        let streak = (1.0 - ((dx + dy).abs() / (c * 0.5))).clamp(0.0, 1.0) * 0.14;
        let r = (dx * dx + dy * dy).sqrt() / c;
        let hole = ((r - 0.15) * c).clamp(0.0, 1.0);
        let alpha = (col[3] + streak) * coverage(x, y, size) * hole;
        Rgba([
            col[0] as u8,
            col[1] as u8,
            col[2] as u8,
            (alpha * 255.0) as u8,
        ])
    })
}

/// Generic silver disc for "no cover yet".
pub fn blank_disc(size: u32, dark: bool) -> RgbaImage {
    let (hi, lo) = if dark {
        ([110.0, 120.0, 138.0], [44.0, 50.0, 62.0])
    } else {
        ([250.0, 252.0, 255.0], [176.0, 188.0, 202.0])
    };
    let mut img = RgbaImage::from_fn(size, size, |x, y| {
        let t = ((x + y) as f32 / (2 * size) as f32 * 2.0 - 0.5).abs() * 2.0;
        let t = t.clamp(0.0, 1.0);
        let c: Vec<u8> = (0..3)
            .map(|i| (hi[i] + (lo[i] - hi[i]) * t) as u8)
            .collect();
        Rgba([c[0], c[1], c[2], 255])
    });
    circle_mask(&mut img);
    img
}

/// Procedural cover for fixtures: diagonal gradient + rings, tinted by `hue`.
pub fn fixture_cover(size: u32, hue: f32) -> RgbaImage {
    let c = size as f32 / 2.0;
    RgbaImage::from_fn(size, size, |x, y| {
        let (dx, dy) = (x as f32 - c, y as f32 - c);
        let r = (dx * dx + dy * dy).sqrt() / c;
        let t = (x + y) as f32 / (2 * size) as f32;
        let ring = ((r * 9.0).sin() * 0.5 + 0.5) * 0.25;
        let (rr, gg, bb) = hsv(hue + t * 80.0, 0.65, 0.55 + 0.4 * (1.0 - t) + ring);
        Rgba([rr, gg, bb, 255])
    })
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let v = v.clamp(0.0, 1.0);
    let h = h.rem_euclid(360.0) / 60.0;
    let (i, f) = (h.floor() as i32, h - h.floor());
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mask_clears_corners_keeps_center() {
        let mut i = fixture_cover(64, 100.0);
        circle_mask(&mut i);
        assert_eq!(i.get_pixel(0, 0).0[3], 0);
        assert_eq!(i.get_pixel(32, 32).0[3], 255);
    }
    #[test]
    fn sheen_has_hole_and_alpha() {
        let s = sheen(128);
        assert_eq!(s.get_pixel(64, 64).0[3], 0);
        assert!(s.pixels().any(|p| p.0[3] > 20));
    }
}
