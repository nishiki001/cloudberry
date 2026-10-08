//! The decode thread: owns the current/pending tracks, fills the output ring and queues
//! track-boundary marks. Network opens run on worker threads so commands stay responsive.
use super::decoder::TrackDecoder;
use super::engine::{Cmd, Opener};
use super::output::Shared;
use super::position::{Mark, MarkKind, Marks};
use super::resample::Resample;
use crate::player::PlayerEvent;
use rtrb::Producer;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;

pub(super) struct Playing {
    pub(super) dec: TrackDecoder,
    pub(super) rs: Option<Resample>,
    pub(super) id: u64,
    pub(super) bitrate: Option<u32>,
    pub(super) in_frames: u64,
    pub(super) out_frames: u64,
    pub(super) start_secs: f64,
}

pub(super) enum Pending {
    Opening(u64),
    Ready(Box<TrackDecoder>, Option<u32>),
    Failed(String),
}

pub(super) struct Core {
    pub(super) rx: Receiver<Cmd>,
    pub(super) tx: Sender<Cmd>,
    pub(super) opener: Arc<dyn Opener>,
    pub(super) prod: Producer<f32>,
    pub(super) shared: Arc<Shared>,
    pub(super) marks: Marks,
    pub(super) epoch: Arc<AtomicU64>,
    pub(super) events: UnboundedSender<PlayerEvent>,
    pub(super) cur: Option<Playing>,
    pub(super) pending: Vec<(u64, Pending)>,
    pub(super) carry: Vec<f32>,
    /// Frames handed to the ring so far (same clock as `shared.played`).
    pub(super) pushed: u64,
    pub(super) next_id: u64,
    pub(super) next_ticket: u64,
    pub(super) replace_ticket: Option<u64>,
    /// Current track hit EOS; advance once `carry` is flushed into the ring.
    pub(super) ending: bool,
    /// Next track still opening when the current one ended.
    pub(super) waiting_next: bool,
    /// Fully decoded tracks that may still be audible or queued in the ring (seek targets).
    pub(super) finished: Vec<Playing>,
    pub(super) quit: bool,
}

impl Core {
    pub(super) fn run(mut self) {
        while !self.quit {
            // commands first, so Pause/Seek/Load stay responsive
            loop {
                match self.rx.try_recv() {
                    Ok(c) => self.handle(c),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(_) => return,
                }
            }
            if self.quit {
                return;
            }
            // forget the finished track once the next one is audible
            let audible = self.shared.audible.load(Ordering::SeqCst);
            self.finished.retain(|p| p.id >= audible);
            let busy = self.cur.is_some() || !self.carry.is_empty() || self.ending;
            if !busy {
                self.wait(100);
                continue;
            }
            if !self.carry.is_empty() {
                self.push_carry();
                if !self.carry.is_empty() {
                    self.wait(15);
                    continue;
                }
            }
            if self.ending {
                self.advance();
                continue;
            }
            if self.cur.is_some() {
                if self.prod.slots() < 8192 {
                    self.wait(15);
                    continue;
                }
                self.decode_step();
            }
        }
    }

    /// Sleep until a command arrives (or `ms` pass) and handle it.
    fn wait(&mut self, ms: u64) {
        match self.rx.recv_timeout(Duration::from_millis(ms)) {
            Ok(c) => self.handle(c),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => self.quit = true,
        }
    }

    pub(super) fn push_carry(&mut self) {
        let n = self.prod.slots().min(self.carry.len()) & !1;
        if n == 0 {
            return;
        }
        for s in self.carry.drain(..n) {
            let _ = self.prod.push(s);
        }
        self.pushed += (n / 2) as u64;
    }

    pub(super) fn mark(
        &mut self,
        id: u64,
        start_secs: f64,
        duration: Option<f64>,
        bitrate: Option<u32>,
        kind: MarkKind,
    ) {
        self.marks.lock().unwrap().push_back(Mark {
            start_frame: self.pushed,
            id,
            start_secs,
            duration,
            bitrate,
            kind,
        });
    }

    pub(super) fn flush_all(&mut self) {
        self.shared.flush();
        self.carry.clear();
        self.pushed = self.shared.played.load(Ordering::SeqCst);
        self.marks.lock().unwrap().clear();
        self.epoch.fetch_add(1, Ordering::SeqCst);
        self.ending = false;
        self.waiting_next = false;
    }

    pub(super) fn spawn_open(&mut self, target: String) -> u64 {
        let ticket = self.next_ticket;
        self.next_ticket += 1;
        let (opener, tx) = (self.opener.clone(), self.tx.clone());
        std::thread::spawn(move || {
            let result = opener
                .open(&target)
                .and_then(|o| {
                    let mut dec = TrackDecoder::open(o.source, &o.ext)?;
                    // some containers carry no frame count: fall back to what yt-dlp said
                    dec.duration_secs = dec.duration_secs.or(o.duration);
                    Ok((dec, o.bitrate_kbps))
                })
                .map_err(|e| format!("{e:#}"));
            let _ = tx.send(Cmd::Opened { ticket, result });
        });
        ticket
    }

    pub(super) fn decode_step(&mut self) {
        let Some(p) = self.cur.as_mut() else { return };
        match p.dec.next_block() {
            Ok(Some(block)) => {
                p.in_frames += (block.len() / 2) as u64;
                let out = match p.rs.as_mut() {
                    Some(rs) => {
                        let mut o = Vec::with_capacity(block.len() * 2);
                        rs.process(&block, &mut o);
                        o
                    }
                    None => block,
                };
                p.out_frames += (out.len() / 2) as u64;
                self.carry = out;
                self.push_carry();
            }
            Ok(None) => {
                if let Some(rs) = p.rs.as_mut() {
                    let mut tail = Vec::new();
                    rs.finish(
                        p.in_frames,
                        p.dec.sample_rate,
                        self.shared.rate,
                        p.out_frames,
                        &mut tail,
                    );
                    self.carry = tail;
                }
                self.finished.extend(self.cur.take());
                self.ending = true;
            }
            Err(e) => {
                tracing::warn!("{e:#}");
                let id = p.id;
                self.finished.extend(self.cur.take());
                self.carry.clear();
                // report the failure when playback reaches this point, then carry on with
                // the prefetched track (if any)
                self.mark(
                    id + 1_000_000,
                    0.0,
                    None,
                    None,
                    MarkKind::Error(format!("{e:#}")),
                );
                self.ending = true;
            }
        }
    }

    /// The current track is fully in the ring: start the next one or finish.
    pub(super) fn advance(&mut self) {
        self.ending = false;
        if self.pending.is_empty() {
            let id = self.next_id;
            self.next_id += 1;
            self.mark(id, 0.0, None, None, MarkKind::End);
            return;
        }
        match &self.pending[0].1 {
            Pending::Opening(_) => self.waiting_next = true,
            Pending::Ready(..) => {
                if let (_, Pending::Ready(d, br)) = self.pending.remove(0) {
                    self.start_track(*d, br);
                }
            }
            Pending::Failed(_) => {
                if let (_, Pending::Failed(e)) = self.pending.remove(0) {
                    let id = self.next_id;
                    self.next_id += 1;
                    self.mark(id, 0.0, None, None, MarkKind::PendingError(e));
                }
            }
        }
    }
}
