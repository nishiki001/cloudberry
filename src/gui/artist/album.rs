//! The album page: the artist page tab in "album mode" (square cover, byline, track list).
use super::*;
use crate::core::api::browse::AlbumPage;

/// Open an album page (an existing tab for it is reused).
pub fn open_album(ui: &AppWindow, id: String) {
    super::super::tabs::open_special(ui, "Album", "album", &id);
}

pub fn open_album_global(id: String) {
    if let Some(ui) = STATE.with(|s| s.borrow().ui.as_ref().and_then(|w| w.upgrade())) {
        open_album(&ui, id);
    }
}

/// An album page arrived: shown through the same models as an artist page.
pub fn album_arrived(ui: &AppWindow, id: String, page: AlbumPage) {
    if STATE.with(|s| s.borrow().artist.current != id) {
        return;
    }
    let secs: u32 = page.tracks.iter().filter_map(|t| t.duration_secs).sum();
    let mut meta: Vec<String> = Vec::new();
    meta.extend(page.year.clone());
    meta.push(format!("{} tracks", page.tracks.len()));
    if secs > 0 {
        meta.push(format!("{} min", secs.div_ceil(60)));
    }
    let info = ArtistInfo {
        id: id.clone(),
        name: page.title.clone(),
        description: String::new(),
        subscribers: Some(meta.join(" • ")),
        monthly: None,
        image: page.thumbnail.clone(),
        songs: page.tracks,
        plays: Vec::new(),
        songs_playlist: None,
        play: None,
        radio: None,
        shelves: Vec::new(),
    };
    let p = ui.global::<ArtistPage>();
    p.set_byline(page.artist.into());
    STATE.with(|s| s.borrow_mut().artist.byline_artist = page.artist_id);
    arrived(ui, id, info);
}
