//! Colour picker glue: the maths lives in core/color.rs; this wires it to the Slint global and
//! keeps the "recent colours" list in the config.
use super::state::STATE;
use crate::core::color::{hsv_to_hex, parse_hex, push_recent, rgb_to_hsv};
use crate::{AppWindow, ColorMath, HsvValue};
use slint::{Color, ComponentHandle, ModelRc, VecModel};

const MAX_RECENT: usize = 8;

fn slint_color(hex: &str) -> Color {
    parse_hex(hex).map_or(Color::from_argb_u8(0, 0, 0, 0), |c| {
        Color::from_rgb_u8(c.0, c.1, c.2)
    })
}

fn show_recent(ui: &AppWindow, list: &[String]) {
    let rows: Vec<(Color, slint::SharedString)> = list
        .iter()
        .map(|h| (slint_color(h), h.as_str().into()))
        .collect();
    ui.global::<ColorMath>()
        .set_recent(ModelRc::new(VecModel::from(rows)));
}

pub fn init(ui: &AppWindow) {
    let m = ui.global::<ColorMath>();
    m.on_hsv_to_hex(|h, s, v| hsv_to_hex(h, s, v).into());
    m.on_hex_to_hsv(|hex| match parse_hex(&hex) {
        Some(c) => {
            let h = rgb_to_hsv(c);
            HsvValue {
                h: h.h,
                s: h.s,
                v: h.v,
                ok: true,
            }
        }
        None => HsvValue {
            h: 0.0,
            s: 0.0,
            v: 0.0,
            ok: false,
        },
    });
    m.on_to_color(|hex| slint_color(&hex));
    let weak = ui.as_weak();
    m.on_commit(move |hex| {
        let list = STATE.with(|s| {
            let mut s = s.borrow_mut();
            push_recent(&mut s.cfg.recent_colors, &hex, MAX_RECENT);
            let _ = s.cfg.save();
            s.cfg.recent_colors.clone()
        });
        if let Some(ui) = weak.upgrade() {
            show_recent(&ui, &list);
        }
    });
    let list = STATE.with(|s| s.borrow().cfg.recent_colors.clone());
    show_recent(ui, &list);
}
