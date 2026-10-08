//! Managed helper tools: the official standalone yt-dlp and deno binaries, downloaded into the app
//! data dir (checked against the published SHA-256 sums), so users need no system install and
//! yt-dlp stays current. `deps::locate` prefers them; "use system tools" or the `system-tools`
//! cargo feature (distro packages) turn the managed copies off.
mod fetch;

pub use fetch::{ensure, update_due, update_ytdlp};
use std::path::PathBuf;

/// Distro packagers build with `--features system-tools`: nothing is downloaded or updated.
pub const SYSTEM_ONLY_BUILD: bool = cfg!(feature = "system-tools");

pub fn managed_dir() -> PathBuf {
    crate::config::data_dir().join("tools")
}

/// File name of a tool in the managed dir.
pub fn exe_name(tool: &str) -> String {
    if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_string()
    }
}

pub fn managed_path(tool: &str) -> Option<PathBuf> {
    let p = managed_dir().join(exe_name(tool));
    p.is_file().then_some(p)
}

/// Managed copies are in use unless the build or the user chose the system ones.
pub fn managed_enabled() -> bool {
    !SYSTEM_ONLY_BUILD && !crate::config::Config::load().use_system_tools
}

/// Release asset of yt-dlp for this platform.
pub fn ytdlp_asset() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("yt-dlp_linux"),
        ("linux", "aarch64") => Some("yt-dlp_linux_aarch64"),
        ("macos", _) => Some("yt-dlp_macos"),
        ("windows", _) => Some("yt-dlp.exe"),
        _ => None,
    }
}

/// Release target triple of deno for this platform.
pub fn deno_triple() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
        _ => None,
    }
}

/// `<tool> --version` of a managed or system binary (first line), for Settings and `tools status`.
pub fn version_of(path: &std::path::Path) -> Option<String> {
    let out = std::process::Command::new(path)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let line = text.lines().next()?.trim();
    (!line.is_empty()).then(|| line.to_string())
}

/// `tools` subcommand: install / update / status.
pub fn cli(action: &str) -> anyhow::Result<i32> {
    match action {
        "install" => {
            ensure(&|p| eprintln!("{}", p.text()))?;
            Ok(0)
        }
        "update" => {
            ensure(&|p| eprintln!("{}", p.text()))?;
            println!("{}", update_ytdlp()?);
            Ok(0)
        }
        "status" => {
            for t in ["yt-dlp", "deno"] {
                match managed_path(t) {
                    Some(p) => println!(
                        "managed {t}: {} ({})",
                        p.display(),
                        version_of(&p).unwrap_or_else(|| "broken?".into())
                    ),
                    None => println!("managed {t}: not installed"),
                }
            }
            println!("managed tools in use: {}", managed_enabled());
            Ok(0)
        }
        other => {
            eprintln!("unknown action {other:?} (install | update | status)");
            Ok(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_assets_known_for_release_targets() {
        if cfg!(any(target_os = "linux", target_os = "macos")) {
            assert!(ytdlp_asset().is_some() && deno_triple().is_some());
        }
    }
}
