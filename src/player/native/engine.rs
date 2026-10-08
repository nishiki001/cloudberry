//! The playback engine: one decode thread feeding the output ring, one position thread
//! turning the sink's progress into `PlayerEvent`s. Tracks are queued inside the pipeline,
//! so a prefetched next track starts at the exact frame the previous one ends (gapless).
use super::core::Core;
use super::decoder::TrackDecoder;
use super::output::{self, OutputGuard, Shared, SinkKind};
use super::position::{Marks, position_loop};
use crate::player::PlayerEvent;
use anyhow::Result;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{Sender, channel};
use symphonia::core::io::MediaSource;
use tokio::sync::mpsc::UnboundedSender;

pub struct Opened {
    pub source: Box<dyn MediaSource>,
    pub ext: String,
    pub bitrate_kbps: Option<u32>,
    pub duration: Option<f64>,
}

/// Turns a target (watch URL / id) into a readable stream. Blocking; runs on worker threads.
pub trait Opener: Send + Sync + 'static {
    fn open(&self, target: &str) -> Result<Opened>;
}

pub(super) enum Cmd {
    Load {
        target: String,
        replace: bool,
    },
    Opened {
        ticket: u64,
        result: Result<(TrackDecoder, Option<u32>), String>,
    },
    Pause(bool),
    Seek(f64),
    Stop,
    ClearPending,
    Quit,
}

pub struct Engine {
    tx: Sender<Cmd>,
    pub shared: Arc<Shared>,
    _guard: OutputGuard,
}

impl Engine {
    pub fn start(
        opener: Arc<dyn Opener>,
        kind: SinkKind,
        events: UnboundedSender<PlayerEvent>,
    ) -> Result<Self> {
        let (prod, shared, guard) = output::open(kind)?;
        let (tx, rx) = channel();
        let marks: Marks = Arc::default();
        let epoch = Arc::new(AtomicU64::new(0));
        {
            let (shared, marks, epoch, events) =
                (shared.clone(), marks.clone(), epoch.clone(), events.clone());
            std::thread::Builder::new()
                .name("pos".into())
                .spawn(move || position_loop(shared, marks, epoch, events))?;
        }
        let core = Core {
            rx,
            tx: tx.clone(),
            opener,
            prod,
            shared: shared.clone(),
            marks,
            epoch,
            events,
            cur: None,
            pending: Vec::new(),
            carry: Vec::new(),
            pushed: 0,
            next_id: 1,
            next_ticket: 1,
            replace_ticket: None,
            ending: false,
            waiting_next: false,
            finished: Vec::new(),
            quit: false,
        };
        std::thread::Builder::new()
            .name("decode".into())
            .spawn(move || core.run())?;
        Ok(Self {
            tx,
            shared,
            _guard: guard,
        })
    }

    pub(super) fn send(&self, c: Cmd) {
        let _ = self.tx.send(c);
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Quit);
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "engine_real_tests.rs"]
mod real_tests;
