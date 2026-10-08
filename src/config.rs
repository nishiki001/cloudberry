use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const APP: &str = "cloudberry";

/// Screenshot mode runs the real glue code with fake data: it must never write the user's files.
pub static NO_PERSIST: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub browser: String,
    pub audio_format: String,
    pub theme: String,
    pub reduce_motion: bool,
    pub volume: u8,
    /// The first-run sign-in panel was skipped or completed.
    pub setup_done: bool,
    /// Use yt-dlp / deno from the system instead of the managed copies (needs a restart).
    pub use_system_tools: bool,
    /// "native" (symphonia + cpal, default) or "mpv" (fallback).
    pub backend: String,
    /// "block" | "bars" | "off"
    pub analyzer_style: String,
    pub analyzer_fps: u32,
    /// "accent" | "classic" | "ice" | "sunset" | "mono" | "custom"
    pub analyzer_scheme: String,
    /// Gradient stops ("#RRGGBB", bottom → top, 1–3) of the custom scheme.
    pub analyzer_custom: Vec<String>,
    /// Peak-cap colour; empty = derived from the top stop.
    pub analyzer_peak: String,
    /// Spinning-disc cover needs item rotation, which only the GL renderer supports.
    pub spinning_disc: bool,
    /// "slow" (12 s per turn) | "normal" (8 s) | "fast" (4 s)
    pub disc_speed: String,
    /// Playlist table: column order, widths, visibility.
    pub table: crate::core::columns::ColumnConfig,
    /// Look up synced lyrics on lrclib.net (sends title/artist/album/duration to that service).
    pub lyrics_lrclib: bool,
    /// Custom colours instead of the system ones ("#RRGGBB"; empty = system value).
    pub colors_custom: bool,
    pub accent: String,
    pub window_color: String,
    pub text_color: String,
    /// Playlist background: "none" | "pattern" | "cover" | "image".
    pub bg_mode: String,
    pub bg_path: String,
    /// "cover" | "contain" | "stretch"
    pub bg_fit: String,
    /// "top" | "center" | "bottom"
    pub bg_pos: String,
    pub bg_opacity: f32,
    /// 0..1 (of 24 px)
    pub bg_blur: f32,
    /// 0..1 (of 0.8)
    pub bg_tint: f32,
    /// Tint colour ("#RRGGBB"); empty = the theme's base colour.
    pub bg_tint_color: String,
    /// Most recently committed picker colours ("#RRGGBB"), newest first.
    pub recent_colors: Vec<String>,
    /// Playlist ids most recently added to, newest first (orders the "Add to playlist" menu).
    pub playlist_mru: Vec<String>,
    /// "classic" | "aero"
    pub skin: String,
    /// Icon set: "line" | "aero" | "pixel" (independent of the skin).
    pub icon_set: String,
    /// UI font family ("" = the system default) and size in px (0 = default).
    pub font_family: String,
    pub font_size: u32,
    /// Optional separate font for the lyrics ("" = same as the UI font).
    pub lyrics_font: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            browser: "firefox".into(),
            audio_format: "bestaudio/best".into(),
            theme: "system".into(),
            reduce_motion: false,
            volume: 70,
            setup_done: false,
            use_system_tools: false,
            backend: "native".into(),
            analyzer_style: "block".into(),
            analyzer_fps: 30,
            analyzer_scheme: "accent".into(),
            analyzer_custom: vec!["#2ECC71".into(), "#F1C40F".into(), "#E74C3C".into()],
            analyzer_peak: String::new(),
            spinning_disc: false,
            disc_speed: "normal".into(),
            table: Default::default(),
            lyrics_lrclib: true,
            colors_custom: false,
            accent: String::new(),
            window_color: String::new(),
            text_color: String::new(),
            bg_mode: "none".into(),
            bg_path: String::new(),
            bg_fit: "cover".into(),
            bg_pos: "center".into(),
            bg_opacity: 0.5,
            bg_blur: 0.0,
            bg_tint: 0.2,
            bg_tint_color: String::new(),
            recent_colors: Vec::new(),
            playlist_mru: Vec::new(),
            skin: "classic".into(),
            icon_set: "line".into(),
            font_family: String::new(),
            font_size: 0,
            lyrics_font: String::new(),
        }
    }
}

fn dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("", "", APP)
}

/// Tests run against a scratch directory, never the user's real config / cache / data.
#[cfg(test)]
static TEST_ROOT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(test)]
pub fn use_test_dirs() -> PathBuf {
    TEST_ROOT
        .get_or_init(|| {
            let d = std::env::temp_dir().join(format!("cloudberry-test-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&d);
            d
        })
        .clone()
}

pub fn config_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(r) = TEST_ROOT.get() {
        return r.join("config");
    }
    dirs()
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
#[allow(dead_code)] // used from M5
pub fn cache_dir() -> PathBuf {
    #[cfg(test)]
    if let Some(r) = TEST_ROOT.get() {
        return r.join("cache");
    }
    dirs()
        .map(|d| d.cache_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".cache"))
}
pub fn data_dir() -> PathBuf {
    // CLOUDBERRY_TEST_REAL_AUTH=1: the ignored real-API tests use the real cookie file
    #[cfg(test)]
    if let (Some(r), None) = (
        TEST_ROOT.get(),
        std::env::var_os("CLOUDBERRY_TEST_REAL_AUTH"),
    ) {
        return r.join("data");
    }
    dirs()
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".data"))
}
pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}
#[allow(dead_code)] // used from M2
pub fn cookies_path() -> PathBuf {
    data_dir().join("cookies.txt")
}

impl Config {
    /// Load config, writing defaults on first run. Bad files fall back to defaults.
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(s) => {
                let mut c: Self = toml::from_str(&s).unwrap_or_else(|e| {
                    tracing::warn!("bad config.toml: {e}");
                    Self::default()
                });
                c.table = c.table.sanitized();
                c
            }
            Err(_) => {
                let c = Self::default();
                let _ = c.save();
                c
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        if NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
            return Ok(());
        }
        let path = config_path();
        std::fs::create_dir_all(path.parent().unwrap()).context("create config dir")?;
        std::fs::write(&path, toml::to_string_pretty(self)?).context("write config")?;
        Ok(())
    }
}

/// File logging via tracing; the guard must live for the whole run.
pub fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dir = data_dir().join("logs");
    std::fs::create_dir_all(&dir).ok()?;
    let appender = tracing_appender::rolling::daily(dir, "cloudberry.log");
    let (nb, guard) = tracing_appender::non_blocking(appender);
    tracing_subscriber::fmt()
        .with_writer(nb)
        .with_ansi(false)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cloudberry=debug,info".into()),
        )
        .try_init()
        .ok()?;
    Some(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_roundtrip() {
        let s = toml::to_string_pretty(&Config::default()).unwrap();
        let c: Config = toml::from_str(&s).unwrap();
        assert_eq!(c.volume, 70);
        assert_eq!(c.browser, "firefox");
    }
    #[test]
    fn partial_file_uses_defaults() {
        let c: Config = toml::from_str("volume = 10").unwrap();
        assert_eq!(c.volume, 10);
        assert_eq!(c.audio_format, "bestaudio/best");
    }
}
