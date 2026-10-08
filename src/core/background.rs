//! Playlist background: fit / position / blur / tint of a source picture rendered at the
//! table's pixel size, plus an original default pattern. Pure; callers run it off the UI thread.
use super::theme::Rgb;
use image::imageops::{self, FilterType};
use image::{Rgba, RgbaImage};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fit {
    Cover,
    Contain,
    Stretch,
}

/// Where the picture sits in the canvas: one of the nine cells of a 3×3 grid.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Pos {
    /// -1 left, 0 centre, 1 right
    pub h: i8,
    /// -1 top, 0 centre, 1 bottom
    pub v: i8,
}

impl Fit {
    pub fn parse(s: &str) -> Fit {
        match s {
            "contain" => Fit::Contain,
            "stretch" => Fit::Stretch,
            _ => Fit::Cover,
        }
    }
}

impl Pos {
    /// "top-left", "top", "top-right", "left", "center", "right", "bottom-left", "bottom",
    /// "bottom-right". The older values `top`, `center`, `bottom` are part of that set, so saved
    /// configs keep working; anything unknown is the centre.
    pub fn parse(s: &str) -> Pos {
        let mut p = Pos { h: 0, v: 0 };
        for part in s.split('-') {
            match part {
                "left" => p.h = -1,
                "right" => p.h = 1,
                "top" => p.v = -1,
                "bottom" => p.v = 1,
                _ => {}
            }
        }
        p
    }

    /// Top-left corner of a `tw`×`th` picture inside a `w`×`h` canvas.
    pub fn offset(self, (w, h): (u32, u32), (tw, th): (u32, u32)) -> (i64, i64) {
        let at = |dir: i8, canvas: u32, size: u32| {
            let free = canvas as i64 - size as i64;
            match dir {
                -1 => 0,
                1 => free,
                _ => free / 2,
            }
        };
        (at(self.h, w, tw), at(self.v, h, th))
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Params {
    pub fit: Fit,
    pub pos: Pos,
    /// Blur radius in output pixels (0 = sharp).
    pub blur: f32,
    /// Colour laid over the picture (and used behind it for "contain").
    pub tint: Rgb,
    /// 0..1 strength of the tint.
    pub tint_alpha: f32,
}

fn fill(w: u32, h: u32, c: Rgb) -> RgbaImage {
    RgbaImage::from_pixel(w, h, Rgba([c.0, c.1, c.2, 255]))
}

/// Render `src` into a `w`×`h` canvas.
pub fn render(src: &RgbaImage, w: u32, h: u32, p: &Params) -> RgbaImage {
    let (w, h) = (w.clamp(1, 4096), h.clamp(1, 4096));
    let mut canvas = fill(w, h, p.tint);
    let (sw, sh) = (src.width().max(1) as f32, src.height().max(1) as f32);
    let (tw, th) = match p.fit {
        Fit::Stretch => (w as f32, h as f32),
        Fit::Cover => {
            let k = (w as f32 / sw).max(h as f32 / sh);
            (sw * k, sh * k)
        }
        Fit::Contain => {
            let k = (w as f32 / sw).min(h as f32 / sh);
            (sw * k, sh * k)
        }
    };
    let (tw, th) = ((tw.round() as u32).max(1), (th.round() as u32).max(1));
    let scaled = imageops::resize(src, tw, th, FilterType::Triangle);
    let (x, y) = p.pos.offset((w, h), (tw, th));
    imageops::overlay(&mut canvas, &scaled, x, y);
    if p.blur > 0.5 {
        // blur at quarter size: cheap and indistinguishable once smoothed
        let (qw, qh) = ((w / 4).max(1), (h / 4).max(1));
        let small = imageops::resize(&canvas, qw, qh, FilterType::Triangle);
        let blurred = imageops::blur(&small, (p.blur / 4.0).max(0.3));
        canvas = imageops::resize(&blurred, w, h, FilterType::Triangle);
    }
    if p.tint_alpha > 0.0 {
        let a = p.tint_alpha.clamp(0.0, 1.0);
        for px in canvas.pixels_mut() {
            for (c, t) in px.0.iter_mut().zip([p.tint.0, p.tint.1, p.tint.2]) {
                *c = (*c as f32 * (1.0 - a) + t as f32 * a).round() as u8;
            }
        }
    }
    canvas
}

/// The default pattern: soft concentric "disc" rings fanning out from the bottom-right corner,
/// in the accent colour over the base colour. Original artwork, generated, no asset file.
pub fn pattern(w: u32, h: u32, accent: Rgb, base: Rgb) -> RgbaImage {
    let (w, h) = (w.clamp(1, 4096), h.clamp(1, 4096));
    let (cx, cy) = (w as f32, h as f32);
    RgbaImage::from_fn(w, h, |x, y| {
        let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        let ring = ((d / 22.0).sin() * 0.5 + 0.5).powi(6);
        let fade = (1.0 - d / (cx.max(cy) * 1.1)).clamp(0.0, 1.0);
        let c = base.mix(accent, ring * fade * 0.55);
        Rgba([c.0, c.1, c.2, 255])
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(fit: Fit, pos: Pos) -> Params {
        Params {
            fit,
            pos,
            blur: 0.0,
            tint: Rgb(10, 20, 30),
            tint_alpha: 0.0,
        }
    }

    fn two_tone() -> RgbaImage {
        RgbaImage::from_fn(40, 20, |x, _| {
            if x < 20 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        })
    }

    #[test]
    fn cover_fills_every_pixel_and_contain_letterboxes() {
        let src = two_tone();
        let out = render(&src, 100, 100, &params(Fit::Cover, Pos::parse("center")));
        assert_eq!(out.dimensions(), (100, 100));
        assert_eq!(out.get_pixel(0, 0).0[..3], [255, 0, 0]);
        let out = render(&src, 100, 100, &params(Fit::Contain, Pos::parse("top")));
        assert_eq!(out.get_pixel(5, 99).0[..3], [10, 20, 30]); // empty band uses the tint colour
        assert_ne!(out.get_pixel(5, 5).0[..3], [10, 20, 30]);
        let out = render(&src, 100, 100, &params(Fit::Contain, Pos::parse("bottom")));
        assert_eq!(out.get_pixel(5, 0).0[..3], [10, 20, 30]);
    }

    #[test]
    fn blur_smooths_the_edge_and_tint_covers() {
        let src = two_tone();
        let mut p = params(Fit::Stretch, Pos::parse("center"));
        let sharp = render(&src, 80, 40, &p);
        p.blur = 16.0;
        let soft = render(&src, 80, 40, &p);
        assert_eq!(soft.dimensions(), (80, 40));
        let near = |i: &RgbaImage| i.get_pixel(41, 20).0[0] as i32;
        assert!(
            near(&sharp) < 10 && near(&soft) > 20,
            "{} {}",
            near(&sharp),
            near(&soft)
        );
        p.tint_alpha = 1.0;
        assert_eq!(
            render(&src, 80, 40, &p).get_pixel(3, 3).0[..3],
            [10, 20, 30]
        );
    }

    #[test]
    fn pattern_is_deterministic_and_not_flat() {
        let a = pattern(120, 80, Rgb(0x3D, 0xAE, 0xE9), Rgb(250, 250, 250));
        let b = pattern(120, 80, Rgb(0x3D, 0xAE, 0xE9), Rgb(250, 250, 250));
        assert_eq!(a, b);
        assert!(a.pixels().any(|p| p != a.get_pixel(0, 0)));
    }

    #[test]
    fn anchors_place_the_picture_in_all_nine_cells_for_every_fit() {
        let canvas = (100u32, 60u32);
        let names = [
            "top-left",
            "top",
            "top-right",
            "left",
            "center",
            "right",
            "bottom-left",
            "bottom",
            "bottom-right",
        ];
        // (picture size under each fit for a 40×20 source on a 100×60 canvas)
        let sizes = [
            (Fit::Cover, (120u32, 60u32)),
            (Fit::Contain, (100, 50)),
            (Fit::Stretch, (100, 60)),
        ];
        for (fit, (tw, th)) in sizes {
            let (free_x, free_y) = (100i64 - tw as i64, 60i64 - th as i64);
            for (i, name) in names.iter().enumerate() {
                let (col, row) = ((i % 3) as i64, (i / 3) as i64);
                let want = (free_x * col / 2, free_y * row / 2);
                // centre uses integer halves, as the renderer does
                let want = (
                    if col == 1 { free_x / 2 } else { want.0 },
                    if row == 1 { free_y / 2 } else { want.1 },
                );
                assert_eq!(
                    Pos::parse(name).offset(canvas, (tw, th)),
                    want,
                    "{name} {fit:?}"
                );
            }
        }
        assert_eq!(Pos::parse("whatever"), Pos { h: 0, v: 0 });
    }

    #[test]
    fn left_and_right_anchor_show_different_sides_of_a_wide_picture() {
        let src = two_tone(); // red left half, blue right half
        let left = render(&src, 20, 20, &params(Fit::Cover, Pos::parse("left")));
        let right = render(&src, 20, 20, &params(Fit::Cover, Pos::parse("right")));
        assert_eq!(left.get_pixel(10, 10).0[..3], [255, 0, 0]);
        assert_eq!(right.get_pixel(10, 10).0[..3], [0, 0, 255]);
    }
}
