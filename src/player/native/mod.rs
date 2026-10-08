//! Native playback backend: yt-dlp resolves, we download and decode (symphonia), cpal plays.
mod core;
mod core_ops;
pub(super) mod decoder;
mod engine;
mod output;
mod position;
mod resample;
mod resolver;
mod source;

pub use engine::{Opened, Opener};
pub use output::{Shared, SinkKind, TAP_LEN};

use crate::player::{LoadMode, Player, PlayerEvents};
use anyhow::Result;
use engine::{Cmd, Engine};
use resolver::{Resolver, video_id};
use source::HttpSource;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub struct NativeOptions {
    pub ytdl_path: PathBuf,
    pub cookies: Option<PathBuf>,
    pub volume: u8,
}

/// Resolves with yt-dlp, streams with HTTP range requests.
pub(super) struct YtOpener {
    pub(super) resolver: Arc<Resolver>,
}

impl Opener for YtOpener {
    fn open(&self, target: &str) -> Result<Opened> {
        let id = video_id(target);
        let r = self.resolver.resolve(&id, false)?;
        let resolver = self.resolver.clone();
        let refresh: source::Refresher = Box::new(move || {
            let r = resolver.resolve(&id, true)?;
            Ok((r.url, r.headers))
        });
        let src = HttpSource::open(r.url, r.headers, Some(refresh))?;
        Ok(Opened {
            source: Box::new(src),
            ext: "m4a".into(),
            bitrate_kbps: r.bitrate_kbps,
            duration: r.duration,
        })
    }
}

pub struct NativePlayer {
    engine: Engine,
    pub(super) resolver: Arc<Resolver>,
}

impl NativePlayer {
    pub fn new(opts: NativeOptions) -> Result<(Self, PlayerEvents)> {
        let kind = if std::env::var("APP_AO").is_ok_and(|v| v == "null") {
            SinkKind::Null
        } else {
            SinkKind::Device
        };
        let resolver = Resolver::new(opts.ytdl_path, opts.cookies);
        let opener = Arc::new(YtOpener {
            resolver: resolver.clone(),
        });
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = Engine::start(opener, kind, tx)?;
        engine.shared.set_volume(opts.volume as f32 / 100.0);
        Ok((Self { engine, resolver }, rx))
    }

    /// Latest mono samples for the analyzer.
    pub fn tap(&self) -> Arc<output::Shared> {
        self.engine.shared.clone()
    }
}

impl Player for NativePlayer {
    fn load(&self, url: &str, mode: LoadMode) -> Result<()> {
        self.engine.send(Cmd::Load {
            target: url.to_string(),
            replace: mode == LoadMode::Replace,
        });
        Ok(())
    }
    // pause state is written directly: the decode thread may be busy with the network
    fn play(&self) -> Result<()> {
        self.engine.shared.paused.store(false, Ordering::SeqCst);
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        self.engine.shared.paused.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn toggle(&self) -> Result<()> {
        self.engine.shared.paused.fetch_xor(true, Ordering::SeqCst);
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        self.engine.send(Cmd::Stop);
        Ok(())
    }
    fn seek(&self, secs: f64) -> Result<()> {
        self.engine.send(Cmd::Seek(secs));
        Ok(())
    }
    fn set_volume(&self, vol: u8) -> Result<()> {
        self.engine.shared.set_volume(vol.min(100) as f32 / 100.0);
        Ok(())
    }
    fn next(&self) -> Result<()> {
        Ok(()) // the controller drives the queue; there is no player-side playlist
    }
    fn prev(&self) -> Result<()> {
        Ok(())
    }
    fn set_audio_format(&self, _format: &str) -> Result<()> {
        Ok(()) // the native backend always asks for the best m4a/AAC stream
    }
    fn set_cookies(&self, path: Option<&Path>) -> Result<()> {
        self.resolver.set_cookies(path.map(Path::to_path_buf));
        Ok(())
    }
    fn clear_pending(&self) -> Result<()> {
        self.engine.send(Cmd::ClearPending);
        Ok(())
    }
}
