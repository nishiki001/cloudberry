//! Library tree: lazy sections (Liked, Playlists, Albums, Artists, History), flattened for Slint.
use super::super::state::*;
use super::request_thumbs;
use crate::core::model::{Item, Kind};
use crate::core::msg::{Command, LibraryKind};
use crate::core::runtime::CoreHandle;
use crate::{AppWindow, Panels, TreeRow};
use slint::ComponentHandle;
use slint::{Image, Model, ModelRc};

/// Children shown per section (Liked/History can hold thousands; the full list opens as a tab).
const MAX_CHILDREN: usize = 300;

pub fn init_tree() {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tree = [
            (LibraryKind::Liked, "Liked songs"),
            (LibraryKind::Playlists, "Playlists"),
            (LibraryKind::Albums, "Albums"),
            (LibraryKind::Artists, "Artists"),
            (LibraryKind::History, "History"),
        ]
        .into_iter()
        .map(|(kind, label)| TreeSection {
            kind,
            label,
            expanded: false,
            loading: false,
            items: None,
        })
        .collect();
        s.search_filter = "songs".into();
    });
}

pub(super) fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Song => "song",
        Kind::Video => "video",
        Kind::Album => "album",
        Kind::Artist => "artist",
        Kind::Playlist => "playlist",
    }
}

pub(super) fn artist_names(i: &Item) -> String {
    i.artists
        .iter()
        .map(|a| a.name.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Flattened tree: (section index, child index or None) per visible row.
fn flat_index(s: &UiState) -> Vec<(usize, Option<usize>)> {
    let mut v = Vec::new();
    for (si, sec) in s.tree.iter().enumerate() {
        v.push((si, None));
        if sec.expanded {
            let n = sec.items.as_ref().map_or(0, |i| i.len().min(MAX_CHILDREN));
            v.extend((0..n).map(|c| (si, Some(c))));
        }
    }
    v
}

pub fn refresh_tree(_ui: &AppWindow) {
    STATE.with(|s| {
        let s = s.borrow();
        let rows: Vec<TreeRow> = flat_index(&s)
            .into_iter()
            .map(|(si, ci)| {
                let sec = &s.tree[si];
                match ci {
                    None => TreeRow {
                        label: match &sec.items {
                            Some(items) => format!(
                                "{} ({}{})",
                                sec.label,
                                items.len(),
                                if items.len() > MAX_CHILDREN { "+" } else { "" }
                            ),
                            None => sec.label.to_string(),
                        }
                        .into(),
                        sub: Default::default(),
                        depth: 0,
                        kind: "section".into(),
                        expanded: sec.expanded,
                        loading: sec.loading,
                        playing: false,
                        thumb: Image::default(),
                        has_thumb: false,
                    },
                    Some(c) => {
                        let it = &sec.items.as_ref().unwrap()[c];
                        let thumb = it.thumbnail.as_ref().and_then(|u| s.thumbs.get(u)).cloned();
                        TreeRow {
                            label: it.title.clone().into(),
                            sub: if matches!(it.kind, Kind::Song | Kind::Video) {
                                artist_names(it)
                            } else {
                                it.subtitle.clone()
                            }
                            .into(),
                            depth: 1,
                            kind: kind_name(it.kind).into(),
                            expanded: false,
                            loading: false,
                            playing: s.snap.video_id.is_some() && it.video_id == s.snap.video_id,
                            has_thumb: thumb.is_some(),
                            thumb: thumb.unwrap_or_default(),
                        }
                    }
                }
            })
            .collect();
        // patch in place when the shape is unchanged (keeps clicks alive, no full reset)
        let m = &s.tree_model;
        if m.row_count() == rows.len() {
            for (n, r) in rows.into_iter().enumerate() {
                if m.row_data(n).is_none_or(|old| old != r) {
                    m.set_row_data(n, r);
                }
            }
        } else {
            m.set_vec(rows);
        }
    });
}

pub fn install_models(ui: &AppWindow) {
    STATE.with(|s| {
        ui.global::<Panels>()
            .set_tree(ModelRc::from(s.borrow().tree_model.clone()))
    });
}

/// Library tree data arrived for a section.
pub fn tree_loaded(ui: &AppWindow, core: &CoreHandle, kind: LibraryKind, items: Vec<Item>) {
    request_thumbs(
        core,
        &items.iter().take(MAX_CHILDREN).cloned().collect::<Vec<_>>(),
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if let Some(sec) = s.tree.iter_mut().find(|t| t.kind == kind) {
            sec.items = Some(items);
            sec.loading = false;
        }
    });
    install_models(ui);
    refresh_tree(ui);
}

/// The load of one section failed: that section stops spinning and can be retried by
/// collapsing and expanding it (the others keep loading).
pub fn tree_failed(ui: &AppWindow, kind: LibraryKind) {
    let any = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut any = false;
        for t in s.tree.iter_mut().filter(|t| t.kind == kind && t.loading) {
            t.loading = false;
            t.expanded = false;
            any = true;
        }
        any
    });
    if any {
        refresh_tree(ui);
    }
}

/// What a tree row stands for.
pub(super) enum Target {
    Section(usize),
    Item(Box<Item>),
}

pub(super) fn target(row: usize) -> Option<Target> {
    STATE.with(|s| {
        let s = s.borrow();
        let (si, ci) = *flat_index(&s).get(row)?;
        match ci {
            None => Some(Target::Section(si)),
            Some(c) => s.tree[si]
                .items
                .as_ref()
                .and_then(|i| i.get(c))
                .cloned()
                .map(|i| Target::Item(Box::new(i))),
        }
    })
}

pub(super) fn toggle_section(ui: &AppWindow, core: &CoreHandle, si: usize) {
    let load = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let sec = &mut s.tree[si];
        sec.expanded = !sec.expanded;
        let need = sec.expanded && sec.items.is_none() && !sec.loading;
        if need {
            sec.loading = true;
        }
        let kind = sec.kind;
        need.then_some(kind)
    });
    if let Some(kind) = load {
        core.send(Command::LoadLibrary(kind));
    }
    install_models(ui);
    refresh_tree(ui);
}
