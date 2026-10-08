//! UI-thread state shared by the glue modules.
use crate::AppWindow;
use crate::Art;
use crate::TrackData;
use crate::art;
use crate::core::model::Item;
use crate::core::msg::{LibraryKind, Snapshot};
use slint::{ComponentHandle, SharedString, Timer, TimerMode};
use std::cell::RefCell;
use std::time::Duration;

pub struct TreeSection {
    pub kind: LibraryKind,
    pub label: &'static str,
    pub expanded: bool,
    pub loading: bool,
    pub items: Option<Vec<Item>>,
}

/// Process-unique id for a tab: streams find "their" tab by it even after tabs are closed or moved.
pub fn new_uid() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

pub struct TabState {
    pub uid: u64,
    pub source: Option<String>,
    pub name: String,
    pub kind: String,
    pub items: Vec<Item>,
}

#[derive(Default)]
pub struct UiState {
    /// Open playlist tabs (the Queue is tab 0 and not listed here).
    pub tabs: Vec<TabState>,
    /// 0 = Queue, i = tabs[i - 1].
    pub cur_tab: usize,
    pub last_list_tab: usize,
    pub rename_tab: Option<usize>,
    /// Selected rows of the current tab (sorted).
    pub sel: Vec<usize>,
    pub anchor: usize,
    /// The last row press carried Ctrl/Shift (so the following click must not collapse the selection).
    pub click_mod: bool,
    /// Track keys of the rows in `rows_model` (for the in-place diff), and the tab they belong to.
    pub row_keys: Vec<String>,
    pub shown_tab: Option<u64>,
    /// The row a pointer gesture started on: (row, track key, when). Actions use the key.
    pub gesture: Option<(usize, String, std::time::Instant)>,
    /// Last user interaction with the table (press, menu, double-click, scroll gesture).
    pub touched: Option<std::time::Instant>,
    /// A background refresh of a playlist tab waiting for the user to be idle: (tab uid, tracks).
    /// (tab uid, the tab's items when the copy arrived, the copy). The copy is dropped when the
    /// user edited the list meanwhile.
    pub deferred_refresh: Vec<(u64, Vec<Item>, Vec<Item>)>,
    /// How long the table must be left alone before a refresh is applied (ms; 0 = 3000).
    pub quiet_ms: u64,
    // persistent models behind the table (updated in place, see gui/table/mod.rs)
    pub rows_model: std::rc::Rc<slint::VecModel<TrackData>>,
    pub cols_model: std::rc::Rc<slint::VecModel<crate::ColumnData>>,
    pub all_cols_model: std::rc::Rc<slint::VecModel<crate::ColumnData>>,
    pub sort: Option<(crate::core::columns::Col, bool)>,
    pub table_w: f32,
    /// Column widths when the current resize drag started.
    pub resize_base: Option<crate::core::columns::ColumnConfig>,
    /// The next Library page that arrives opens as a tab with this name.
    pub open_as_tab: Option<(LibraryKind, String, String)>,
    pub tree: Vec<TreeSection>,
    pub tree_model: std::rc::Rc<slint::VecModel<crate::TreeRow>>,
    /// Small thumbnails by item thumbnail url (already decoded to Slint images).
    pub thumbs: std::collections::HashMap<String, slint::Image>,
    pub thumb_pending: std::collections::HashSet<String>,
    pub search_filter: String,
    pub last_query: String,
    pub results: Vec<Item>,
    /// The current search has a next page / one is being fetched.
    pub search_more: bool,
    pub search_loading: bool,
    pub library: Vec<Item>,
    pub library_mode: String,
    pub toast_action: Option<ToastAction>,
    /// Tracks waiting for a playlist choice (one of the two "Add to playlist" dialogs is open).
    pub pending_add: Option<Vec<Item>>,
    /// The playlists tracks can be added to, as the server listed them (session cache).
    pub add_targets: Vec<crate::core::api::playlist_edit::AddTarget>,
    /// The list was answered at least once (an empty answer is not "still loading").
    pub add_targets_loaded: bool,
    /// The rows of the "Existing playlist" dialog (filtered, in display order).
    pub add_shown: Vec<crate::core::api::playlist_edit::AddTarget>,
    pub discover: super::discover::DiscoverState,
    pub artist: super::artist::ArtistState,
    /// The window (for code that has no `ui` argument, e.g. opening an artist from a card).
    pub ui: Option<slint::Weak<crate::AppWindow>>,
    pub discover_moods: Vec<String>,
    /// An album / playlist / artist to start playing as soon as it has been loaded.
    pub play_on_load: Option<(crate::core::msg::LibraryKind, String)>,
    /// The streams of chunks currently being applied (see gui/library_stream.rs).
    pub lib_streams: Vec<super::library_stream::LibStream>,
    pub library_pending: Option<Vec<Item>>,
    pub lib_request: Option<(LibraryKind, Option<String>)>,
    pub pending_title: String,
    pub lyrics: Option<crate::core::lyrics::Lyrics>,
    pub queue_items: Vec<Item>,
    pub queue_current: Option<usize>,
    pub last_video: Option<String>,
    pub snap: Snapshot,
    pub progressed: bool,
    pub toast_timer: Timer,
    pub media: Option<crate::mediactl::MediaCtl>,
    pub cfg: crate::config::Config,
    pub analyzer: Option<super::analyzer::AnalyzerState>,
    pub core: Option<crate::core::runtime::CoreHandle>,
}

thread_local! {
    pub static STATE: RefCell<UiState> = RefCell::new(UiState::default());
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

pub fn row(i: &Item, playing: bool) -> TrackData {
    TrackData {
        title: i.title.clone().into(),
        artist: i
            .artists
            .iter()
            .map(|a| a.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
            .into(),
        album: i.album.clone().unwrap_or_default().into(),
        year: SharedString::new(),
        time: i.duration.clone().unwrap_or_default().into(),
        playing,
        liked: false,
        selected: false,
        kind: match i.kind {
            crate::core::model::Kind::Song => "song",
            crate::core::model::Kind::Video => "video",
            crate::core::model::Kind::Album => "album",
            crate::core::model::Kind::Artist => "artist",
            crate::core::model::Kind::Playlist => "playlist",
        }
        .into(),
        thumb: Default::default(),
        has_thumb: false,
    }
}

/// Row for a list panel: non-song items show their subtitle in the artist column, and a
/// thumbnail that is already cached.
pub fn panel_row(i: &Item, playing: bool) -> TrackData {
    let mut r = row(i, playing);
    if !matches!(
        i.kind,
        crate::core::model::Kind::Song | crate::core::model::Kind::Video
    ) {
        r.artist = i.subtitle.clone().into();
    }
    if let Some(img) = i
        .thumbnail
        .as_ref()
        .and_then(|u| STATE.with(|s| s.borrow().thumbs.get(u).cloned()))
    {
        r.thumb = img;
        r.has_thumb = true;
    }
    r
}

pub fn install_art(ui: &AppWindow) {
    let a = ui.global::<Art>();
    a.set_sheen(art::to_slint(&art::sheen(512)));
    a.set_blank_disc(art::to_slint(&art::blank_disc(512, false)));
}

pub fn show_toast(ui: &AppWindow, msg: &str) {
    toast(ui, msg, None);
}

/// A toast with an action button ("Undo", "Add anyway"); the action stays available for 10 s.
pub fn show_toast_action(ui: &AppWindow, msg: &str, label: &str, action: ToastAction) {
    STATE.with(|s| s.borrow_mut().toast_action = Some(action));
    toast(ui, msg, Some(label));
}

fn toast(ui: &AppWindow, msg: &str, action: Option<&str>) {
    ui.set_status_message(msg.into());
    let dialogs = ui.global::<crate::Dialogs>();
    dialogs.set_status_action(action.unwrap_or_default().into());
    if action.is_none() {
        STATE.with(|s| s.borrow_mut().toast_action = None);
    }
    let weak = ui.as_weak();
    let secs = if action.is_some() { 10 } else { 4 };
    STATE.with(|s| {
        s.borrow().toast_timer.start(
            TimerMode::SingleShot,
            Duration::from_secs(secs),
            move || {
                if let Some(ui) = weak.upgrade() {
                    ui.set_status_message(SharedString::new());
                    ui.global::<crate::Dialogs>()
                        .set_status_action(SharedString::new());
                    STATE.with(|s| s.borrow_mut().toast_action = None);
                }
            },
        );
    });
}

/// What the status bar action button does.
pub enum ToastAction {
    Undo {
        playlist: String,
        entries: Vec<(String, String)>,
    },
    AddAnyway {
        playlist: String,
        title: String,
        items: Vec<Item>,
    },
}
