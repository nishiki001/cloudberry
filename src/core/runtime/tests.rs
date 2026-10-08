use super::*;
use crate::core::model::Kind;
use std::sync::Mutex;

#[derive(Default)]
struct Fake {
    appended: Mutex<Vec<String>>,
    loaded: Mutex<Vec<String>>,
}
impl Player for Fake {
    fn load(&self, url: &str, mode: LoadMode) -> Result<()> {
        let list = if mode == LoadMode::Append {
            &self.appended
        } else {
            &self.loaded
        };
        list.lock().unwrap().push(url.to_string());
        Ok(())
    }
    fn play(&self) -> Result<()> {
        Ok(())
    }
    fn pause(&self) -> Result<()> {
        Ok(())
    }
    fn toggle(&self) -> Result<()> {
        Ok(())
    }
    fn stop(&self) -> Result<()> {
        Ok(())
    }
    fn seek(&self, _: f64) -> Result<()> {
        Ok(())
    }
    fn set_volume(&self, _: u8) -> Result<()> {
        Ok(())
    }
    fn next(&self) -> Result<()> {
        Ok(())
    }
    fn prev(&self) -> Result<()> {
        Ok(())
    }
    fn clear_pending(&self) -> Result<()> {
        Ok(())
    }
    fn set_audio_format(&self, _: &str) -> Result<()> {
        Ok(())
    }
    fn set_cookies(&self, _: Option<&std::path::Path>) -> Result<()> {
        Ok(())
    }
}

fn item(id: &str) -> Item {
    Item {
        kind: Kind::Song,
        title: id.into(),
        video_id: Some(id.into()),
        browse_id: None,
        artists: vec![],
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail: None,
        subtitle: String::new(),
    }
}

fn start(
    fake: Arc<Fake>,
) -> (
    CoreHandle,
    mpsc::UnboundedSender<PlayerEvent>,
    std::sync::mpsc::Receiver<Event>,
) {
    let (ptx, prx) = mpsc::unbounded_channel();
    let (etx, erx) = std::sync::mpsc::channel();
    let h = spawn(
        Arc::new(move |e| {
            let _ = etx.send(e);
        }),
        Box::new(move || {
            Ok(crate::player::Backend {
                player: fake as Arc<dyn Player>,
                events: prx,
                notice: None,
                tap: None,
            })
        }),
        CoreDeps {
            cookie_path: None,
            thumb_dir: None,
            volume: 50,
            lrclib: false,
            api_base: None,
        },
    );
    (h, ptx, erx)
}

fn drain(erx: &std::sync::mpsc::Receiver<Event>) -> Vec<Event> {
    let mut v = vec![];
    while let Ok(e) = erx.recv_timeout(std::time::Duration::from_millis(300)) {
        v.push(e);
    }
    v
}

#[test]
fn eof_advances_and_end_stops() {
    let fake = Arc::new(Fake::default());
    let (h, ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a"), item("b")],
        index: 0,
    });
    ptx.send(PlayerEvent::EndFile(EndReason::Eof)).unwrap();
    ptx.send(PlayerEvent::EndFile(EndReason::Eof)).unwrap();
    let last = drain(&erx)
        .into_iter()
        .filter_map(|e| {
            if let Event::State(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .next_back();
    h.send(Command::Quit);
    let loaded = fake.loaded.lock().unwrap().clone();
    assert_eq!(loaded.len(), 2);
    assert!(loaded[1].ends_with("v=b"));
    assert!(!last.unwrap().has_track);
}

#[test]
fn prefetch_makes_next_gapless() {
    let fake = Arc::new(Fake::default());
    let (h, ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a"), item("b")],
        index: 0,
    });
    ptx.send(PlayerEvent::FileLoaded).unwrap();
    ptx.send(PlayerEvent::EndFile(EndReason::Eof)).unwrap();
    let evs = drain(&erx);
    h.send(Command::Quit);
    assert_eq!(
        fake.loaded.lock().unwrap().len(),
        1,
        "no second replace-load"
    );
    assert!(fake.appended.lock().unwrap()[0].ends_with("v=b"));
    let cur = evs
        .iter()
        .filter_map(|e| {
            if let Event::Queue { current, .. } = e {
                Some(*current)
            } else {
                None
            }
        })
        .next_back();
    assert_eq!(cur, Some(Some(1)));
}

#[test]
fn pause_never_starts_playback() {
    let fake = Arc::new(Fake::default());
    let (h, ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a")],
        index: 0,
    });
    ptx.send(PlayerEvent::Paused(true)).unwrap();
    h.send(Command::Pause);
    h.send(Command::Pause);
    let last = drain(&erx)
        .into_iter()
        .filter_map(|e| {
            if let Event::State(s) = e {
                Some(s)
            } else {
                None
            }
        })
        .next_back()
        .unwrap();
    h.send(Command::Quit);
    assert!(!last.playing && last.has_track);
    assert_eq!(
        fake.loaded.lock().unwrap().len(),
        1,
        "Pause must not reload/start"
    );
}

#[test]
fn add_and_play_appends_and_starts_the_new_item() {
    let fake = Arc::new(Fake::default());
    let (h, _ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a")],
        index: 0,
    });
    h.send(Command::AddAndPlay(item("z")));
    let evs = drain(&erx);
    h.send(Command::Quit);
    let loaded = fake.loaded.lock().unwrap().clone();
    assert!(loaded.last().unwrap().ends_with("v=z"), "{loaded:?}");
    let q = evs
        .iter()
        .rev()
        .find_map(|e| {
            if let Event::Queue { items, current } = e {
                Some((items.len(), *current))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(q, (2, Some(1)));
}

#[test]
fn clear_queue_stops_and_empties() {
    let fake = Arc::new(Fake::default());
    let (h, _ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a"), item("b")],
        index: 0,
    });
    h.send(Command::ClearQueue);
    let evs = drain(&erx);
    h.send(Command::Quit);
    let last = evs
        .iter()
        .rev()
        .find_map(|e| {
            if let Event::State(s) = e {
                Some(s.clone())
            } else {
                None
            }
        })
        .unwrap();
    assert!(!last.has_track);
    let q = evs
        .iter()
        .rev()
        .find_map(|e| {
            if let Event::Queue { items, .. } = e {
                Some(items.len())
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(q, 0);
}

#[test]
fn reorder_and_remove_many_through_the_controller() {
    let fake = Arc::new(Fake::default());
    let (h, _ptx, erx) = start(fake.clone());
    h.send(Command::PlayItems {
        items: vec![item("a"), item("b"), item("c")],
        index: 1,
    });
    h.send(Command::QueueReorder(vec![2, 1, 0]));
    h.send(Command::QueueRemoveMany(vec![0]));
    let evs = drain(&erx);
    h.send(Command::Quit);
    let (titles, cur) = evs
        .iter()
        .rev()
        .find_map(|e| {
            if let Event::Queue { items, current } = e {
                Some((
                    items.iter().map(|i| i.title.clone()).collect::<Vec<_>>(),
                    *current,
                ))
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(titles, ["b", "a"]);
    assert_eq!(cur, Some(0), "b keeps playing");
}

/// The real controller with a silent player, talking to a fake InnerTube server and the
/// scratch config dirs (for GUI tests). Events arrive on the returned receiver.
pub(crate) fn start_with_api(
    api_base: Option<String>,
) -> (CoreHandle, std::sync::mpsc::Receiver<Event>) {
    let fake = Arc::new(Fake::default());
    let (_ptx, prx) = mpsc::unbounded_channel();
    let (etx, erx) = std::sync::mpsc::channel();
    let h = spawn(
        Arc::new(move |e| {
            let _ = etx.send(e);
        }),
        Box::new(move || {
            Ok(crate::player::Backend {
                player: fake as Arc<dyn Player>,
                events: prx,
                notice: None,
                tap: None,
            })
        }),
        CoreDeps {
            cookie_path: Some(crate::config::cookies_path()),
            thumb_dir: None,
            volume: 50,
            lrclib: false,
            api_base,
        },
    );
    (h, erx)
}
