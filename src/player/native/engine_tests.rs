use super::*;
use crate::player::EndReason;
use crate::player::native::decoder::{MemSource, wav_bytes};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

struct MemOpener;
impl Opener for MemOpener {
    fn open(&self, target: &str) -> Result<Opened> {
        let (rate, secs, freq) = match target {
            "a" => (44100, 1.0, 440.0),
            "b" => (44100, 1.0, 880.0),
            "long" => (44100, 4.0, 300.0),
            "r48" => (48000, 1.0, 440.0),
            "slow" => {
                std::thread::sleep(Duration::from_millis(800));
                (44100, 1.0, 500.0)
            }
            "bad" => anyhow::bail!("boom"),
            _ => anyhow::bail!("unknown"),
        };
        Ok(Opened {
            source: Box::new(MemSource(std::io::Cursor::new(wav_bytes(
                rate, secs, freq, 0.5,
            )))),
            ext: "wav".into(),
            bitrate_kbps: None,
            duration: None,
        })
    }
}

fn start() -> (Engine, UnboundedReceiver<PlayerEvent>) {
    let (tx, rx) = unbounded_channel();
    (
        Engine::start(Arc::new(MemOpener), SinkKind::Capture, tx).unwrap(),
        rx,
    )
}

fn load(e: &Engine, t: &str, replace: bool) {
    e.send(Cmd::Load {
        target: t.into(),
        replace,
    });
}

/// Collect events until `stop` matches (10 s timeout).
fn wait_for(
    rx: &mut UnboundedReceiver<PlayerEvent>,
    mut stop: impl FnMut(&PlayerEvent) -> bool,
) -> Vec<PlayerEvent> {
    let mut v = Vec::new();
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(10) {
        match rx.try_recv() {
            Ok(ev) => {
                let done = stop(&ev);
                v.push(ev);
                if done {
                    return v;
                }
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    panic!("timed out; events so far: {v:?}");
}

fn longest_silence(samples: &[f32]) -> usize {
    let (mut best, mut run) = (0, 0);
    for f in samples.chunks(2) {
        if f[0].abs() < 1e-3 && f[1].abs() < 1e-3 {
            run += 1;
            best = best.max(run);
        } else {
            run = 0;
        }
    }
    best
}

#[test]
fn two_tracks_are_gapless() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    load(&e, "b", false);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let starts = evs
        .iter()
        .filter(|e| matches!(e, PlayerEvent::StartFile))
        .count();
    let ends = evs
        .iter()
        .filter(|e| matches!(e, PlayerEvent::EndFile(EndReason::Eof)))
        .count();
    assert_eq!((starts, ends), (2, 2), "{evs:?}");
    let cap = e.shared.capture.lock().unwrap().clone().unwrap();
    let frames = cap.len() / 2;
    assert!((frames as i64 - 88200).abs() < 200, "frames {frames}");
    let gap = longest_silence(&cap);
    assert!(
        gap < 882,
        "silence of {gap} frames at the boundary (limit 882 = 20 ms)"
    );
}

#[test]
fn resampled_track_keeps_length() {
    let (e, mut rx) = start();
    load(&e, "r48", true);
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let frames = e.shared.capture.lock().unwrap().as_ref().unwrap().len() / 2;
    assert!((frames as i64 - 44100).abs() < 100, "frames {frames}");
}

#[test]
fn seek_moves_position_and_shortens_output() {
    let (e, mut rx) = start();
    load(&e, "long", true);
    e.send(Cmd::Pause(true)); // a replace load unpauses; hold the capture sink so it cannot race through the track
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::FileLoaded));
    e.send(Cmd::Seek(3.0));
    std::thread::sleep(Duration::from_millis(100));
    e.shared.paused.store(false, Ordering::SeqCst);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let positions: Vec<f64> = evs
        .iter()
        .filter_map(|e| {
            if let PlayerEvent::TimePos(p) = e {
                Some(*p)
            } else {
                None
            }
        })
        .collect();
    assert!(
        positions.iter().any(|p| (2.9..3.6).contains(p)),
        "positions {positions:?}"
    );
    // 4 s track: roughly 1 s plays after the seek (plus what played before it)
    let frames = e.shared.capture.lock().unwrap().as_ref().unwrap().len() / 2;
    assert!(frames < 44100 * 3, "frames {frames}");
}

#[test]
fn replace_load_does_not_report_old_track_end() {
    let (e, mut rx) = start();
    e.shared.paused.store(true, Ordering::SeqCst);
    load(&e, "long", true);
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::FileLoaded));
    load(&e, "a", true);
    std::thread::sleep(Duration::from_millis(100));
    e.shared.paused.store(false, Ordering::SeqCst);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let eofs = evs
        .iter()
        .filter(|e| matches!(e, PlayerEvent::EndFile(EndReason::Eof)))
        .count();
    assert_eq!(eofs, 1, "only the new track ends: {evs:?}");
}

#[test]
fn clear_pending_while_waiting_finishes_the_stream() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    load(&e, "slow", false);
    std::thread::sleep(Duration::from_millis(100));
    e.send(Cmd::ClearPending); // the next track is dropped before it opened
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    assert!(
        evs.iter()
            .any(|e| matches!(e, PlayerEvent::EndFile(EndReason::Eof))),
        "{evs:?}"
    );
}

#[test]
fn seek_after_the_last_track_finished_restarts_it() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    e.shared.paused.store(true, Ordering::SeqCst); // observe the restart before the fast sink finishes it
    e.send(Cmd::Seek(0.5));
    std::thread::sleep(Duration::from_millis(150));
    e.shared.paused.store(false, Ordering::SeqCst);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let positions: Vec<f64> = evs
        .iter()
        .filter_map(|e| {
            if let PlayerEvent::TimePos(p) = e {
                Some(*p)
            } else {
                None
            }
        })
        .collect();
    assert!(
        positions.iter().any(|p| *p >= 0.5),
        "restarted from 0.5: {evs:?}"
    );
}

#[test]
fn append_while_tail_is_draining_joins_seamlessly() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    e.send(Cmd::Pause(true)); // A fully decoded into the ring, nothing played
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::FileLoaded));
    std::thread::sleep(Duration::from_millis(300)); // decoder has pushed A and its End mark
    load(&e, "b", false);
    std::thread::sleep(Duration::from_millis(200));
    e.shared.paused.store(false, Ordering::SeqCst);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let starts = evs
        .iter()
        .filter(|e| matches!(e, PlayerEvent::StartFile))
        .count();
    assert_eq!(
        starts, 1,
        "B starts after A (A's StartFile was consumed above): {evs:?}"
    );
    let frames = e.shared.capture.lock().unwrap().as_ref().unwrap().len() / 2;
    assert!((frames as i64 - 88200).abs() < 200, "frames {frames}");
}

#[test]
fn seek_while_previous_track_is_audible_keeps_the_prefetched_one() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    load(&e, "b", false);
    e.send(Cmd::Pause(true)); // A and B both end up decoded into the ring; A is the audible one
    std::thread::sleep(Duration::from_millis(500));
    e.send(Cmd::Seek(0.5));
    std::thread::sleep(Duration::from_millis(200));
    e.shared.paused.store(false, Ordering::SeqCst);
    wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let frames = e.shared.capture.lock().unwrap().as_ref().unwrap().len() / 2;
    // 0.5 s of A after the seek + the whole of B
    assert!((frames as i64 - 66150).abs() < 400, "frames {frames}");
}

#[test]
fn failed_prefetch_ends_previous_track_normally_then_reports_error() {
    let (e, mut rx) = start();
    load(&e, "a", true);
    load(&e, "bad", false);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    let eof = evs
        .iter()
        .position(|e| matches!(e, PlayerEvent::EndFile(EndReason::Eof)));
    let err = evs
        .iter()
        .position(|e| matches!(e, PlayerEvent::EndFile(EndReason::Error)));
    assert!(eof.is_some() && err.is_some() && eof < err, "{evs:?}");
}

#[test]
fn failed_open_reports_error() {
    let (e, mut rx) = start();
    load(&e, "bad", true);
    let evs = wait_for(&mut rx, |ev| matches!(ev, PlayerEvent::Idle(true)));
    assert!(
        evs.iter()
            .any(|e| matches!(e, PlayerEvent::EndFile(EndReason::Error))),
        "{evs:?}"
    );
}
