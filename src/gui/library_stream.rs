//! Playlists and liked songs arrive as a stream of chunks (cached copy first, then pages as
//! they are fetched). Several streams can be in flight, each identified by its stream id.
//! Targets: a new playlist tab (found again by its uid, so closing or moving tabs is safe), the
//! Library panel page — both grow in batches — or a buffer for views that need the whole list
//! (the library tree).
use super::events::{handle_event, show_library_page};
use super::state::*;
use super::{table, tabs};
use crate::core::model::Item;
use crate::core::msg::{Event, LibraryKind};
use crate::{AppWindow, Panels};
use slint::{ComponentHandle, Model};

#[derive(Clone, Copy)]
enum Target {
    /// `TabState::uid`.
    Tab(u64),
    Page,
    Buffer,
}

pub struct LibStream {
    id: u64,
    kind: LibraryKind,
    list_id: Option<String>,
    target: Target,
    acc: Vec<Item>,
}

fn songs(items: Vec<Item>) -> Vec<Item> {
    items.into_iter().filter(|i| i.video_id.is_some()).collect()
}

pub fn chunk(
    ui: &AppWindow,
    stream: u64,
    kind: LibraryKind,
    list_id: Option<String>,
    items: Vec<Item>,
    replace: bool,
    done: bool,
) {
    let t0 = std::time::Instant::now();
    let rows = items.len();
    let known = STATE.with(|s| s.borrow().lib_streams.iter().any(|st| st.id == stream));
    if !known {
        if !replace || !start(ui, stream, kind, &list_id, items) {
            return; // the tail of a request nobody wants any more
        }
    } else {
        apply(ui, stream, items, replace);
    }
    if done {
        finish(ui, stream);
    }
    tracing::info!(
        stream,
        rows,
        replace,
        done,
        ms = t0.elapsed().as_millis() as u64,
        "ui chunk"
    );
}

/// First chunk of a stream: pick where it goes. False = nobody wants it.
fn start(
    ui: &AppWindow,
    stream: u64,
    kind: LibraryKind,
    list_id: &Option<String>,
    items: Vec<Item>,
) -> bool {
    let for_tree = list_id.is_none()
        && STATE.with(|s| s.borrow().tree.iter().any(|t| t.kind == kind && t.loading));
    let fresh = STATE.with(|s| s.borrow().lib_request == Some((kind, list_id.clone())));
    let tab_name = STATE.with(|s| {
        let mut s = s.borrow_mut();
        match &s.open_as_tab {
            Some((k, i, _)) if fresh && *k == kind && list_id.as_ref() == Some(i) => {
                s.open_as_tab.take().map(|t| t.2)
            }
            _ => None,
        }
    });
    let play_pending = STATE.with(|s| {
        matches!(&s.borrow().play_on_load, Some((k, i)) if *k == kind && list_id.as_ref() == Some(i))
    });
    let target = if for_tree || play_pending {
        Target::Buffer
    } else if let Some(name) = tab_name {
        ui.global::<Panels>().set_busy(false);
        tabs::open(ui, name, "list", songs(items.clone()));
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(t) = s.tabs.last_mut() {
                t.source = list_id.clone().filter(|_| kind == LibraryKind::Playlist);
            }
        });
        Target::Tab(STATE.with(|s| s.borrow().tabs.last().map_or(0, |t| t.uid)))
    } else if fresh {
        // a newer page replaces the older page stream
        STATE.with(|s| {
            s.borrow_mut()
                .lib_streams
                .retain(|st| !matches!(st.target, Target::Page))
        });
        show_library_page(ui, kind, None, items.clone());
        Target::Page
    } else {
        return false;
    };
    STATE.with(|s| {
        s.borrow_mut().lib_streams.push(LibStream {
            id: stream,
            kind,
            list_id: list_id.clone(),
            acc: if matches!(target, Target::Buffer) {
                items
            } else {
                Vec::new()
            },
            target,
        });
    });
    true
}

fn apply(ui: &AppWindow, stream: u64, items: Vec<Item>, replace: bool) {
    enum Next {
        Nothing,
        Refresh(u64, Vec<Item>),
        Append(usize),
        Page(LibraryKind),
        AppendPage,
    }
    let next = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let Some(pos) = s.lib_streams.iter().position(|st| st.id == stream) else {
            return Next::Nothing;
        };
        let (target, kind, list_id) = {
            let st = &s.lib_streams[pos];
            (st.target, st.kind, st.list_id.clone())
        };
        match target {
            Target::Buffer => {
                let st = &mut s.lib_streams[pos];
                if replace {
                    st.acc = items;
                } else {
                    st.acc.extend(items);
                }
                Next::Nothing
            }
            Target::Tab(uid) => {
                let Some(t) = s.tabs.iter().position(|t| t.uid == uid) else {
                    s.lib_streams.remove(pos); // the tab was closed
                    return Next::Nothing;
                };
                let songs = songs(items);
                let cur = t + 1 == s.cur_tab;
                let before = s.tabs[t].items.len();
                if replace {
                    // the network copy of a list that is already shown: applied when the user
                    // is not in the middle of something (see `refresh_tick`)
                    return Next::Refresh(uid, songs);
                }
                s.tabs[t].items.extend(songs);
                if cur {
                    Next::Append(before)
                } else {
                    Next::Nothing
                }
            }
            Target::Page => {
                if s.lib_request != Some((kind, list_id)) {
                    s.lib_streams.remove(pos); // the user moved on
                    return Next::Nothing;
                }
                s.library_pending = Some(items);
                if replace {
                    Next::Page(kind)
                } else {
                    Next::AppendPage
                }
            }
        }
    });
    match next {
        Next::Refresh(uid, songs) => super::library_refresh::offer(ui, uid, songs),
        Next::Append(before) => table::append_rows(ui, before),
        Next::Page(kind) => {
            let items = STATE.with(|s| s.borrow_mut().library_pending.take().unwrap_or_default());
            show_library_page(ui, kind, None, items);
        }
        Next::AppendPage => {
            let items = STATE.with(|s| s.borrow_mut().library_pending.take().unwrap_or_default());
            append_page(ui, items);
        }
        Next::Nothing => {}
    }
}

/// More rows for the Library page: one batch into the existing model.
fn append_page(ui: &AppWindow, items: Vec<Item>) {
    let rows: Vec<crate::TrackData> = items.iter().map(|i| panel_row(i, false)).collect();
    let core = STATE.with(|s| s.borrow().core.clone());
    if let Some(core) = core {
        super::panels::request_thumbs(&core, &items);
    }
    STATE.with(|s| s.borrow_mut().library.extend(items));
    let model = ui.global::<Panels>().get_library();
    if let Some(vm) = model
        .as_any()
        .downcast_ref::<slint::VecModel<crate::TrackData>>()
    {
        vm.extend(rows);
    }
}

/// The load failed: forget its streams (their rows stay as they were; a tree buffer is dropped,
/// not delivered as an empty list).
pub fn failed(kind: LibraryKind, list_id: &Option<String>) {
    let tabs_touched = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let before = s.lib_streams.len();
        let mut tab = false;
        s.lib_streams.retain(|st| {
            let hit = st.kind == kind && st.list_id == *list_id;
            tab |= hit && matches!(st.target, Target::Tab(_));
            !hit
        });
        tab && s.lib_streams.len() != before
    });
    if tabs_touched {
        tabs::save();
    }
}

fn finish(ui: &AppWindow, stream: u64) {
    let st = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let pos = s.lib_streams.iter().position(|st| st.id == stream)?;
        Some(s.lib_streams.remove(pos))
    });
    let Some(st) = st else { return };
    match st.target {
        Target::Buffer => handle_event(
            ui,
            Event::Library {
                kind: st.kind,
                id: st.list_id,
                title: None,
                items: st.acc,
            },
        ),
        Target::Tab(_) => tabs::save(),
        Target::Page => {}
    }
}
