//! OS media integration via souvlaki: MPRIS (Linux), Now Playing (macOS), SMTC (Windows).
//! Failures are logged and ignored: the app works without it.
use crate::core::msg::{Command, Snapshot};
use crate::core::runtime::CoreHandle;
use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
    SeekDirection,
};
use std::time::Duration;

pub struct MediaCtl {
    controls: MediaControls,
    last_video: Option<String>,
    last_playing: Option<bool>,
    last_pos_sent: f64,
    last_duration: f64,
}

impl MediaCtl {
    /// `hwnd` is only needed on Windows (SMTC); without it controls are skipped there.
    pub fn new(core: CoreHandle, hwnd: Option<*mut std::ffi::c_void>) -> Option<Self> {
        #[cfg(target_os = "windows")]
        if hwnd.is_none() {
            // souvlaki panics without an HWND on Windows
            tracing::info!("SMTC skipped: no window handle");
            return None;
        }
        let cfg = PlatformConfig {
            dbus_name: crate::deps::APP_ID,
            display_name: "Cloudberry",
            hwnd,
        };
        let mut controls = match MediaControls::new(cfg) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("media controls unavailable: {e:?}");
                return None;
            }
        };
        let attached = controls.attach(move |ev| match ev {
            MediaControlEvent::Play => core.send(Command::Play),
            MediaControlEvent::Pause => core.send(Command::Pause),
            MediaControlEvent::Toggle => core.send(Command::Toggle),
            MediaControlEvent::Next => core.send(Command::Next),
            MediaControlEvent::Previous => core.send(Command::Prev),
            MediaControlEvent::Stop => core.send(Command::Stop),
            MediaControlEvent::SeekBy(dir, d) => {
                core.send(Command::SeekBy(if dir == SeekDirection::Forward {
                    d.as_secs_f64()
                } else {
                    -d.as_secs_f64()
                }))
            }
            MediaControlEvent::SetPosition(MediaPosition(d)) => {
                core.send(Command::SeekTo(d.as_secs_f64()))
            }
            _ => {}
        });
        if let Err(e) = attached {
            tracing::warn!("media controls attach failed: {e:?}");
            return None;
        }
        Some(Self {
            controls,
            last_video: None,
            last_playing: None,
            last_pos_sent: -100.0,
            last_duration: 0.0,
        })
    }

    /// Push state to the OS. Cheap to call on every snapshot: only changes are forwarded
    /// (position every ~5 s so the OS widget can interpolate).
    pub fn update(&mut self, snap: &Snapshot, cover_url: Option<&str>) {
        // metadata is re-sent on track change and when the real duration arrives
        let dur_changed = (snap.duration - self.last_duration).abs() > 1.0;
        if snap.video_id != self.last_video || dur_changed {
            self.last_duration = snap.duration;
            self.last_video.clone_from(&snap.video_id);
            self.last_playing = None;
            if snap.has_track {
                let (title, artist) = (snap.title.as_str(), snap.artist_line.as_str());
                let (artist, album) = match artist.split_once(" — ") {
                    Some((a, al)) => (a, Some(al)),
                    None => (artist, None),
                };
                let _ = self.controls.set_metadata(MediaMetadata {
                    title: Some(title),
                    artist: Some(artist),
                    album,
                    cover_url,
                    duration: (snap.duration > 0.0).then(|| Duration::from_secs_f64(snap.duration)),
                });
            } else {
                let _ = self.controls.set_metadata(MediaMetadata::default());
            }
        }
        let moved = (snap.pos - self.last_pos_sent).abs() > 5.0;
        if self.last_playing != Some(snap.playing) || moved {
            self.last_playing = Some(snap.playing);
            self.last_pos_sent = snap.pos;
            let progress = Some(MediaPosition(Duration::from_secs_f64(snap.pos.max(0.0))));
            let pb = if !snap.has_track {
                MediaPlayback::Stopped
            } else if snap.playing {
                MediaPlayback::Playing { progress }
            } else {
                MediaPlayback::Paused { progress }
            };
            let _ = self.controls.set_playback(pb);
        }
    }
}
