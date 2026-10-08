//! Background tokio runtime and the playback controller. The UI sends `Command`s in; events
//! come back through a callback (the GUI wraps it in `invoke_from_event_loop`).
use super::api::client::Client;
use super::lyrics;
use super::model::Item;
use super::msg::{Command, Event, LibraryKind, Snapshot};
use super::queue::Queue;
use super::thumbs::Thumbs;
use crate::player::{EndReason, LoadMode, Player, PlayerEvent, watch_url};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Liked-id updates for the controller.
enum LikedMsg {
    /// A full liked list arrived: replaces the set (in-flight toggles are kept).
    Replace(Vec<String>),
    /// Force one id's state (rollback of a failed toggle).
    Set(String, bool),
    /// A like/unlike request finished (id, requested state, success). Ignored when a newer
    /// toggle for the same id is in flight.
    Done(String, bool, bool),
}

pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;
pub type PlayerFactory = Box<dyn FnOnce() -> Result<crate::player::Backend> + Send>;

#[derive(Clone)]
pub struct CoreHandle {
    tx: mpsc::UnboundedSender<Command>,
}

impl CoreHandle {
    /// A handle that is not connected to a controller; the receiver shows what was sent (tests).
    #[cfg(test)]
    pub fn fake() -> (CoreHandle, mpsc::UnboundedReceiver<Command>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (CoreHandle { tx }, rx)
    }

    pub fn send(&self, c: Command) {
        let _ = self.tx.send(c);
    }
}

/// Plain data; the client and thumbnail pipeline are built on the core thread (TLS setup,
/// cookie reads) so they never delay the first frame.
pub struct CoreDeps {
    pub cookie_path: Option<std::path::PathBuf>,
    pub thumb_dir: Option<std::path::PathBuf>,
    pub volume: u8,
    /// Send title/artist/album to lrclib.net for synced lyrics.
    pub lrclib: bool,
    /// Another InnerTube base URL (tests with a fake server).
    #[cfg(test)]
    pub api_base: Option<String>,
}

struct Services {
    lyrics: Option<Arc<super::lyrics::LyricsClient>>,
    client: Option<Arc<Client>>,
    thumbs: Option<Arc<Thumbs>>,
}

/// Start the runtime thread. It exits on `Command::Quit` or when all handles drop.
pub fn spawn(sink: EventSink, make_player: PlayerFactory, deps: CoreDeps) -> CoreHandle {
    let (tx, rx) = mpsc::unbounded_channel::<Command>();
    std::thread::Builder::new()
        .name("core-rt".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("tokio runtime");
            rt.block_on(Controller::run(rx, sink, make_player, deps));
        })
        .expect("spawn core thread");
    CoreHandle { tx }
}

struct Controller {
    player: Arc<dyn Player>,
    sink: EventSink,
    svc: Services,
    queue: Queue,
    search_seq: Arc<std::sync::atomic::AtomicU64>,
    /// (search sequence, continuation token) of the newest search that has a next page.
    search_next: Arc<std::sync::Mutex<Option<(u64, String)>>>,
    snap: Snapshot,
    paused: bool,
    idle: bool,
    progressed: bool,
    /// Re-sent to the UI as covers arrive for the current track.
    cover_tx: mpsc::UnboundedSender<(String, Arc<image::RgbaImage>)>,
    cover_url: Option<String>,
    /// The next queue entry is already appended to the player (gapless).
    prefetched: bool,
    /// Radio results arrive here: (seed video id, items).
    radio_tx: mpsc::UnboundedSender<(String, Vec<Item>)>,
    radio_for: Option<String>,
    /// Current track ended with nothing queued; start the radio result when it arrives.
    awaiting_next: bool,
    last_lyrics_for: Option<String>,
    liked_ids: std::collections::HashSet<String>,
    /// Ids of the liked list arrive here (preload at start, or when the Liked view loads).
    liked_tx: mpsc::UnboundedSender<LikedMsg>,
    inflight: std::collections::HashMap<String, bool>,
}

impl Controller {
    async fn run(
        mut rx: mpsc::UnboundedReceiver<Command>,
        sink: EventSink,
        make_player: PlayerFactory,
        deps: CoreDeps,
    ) {
        let (player, mut pev, notice) = match make_player() {
            Ok(b) => (b.player, b.events, b.notice),
            Err(e) => {
                sink(Event::Error(format!("player unavailable: {e:#}")));
                // still serve search etc.; playback commands are ignored
                while let Some(c) = rx.recv().await {
                    if matches!(c, Command::Quit) {
                        break;
                    }
                }
                return;
            }
        };
        if let Some(n) = notice {
            sink(Event::Error(n));
        }
        let (cover_tx, mut cover_rx) = mpsc::unbounded_channel();
        let (radio_tx, mut radio_rx) = mpsc::unbounded_channel();
        let (liked_tx, mut liked_rx) = mpsc::unbounded_channel();
        let mut c = Controller {
            player,
            sink,
            snap: Snapshot {
                volume: deps.volume,
                repeat: Some(super::queue::Repeat::Off),
                ..Default::default()
            },
            svc: Services {
                lyrics: super::lyrics::LyricsClient::new(deps.lrclib)
                    .ok()
                    .map(Arc::new),
                client: {
                    #[cfg(test)]
                    let made = match &deps.api_base {
                        Some(b) => Client::with_base(deps.cookie_path.clone(), b.clone()),
                        None => Client::new(deps.cookie_path.clone()),
                    };
                    #[cfg(not(test))]
                    let made = Client::new(deps.cookie_path.clone());
                    made.ok().map(Arc::new)
                },
                thumbs: deps
                    .thumb_dir
                    .clone()
                    .and_then(|d| Thumbs::new(d).ok())
                    .map(Arc::new),
            },
            search_seq: Arc::default(),
            search_next: Arc::default(),
            queue: Queue::default(),
            paused: false,
            idle: true,
            progressed: false,
            cover_tx,
            cover_url: None,
            prefetched: false,
            radio_tx,
            radio_for: None,
            awaiting_next: false,
            last_lyrics_for: None,
            liked_ids: Default::default(),
            liked_tx,
            inflight: Default::default(),
        };
        c.preload_liked();
        if let Some(t) = c.svc.thumbs.clone() {
            tokio::task::spawn_blocking(move || t.prune(50 << 20));
        }
        let _ = c.player.set_volume(c.snap.volume);
        loop {
            tokio::select! {
                biased;
                cmd = rx.recv() => match cmd {
                    Some(Command::Quit) | None => break,
                    Some(cmd) => c.command(cmd),
                },
                Some(ev) = pev.recv() => c.player_event(ev),
                Some(msg) = liked_rx.recv() => {
                    c.liked_update(msg);
                    c.emit();
                }
                Some((seed, items)) = radio_rx.recv() => c.radio_arrived(seed, items),
                Some((url, img)) = cover_rx.recv() => {
                    if c.cover_url.as_deref() == Some(url.as_str()) {
                        (c.sink)(Event::Cover(img));
                    }
                }
            }
        }
        let _ = c.player.stop();
    }

    fn emit(&mut self) {
        self.snap.playing = self.snap.has_track && !self.paused && !self.idle;
        self.snap.queue_len = self.queue.len();
        self.snap.liked = self
            .snap
            .video_id
            .as_ref()
            .is_some_and(|v| self.liked_ids.contains(v));
        self.snap.repeat = Some(self.queue.repeat());
        self.snap.shuffle = self.queue.shuffle();
        (self.sink)(Event::State(self.snap.clone()));
    }

    fn emit_queue(&mut self) {
        (self.sink)(Event::Queue {
            items: self.queue.items().to_vec(),
            current: self.queue.current_index(),
        });
        self.emit();
    }

    fn fail(&mut self, msg: &str) {
        (self.sink)(Event::Error(msg.to_string()));
    }
}

mod commands;
mod discover;
pub use discover::discover_key;
mod library;
mod playback;
mod playlist_edit;
mod search;
mod stream;
#[cfg(test)]
pub(crate) mod tests;
