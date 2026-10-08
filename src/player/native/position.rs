//! Sink progress → player events (track start/end, position, pause, idle).
use super::output::Shared;
use crate::player::{EndReason, PlayerEvent};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub(super) enum MarkKind {
    Start,
    End,
    Error(String),
    /// The prefetched next track failed to open: the previous one still ended normally.
    PendingError(String),
}

pub(super) struct Mark {
    pub start_frame: u64,
    pub id: u64,
    pub start_secs: f64,
    pub duration: Option<f64>,
    pub bitrate: Option<u32>,
    pub kind: MarkKind,
}

pub(super) type Marks = Arc<Mutex<VecDeque<Mark>>>;

/// Turns sink progress into events. Marks are processed in order, so even if the sink
/// races ahead (tests) no track boundary is skipped.
pub(super) fn position_loop(
    shared: Arc<Shared>,
    marks: Marks,
    epoch: Arc<AtomicU64>,
    events: UnboundedSender<PlayerEvent>,
) {
    struct Cur {
        id: u64,
        start_frame: u64,
        start_secs: f64,
        duration: Option<f64>,
    }
    let rate = shared.rate as f64;
    let mut seen_epoch = 0;
    let mut cur: Option<Cur> = None;
    let mut announced: Option<u64> = None;
    let mut paused = false;
    let mut last_pos = f64::NEG_INFINITY;
    loop {
        std::thread::sleep(Duration::from_millis(50));
        if events.is_closed() {
            return;
        }
        let e = epoch.load(Ordering::SeqCst);
        if e != seen_epoch {
            seen_epoch = e;
            cur = None;
            last_pos = f64::NEG_INFINITY;
        }
        let p = shared.paused.load(Ordering::Relaxed);
        if p != paused {
            paused = p;
            let _ = events.send(PlayerEvent::Paused(p));
        }
        let played = shared.played.load(Ordering::SeqCst);
        loop {
            let m = {
                let mut q = marks.lock().unwrap();
                match q.front() {
                    Some(f) if f.start_frame <= played => q.pop_front(),
                    _ => None,
                }
            };
            let Some(m) = m else { break };
            match m.kind {
                MarkKind::Start => {
                    // a seek re-announces the same track id: that is not an end
                    if cur.as_ref().is_some_and(|c| c.id != m.id) {
                        let _ = events.send(PlayerEvent::EndFile(EndReason::Eof));
                    }
                    shared.audible.store(m.id, Ordering::SeqCst);
                    if announced != Some(m.id) {
                        announced = Some(m.id);
                        let _ = events.send(PlayerEvent::StartFile);
                        if let Some(d) = m.duration {
                            let _ = events.send(PlayerEvent::Duration(d));
                        }
                        let _ = events.send(PlayerEvent::Format(match m.bitrate {
                            Some(b) => format!("AAC {b} kbps"),
                            None => "AAC".to_string(),
                        }));
                        let _ = events.send(PlayerEvent::Idle(false));
                        let _ = events.send(PlayerEvent::FileLoaded);
                    }
                    cur = Some(Cur {
                        id: m.id,
                        start_frame: m.start_frame,
                        start_secs: m.start_secs,
                        duration: m.duration,
                    });
                    last_pos = f64::NEG_INFINITY;
                }
                MarkKind::End => {
                    if cur.take().is_some() {
                        let _ = events.send(PlayerEvent::EndFile(EndReason::Eof));
                    }
                    let _ = events.send(PlayerEvent::Idle(true));
                }
                MarkKind::PendingError(msg) => {
                    if cur.take().is_some() {
                        let _ = events.send(PlayerEvent::EndFile(EndReason::Eof));
                    }
                    let _ = events.send(PlayerEvent::Error(msg));
                    let _ = events.send(PlayerEvent::EndFile(EndReason::Error));
                    let _ = events.send(PlayerEvent::Idle(true));
                }
                MarkKind::Error(msg) => {
                    cur = None;
                    let _ = events.send(PlayerEvent::Error(msg));
                    let _ = events.send(PlayerEvent::EndFile(EndReason::Error));
                    let _ = events.send(PlayerEvent::Idle(true));
                }
            }
        }
        if let Some(c) = &cur {
            let mut pos = c.start_secs + played.saturating_sub(c.start_frame) as f64 / rate;
            if let Some(d) = c.duration {
                pos = pos.min(d);
            }
            let _ = c.id;
            if (pos - last_pos).abs() >= 0.2 {
                last_pos = pos;
                let _ = events.send(PlayerEvent::TimePos(pos));
            }
        }
    }
}
