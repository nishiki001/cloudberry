//! Decode-thread operations: command handling, track start/seek bookkeeping.
use super::core::{Core, Pending, Playing};
use super::decoder::TrackDecoder;
use super::engine::Cmd;
use super::position::MarkKind;
use super::resample::Resample;
use crate::player::PlayerEvent;
use std::sync::atomic::Ordering;

impl Core {
    /// True when nothing is playing or buffered.
    pub(super) fn is_idle(&self) -> bool {
        self.cur.is_none()
            && self.replace_ticket.is_none()
            && !self.waiting_next
            && !self.ending
            && self.pending.is_empty()
            && self.carry.is_empty()
            && self.pushed <= self.shared.played.load(Ordering::SeqCst)
    }

    pub(super) fn handle(&mut self, c: Cmd) {
        match c {
            Cmd::Quit => self.quit = true,
            Cmd::Load { target, replace } => {
                if replace || self.is_idle() {
                    self.flush_all();
                    if replace {
                        // a new track always starts playing (after the old audio is gone)
                        self.shared.paused.store(false, Ordering::SeqCst);
                    }
                    self.cur = None;
                    self.finished.clear();
                    self.pending.clear();
                    self.replace_ticket = Some(self.spawn_open(target));
                } else {
                    // a finished track may still be draining from the ring: take back its End
                    // mark and let the new track follow it seamlessly
                    if self.cur.is_none()
                        && !self.ending
                        && self.pending.is_empty()
                        && self.replace_ticket.is_none()
                    {
                        let mut m = self.marks.lock().unwrap();
                        if m.back().is_some_and(|b| matches!(b.kind, MarkKind::End)) {
                            m.pop_back();
                            drop(m);
                            self.waiting_next = true;
                        }
                    }
                    let t = self.spawn_open(target);
                    self.pending.push((t, Pending::Opening(t)));
                }
            }
            Cmd::Opened { ticket, result } => self.opened(ticket, result),
            Cmd::Pause(p) => self.shared.paused.store(p, Ordering::SeqCst),
            Cmd::Seek(s) => self.seek(s),
            Cmd::Stop => {
                self.flush_all();
                self.cur = None;
                self.finished.clear();
                self.pending.clear();
                self.replace_ticket = None;
                let _ = self.events.send(PlayerEvent::Idle(true));
            }
            Cmd::ClearPending => {
                self.pending.clear();
                if self.waiting_next {
                    // nothing left to wait for: finish the stream
                    self.waiting_next = false;
                    self.advance();
                }
            }
        }
    }

    pub(super) fn opened(
        &mut self,
        ticket: u64,
        result: Result<(TrackDecoder, Option<u32>), String>,
    ) {
        if self.replace_ticket == Some(ticket) {
            self.replace_ticket = None;
            match result {
                Ok((dec, br)) => self.start_track(dec, br),
                Err(e) => {
                    tracing::warn!("open failed: {e}");
                    let id = self.next_id;
                    self.next_id += 1;
                    self.mark(id, 0.0, None, None, MarkKind::Error(e));
                }
            }
            return;
        }
        if let Some(slot) = self.pending.iter_mut().find(|(t, _)| *t == ticket) {
            slot.1 = match result {
                Ok((d, br)) => Pending::Ready(Box::new(d), br),
                Err(e) => Pending::Failed(e),
            };
            if self.waiting_next {
                self.waiting_next = false;
                self.advance();
            }
        }
    }

    pub(super) fn new_playing(&mut self, dec: TrackDecoder, bitrate: Option<u32>) -> Playing {
        let id = self.next_id;
        self.next_id += 1;
        let rate = self.shared.rate;
        let rs = (dec.sample_rate != rate)
            .then(|| Resample::new(dec.sample_rate, rate))
            .flatten();
        Playing {
            dec,
            rs,
            id,
            bitrate,
            in_frames: 0,
            out_frames: 0,
            start_secs: 0.0,
        }
    }

    pub(super) fn start_track(&mut self, dec: TrackDecoder, bitrate: Option<u32>) {
        let p = self.new_playing(dec, bitrate);
        self.mark(p.id, 0.0, p.dec.duration_secs, bitrate, MarkKind::Start);
        self.cur = Some(p);
    }

    /// Seek in whichever track is audible right now. Decoding runs up to ~3 s ahead, so that
    /// may be an earlier, already finished track while `cur` is the prefetched next one.
    pub(super) fn seek(&mut self, secs: f64) {
        let audible = self.shared.audible.load(Ordering::SeqCst);
        let mut p = if let Some(i) = self.finished.iter().position(|p| p.id == audible) {
            let p = self.finished.remove(i);
            // every later track that is already decoded goes back to the front of the queue,
            // rewound, in order
            let mut later: Vec<Playing> =
                self.finished.drain(..).filter(|f| f.id > audible).collect();
            later.extend(self.cur.take());
            for (n, t) in later.into_iter().enumerate() {
                let mut dec = t.dec;
                let _ = dec.seek(0.0);
                self.pending
                    .insert(n, (0, Pending::Ready(Box::new(dec), t.bitrate)));
            }
            p
        } else if let Some(p) = self.cur.take() {
            p
        } else {
            return;
        };
        self.flush_all();
        if let Err(e) = p.dec.seek(secs) {
            tracing::warn!("{e:#}");
        } else {
            p.start_secs = secs;
        }
        p.rs = (p.dec.sample_rate != self.shared.rate)
            .then(|| Resample::new(p.dec.sample_rate, self.shared.rate))
            .flatten();
        p.in_frames = 0;
        p.out_frames = 0;
        let (id, start, dur, br) = (p.id, p.start_secs, p.dec.duration_secs, p.bitrate);
        self.mark(id, start, dur, br, MarkKind::Start);
        self.cur = Some(p);
    }
}
