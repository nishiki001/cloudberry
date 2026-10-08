//! Offscreen rendering of a view to PNG using Slint's software renderer. No window, no network.
use crate::cli::ThemeArg;
use crate::{AppWindow, DepsDialog, Theme};
use anyhow::{Context, Result};
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType,
};
use slint::platform::{Platform, WindowAdapter};
use slint::{ComponentHandle, PhysicalSize, PlatformError};
use std::path::Path;
use std::rc::Rc;

struct ShotPlatform(Rc<MinimalSoftwareWindow>);

impl Platform for ShotPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
}

/// Render `AppWindow` after `setup` has configured it, and write a PNG.
pub fn render(out: &Path, size: (u32, u32), setup: impl FnOnce(&AppWindow)) -> Result<()> {
    render_component(out, size, AppWindow::new, setup)
}

/// The missing-dependencies dialog with its real text.
pub fn render_deps(out: &Path, theme: ThemeArg) -> Result<()> {
    render_component(out, (640, 360), DepsDialog::new, |d| {
        d.global::<Theme>()
            .set_mode(if matches!(theme, ThemeArg::Dark) {
                2
            } else {
                1
            });
        d.set_message(crate::deps::install_help(&["yt-dlp", "deno"]).into());
    })
}

fn render_component<C: ComponentHandle>(
    out: &Path,
    (w, h): (u32, u32),
    create: fn() -> Result<C, PlatformError>,
    setup: impl FnOnce(&C),
) -> Result<()> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(ShotPlatform(window.clone())))
        .map_err(|e| anyhow::anyhow!("set_platform: {e}"))?;
    let ui = create().context("create window")?;
    setup(&ui);
    ui.show()?;
    window.set_size(PhysicalSize::new(w, h));
    let mut buf = vec![PremultipliedRgbaColor::default(); (w * h) as usize];
    let mut render = |window: &MinimalSoftwareWindow| {
        window.draw_if_needed(|r| {
            r.render(&mut buf, w as usize);
        })
    };
    anyhow::ensure!(render(&window), "nothing rendered");
    // let layout-triggered animations and one-shot timers finish, then draw the settled frame
    for _ in 0..4 {
        std::thread::sleep(std::time::Duration::from_millis(250));
        slint::platform::update_timers_and_animations();
        render(&window);
    }
    let mut raw = Vec::with_capacity(buf.len() * 4);
    for p in &buf {
        raw.extend_from_slice(&[p.red, p.green, p.blue, p.alpha]);
    }
    image::save_buffer(out, &raw, w, h, image::ColorType::Rgba8)
        .with_context(|| format!("write {}", out.display()))?;
    Ok(())
}
