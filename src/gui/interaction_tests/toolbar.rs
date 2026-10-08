use super::*;

#[test]
fn fixed_columns_follow_the_font_size() {
    let t = app_timed();
    let got = Rc::new(RefCell::new((0.0f32, 0.0f32)));
    let g = got.clone();
    t.ui.on_table_fixed_metrics(move |_, _, len, _| {
        let mut g = g.borrow_mut();
        g.0 = g.1;
        g.1 = len;
    });
    let theme = t.ui.global::<crate::Theme>();
    theme.set_font_scale(1.0);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(100));
    let small = got.borrow().1;
    theme.set_font_scale(24.0 / 12.0);
    i_slint_backend_testing::mock_elapsed_time(std::time::Duration::from_millis(100));
    let big = got.borrow().1;
    assert!(
        small > 40.0,
        "Length column measured at default size: {small}"
    );
    assert!(
        big > small * 1.6,
        "Length column grows with the font: {small} -> {big}"
    );
}

/// Vertical centre (window coordinates) of the first element with this accessible label.
fn centre_y(ui: &AppWindow, label: &str) -> f32 {
    let e = ElementHandle::find_by_accessible_label(ui, label)
        .next()
        .unwrap_or_else(|| panic!("no element \"{label}\""));
    e.absolute_position().y + e.size().height / 2.0
}

fn centre_of_type(ui: &AppWindow, ty: &str) -> f32 {
    let e = ElementHandle::find_by_element_type_name(ui, ty)
        .next()
        .unwrap_or_else(|| panic!("no {ty}"));
    e.absolute_position().y + e.size().height / 2.0
}

/// The transport buttons, analyzer, volume and toggles share one horizontal axis (both skins,
/// default and huge font).
#[test]
fn toolbar_controls_share_one_centre_line() {
    for (skin, scale) in [
        ("default", 1.0f32),
        ("aero", 1.0),
        ("default", 2.0),
        ("aero", 2.0),
    ] {
        let t = app();
        crate::gui::appearance::fixture(&t.ui, skin);
        t.ui.global::<crate::Theme>().set_font_scale(scale);
        let axis = centre_y(&t.ui, "prev");
        for l in ["pause", "play", "stop", "next", "shuffle", "repeat"] {
            if ElementHandle::find_by_accessible_label(&t.ui, l)
                .next()
                .is_none()
            {
                continue;
            }
            let c = centre_y(&t.ui, l);
            assert!(
                (c - axis).abs() <= 1.0,
                "{skin} x{scale}: {l} centre {c} vs {axis}"
            );
        }
        for ty in ["Analyzer", "HSlider"] {
            let c = centre_of_type(&t.ui, ty);
            assert!(
                (c - axis).abs() <= 1.5,
                "{skin} x{scale}: {ty} centre {c} vs {axis}"
            );
        }
    }
}

/// Clicking and dragging the sliders (Classic and Aero) sets the value.
#[test]
fn volume_slider_click_and_drag() {
    for skin in ["default", "aero"] {
        let t = app();
        crate::gui::appearance::fixture(&t.ui, skin);
        let got = Rc::new(RefCell::new(Vec::<f32>::new()));
        let g = got.clone();
        t.ui.on_set_volume(move |v| g.borrow_mut().push(v));
        // the volume slider is the one in the toolbar (y < 60), the seek bar sits below it
        let e = ElementHandle::find_by_element_type_name(&t.ui, "HSlider")
            .find(|e| {
                e.absolute_position().y > 24.0
                    && e.absolute_position().y < 60.0
                    && e.size().width < 200.0
            })
            .expect("volume slider");
        let (p, s) = (e.absolute_position(), e.size());
        let y = p.y + s.height / 2.0;
        t.at(p.x + s.width * 0.5, y);
        t.send(WindowEvent::PointerPressed {
            position: LogicalPosition::new(p.x + s.width * 0.5, y),
            button: PointerEventButton::Left,
        });
        t.at(p.x + s.width * 0.9, y);
        t.send(WindowEvent::PointerReleased {
            position: LogicalPosition::new(p.x + s.width * 0.9, y),
            button: PointerEventButton::Left,
        });
        let v = got.borrow();
        assert!(v.len() >= 2, "{skin}: press and drag both report: {v:?}");
        assert!(
            (v[0] - 0.5).abs() < 0.1 && v[v.len() - 1] > 0.8,
            "{skin}: {v:?}"
        );
    }
}
