use super::*;

/// Settings → Playback: choose system tools, press "Update now".
#[test]
fn settings_helper_tools_controls() {
    let t = app();
    let d = t.ui.global::<crate::Dialogs>();
    d.set_settings_visible(true);
    d.set_settings_page(0);
    d.set_tools_status("yt-dlp 2026.01.01 · deno 2.0.0".into());
    let log = Log::default();
    let (g1, g2) = (log.clone(), log.clone());
    d.on_set_tools_source(move |id| g1.borrow_mut().push(format!("source:{id}")));
    d.on_update_tools(move || g2.borrow_mut().push("update".into()));
    assert!(
        t.item("Update now").is_some()
            || ElementHandle::find_by_accessible_label(&t.ui, "Update now")
                .next()
                .is_some()
    );
    t.click_label("Update now");
    t.click_label("System");
    assert_eq!(*log.borrow(), ["update", "source:system"]);
    // while an update runs the button is disabled
    d.set_tools_busy(true);
    t.click_label("Updating…");
    assert_eq!(log.borrow().len(), 2);
}

/// The Retry button exists only when a download can be retried, and then it fires.
#[test]
fn deps_dialog_retry_only_when_wired() {
    let _t = app();
    let dlg = crate::DepsDialog::new().unwrap();
    dlg.window().set_size(PhysicalSize::new(640, 360));
    dlg.show().unwrap();
    assert!(
        ElementHandle::find_by_accessible_label(&dlg, "Retry")
            .next()
            .is_none()
    );
    let hits = Rc::new(RefCell::new(0));
    let h = hits.clone();
    dlg.on_retry(move || *h.borrow_mut() += 1);
    dlg.set_can_retry(true);
    let b = ElementHandle::find_by_accessible_label(&dlg, "Retry")
        .next()
        .expect("retry button");
    let (p, s) = (b.absolute_position(), b.size());
    let position = LogicalPosition::new(p.x + s.width / 2.0, p.y + s.height / 2.0);
    let button = PointerEventButton::Left;
    dlg.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    dlg.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    dlg.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
    assert_eq!(*hits.borrow(), 1);
    dlg.set_busy(true);
    assert!(
        ElementHandle::find_by_accessible_label(&dlg, "Retry")
            .next()
            .is_none()
    );
}

/// A build with system tools only (`tools-locked`) ignores the source choice and Update.
#[test]
fn locked_tools_ignore_clicks() {
    let t = app();
    let d = t.ui.global::<crate::Dialogs>();
    d.set_settings_visible(true);
    d.set_settings_page(0);
    d.set_tools_locked(true);
    let log = Log::default();
    let (g1, g2) = (log.clone(), log.clone());
    d.on_set_tools_source(move |id| g1.borrow_mut().push(id.to_string()));
    d.on_update_tools(move || g2.borrow_mut().push("update".into()));
    t.click_label("Update now");
    t.click_label("System");
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
}

/// Settings → Account has the cookies.txt import (the way in from a Flatpak sandbox).
#[test]
fn settings_account_import_cookies() {
    let t = app();
    let d = t.ui.global::<crate::Dialogs>();
    d.set_settings_visible(true);
    d.set_settings_page(5);
    let hits = Rc::new(RefCell::new(0));
    let h = hits.clone();
    d.on_import_cookies(move || *h.borrow_mut() += 1);
    t.click_label("Import cookies.txt…");
    assert_eq!(*hits.borrow(), 1);
}

/// The first-run sign-in panel offers it too.
#[test]
fn setup_panel_import_cookies() {
    let t = app();
    let d = t.ui.global::<crate::Dialogs>();
    d.set_setup_visible(true);
    let hits = Rc::new(RefCell::new(0));
    let h = hits.clone();
    d.on_import_cookies(move || *h.borrow_mut() += 1);
    t.click_label("Import cookies.txt…");
    assert_eq!(*hits.borrow(), 1);
}
