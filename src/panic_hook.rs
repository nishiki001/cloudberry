//! Panic hook: the panic and a backtrace go to the log (and a crash file next to it, written
//! synchronously because the log writer is asynchronous); the GUI additionally shows a small
//! dialog with the log path instead of silently vanishing.
use std::sync::atomic::{AtomicBool, Ordering};

static SHOWING: AtomicBool = AtomicBool::new(false);

fn crash_file() -> std::path::PathBuf {
    crate::config::data_dir().join("logs").join("crash.log")
}

pub fn install(gui: bool) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let report = format!(
            "panic: {info}\n{}",
            std::backtrace::Backtrace::force_capture()
        );
        tracing::error!("{report}");
        let path = crash_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        {
            use std::io::Write;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
            {
                let _ = writeln!(f, "---\n{report}");
            }
        }
        default(info);
        // a second panic while the dialog is up (or from another thread) must not stack dialogs
        // only a panic on the UI thread is fatal for the window; background helpers (the MPRIS
        // thread panics when another instance owns the name) are logged but do not stop the app
        let on_ui_thread = std::thread::current().name() == Some("main");
        if gui && on_ui_thread && !SHOWING.swap(true, Ordering::SeqCst) {
            let _ = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Cloudberry hit an error")
                .set_description(format!(
                    "Something went wrong and Cloudberry has to close.\n\nDetails were written to:\n{}",
                    path.display()
                ))
                .show();
        }
    }));
}
