//! Messages between the UI thread and the core runtime.
use super::model::Item;
use super::queue::Repeat;
use image::RgbaImage;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryKind {
    Liked,
    Playlists,
    Albums,
    Artists,
    History,
    /// Tracks of one playlist (id carried by the event).
    Playlist,
    /// Top songs followed by albums.
    Artist,
    Album,
}

/// UI → core.
#[derive(Debug, Clone)]
pub enum Command {
    Search {
        query: String,
        filter: Option<String>,
    },
    /// Replace the queue and start playing `index`.
    PlayItems {
        items: Vec<Item>,
        index: usize,
    },
    Enqueue(Item),
    /// Append to the queue and start playing it.
    AddAndPlay(Item),
    ClearQueue,
    PlayNext(Item),
    Toggle,
    /// Explicit play / pause (media keys, MPRIS): never flip state.
    Play,
    Pause,
    Stop,
    Next,
    Prev,
    SeekFrac(f64),
    /// Relative / absolute seek in seconds (media keys, MPRIS).
    SeekBy(f64),
    SeekTo(f64),
    SetVolume(u8),
    LoadLibrary(LibraryKind),
    /// Related shelves for a track (Discover home, bottom).
    LoadRelated(String),
    /// Next page of the current search results.
    SearchMore,
    /// Load an artist page (`UC…`); the cached copy arrives first.
    LoadArtistPage {
        id: String,
        force: bool,
    },
    /// A radio / mix from a playlist id (`RD…`): shown as a tab, or played at once.
    MixFromPlaylist {
        playlist: String,
        name: String,
        play: bool,
    },
    /// Look an artist up by name (tracks whose artist has no link) and open the first match.
    FindArtist(String),
    /// Load an album page (`MPRE…`).
    LoadAlbumPage(String),
    /// Refresh the list of playlists tracks can be added to.
    LoadAddTargets,
    /// Add tracks to a playlist; without `force` duplicates are reported first.
    AddToPlaylist {
        playlist: String,
        title: String,
        items: Vec<Item>,
        force: bool,
    },
    CreatePlaylist {
        title: String,
        /// PRIVATE | UNLISTED | PUBLIC
        privacy: String,
        items: Vec<Item>,
    },
    /// Remove what a previous add put there (`(videoId, setVideoId)` pairs).
    UndoAdd {
        playlist: String,
        entries: Vec<(String, String)>,
    },
    /// Open a Discover section: `home`, `new`, `charts` (country), `moods`, `mood` (params).
    /// `force` skips the "fresh enough" check of the disk cache.
    LoadDiscover {
        section: String,
        country: String,
        params: Option<String>,
        force: bool,
    },
    LoadPlaylist(String),
    LoadArtist(String),
    LoadAlbum(String),
    /// Start playing `items` in shuffled order from a random track.
    ShuffleAll(Vec<Item>),
    ToggleLike,
    /// Like/unlike a specific track (table context menu).
    ToggleLikeFor(String),
    /// Fetch the radio for a track and open it as a new tab.
    StartRadio(Item),
    SetAudioFormat(String),
    /// Cookie file changed (first-run sign-in): re-point mpv's yt-dlp at it.
    ReloadCookies(Option<std::path::PathBuf>),
    QueueJump(usize),
    /// New order (permutation of old indices) after sorting / multi-move in the table.
    QueueReorder(Vec<usize>),
    QueueRemoveMany(Vec<usize>),
    /// Insert tracks into the queue before position `at` (drag & drop from the panels).
    QueueInsert {
        items: Vec<Item>,
        at: usize,
    },
    /// Fetch small list thumbnails; answered with `Event::Thumb` per url.
    LoadThumbs(Vec<String>),
    /// Full-size covers for Discover cards; arrive as `Event::Thumb` keyed `<url>#card`.
    LoadCovers(Vec<String>),
    CycleRepeat,
    ToggleShuffle,
    Quit,
}

/// What the toolbar, now-playing widget and status bar show. Cheap to clone and send.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub has_track: bool,
    pub title: String,
    pub artist_line: String,
    pub video_id: Option<String>,
    pub pos: f64,
    pub duration: f64,
    pub buffered: f64,
    pub playing: bool,
    pub volume: u8,
    pub repeat: Option<Repeat>,
    pub shuffle: bool,
    pub queue_len: usize,
    pub liked: bool,
    pub thumbnail: Option<String>,
    pub artist_id: Option<String>,
    pub album_id: Option<String>,
    pub format: String,
    pub artist_name: String,
    pub album_name: String,
}

/// Core → UI.
#[derive(Debug, Clone)]
pub enum Event {
    /// Search results: the first page (`more` = a next page exists).
    Results {
        items: Vec<Item>,
        more: bool,
    },
    /// The next page of the current search; `more` = yet another page exists.
    ResultsMore {
        items: Vec<Item>,
        more: bool,
    },
    /// A small thumbnail for list rows, keyed by the url that was requested.
    Thumb {
        url: String,
        img: Arc<RgbaImage>,
    },
    /// A list to open in a new playlist tab (radio).
    Tab {
        name: String,
        kind: String,
        items: Vec<Item>,
    },
    /// A library / playlist load failed (or came back empty because the session is not signed
    /// in): nothing must be shown as an empty list.
    LibraryFailed {
        kind: LibraryKind,
        id: Option<String>,
        message: String,
    },
    /// `title` is set for artist/album pages.
    Library {
        kind: LibraryKind,
        id: Option<String>,
        title: Option<String>,
        items: Vec<Item>,
    },
    /// Discover content for `key` (`section-country-params`): from the disk cache first
    /// , then fresh from the network.
    Discover {
        key: String,
        data: crate::core::api::discover_parse::DiscoverData,
    },
    /// The channel id found for a name (see `Command::FindArtist`).
    ArtistFound(String),
    /// An album page.
    Album {
        id: String,
        page: crate::core::api::browse::AlbumPage,
    },
    /// Tracks to start playing right away (a mix the user asked to play).
    PlayNow(Vec<Item>),
    /// An artist page (cached copy first, then fresh when it changed).
    Artist {
        id: String,
        info: crate::core::api::artist_info::ArtistInfo,
    },
    /// (id, title) of the playlists tracks can be added to.
    AddTargets(Vec<crate::core::api::playlist_edit::AddTarget>),
    PlaylistAdded {
        playlist: String,
        title: String,
        items: Vec<Item>,
        /// `(videoId, setVideoId)` for the undo.
        entries: Vec<(String, String)>,
        /// The playlist was just created (no undo).
        created: bool,
    },
    /// Some of the tracks are already in the playlist (nothing was added).
    PlaylistDuplicates {
        playlist: String,
        title: String,
        items: Vec<Item>,
        dupes: usize,
    },
    PlaylistRemoved {
        playlist: String,
        video_ids: Vec<String>,
    },
    /// Related shelves of a track.
    Related {
        video_id: String,
        shelves: Vec<crate::core::api::discover_parse::Shelf>,
    },
    /// A playlist / liked songs arriving in pieces (or from the disk cache first): `replace`
    /// starts or replaces the list, otherwise `items` are appended; `done` ends the stream.
    LibraryChunk {
        /// Identifies one fetch (several can be in flight).
        stream: u64,
        kind: LibraryKind,
        id: Option<String>,
        items: Vec<Item>,
        replace: bool,
        done: bool,
    },
    /// Lyrics for a track (None = not found).
    Lyrics {
        video_id: String,
        lyrics: Option<crate::core::lyrics::Lyrics>,
    },
    /// Whole queue plus the index of the current item.
    Queue {
        items: Vec<Item>,
        current: Option<usize>,
    },
    State(Snapshot),
    /// Square-cropped cover for the current track.
    Cover(Arc<RgbaImage>),
    Error(String),
}
