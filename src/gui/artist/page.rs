//! The artist page contents: models, playing marks, lazy covers, banner, screenshot fixtures.
use super::*;

/// An artist page arrived (cached copy first).
pub fn arrived(ui: &AppWindow, id: String, info: ArtistInfo) {
    if STATE.with(|s| s.borrow().artist.current != id) {
        return;
    }
    let page = ui.global::<ArtistPage>();
    page.set_name(info.name.clone().into());
    let stats: Vec<&str> = [info.subscribers.as_deref(), info.monthly.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    page.set_stats(stats.join(" • ").into());
    page.set_description(info.description.clone().into());
    page.set_has_all_songs(info.songs_playlist.is_some());
    let songs = Rc::new(VecModel::from(
        (0..info.songs.len())
            .map(|i| song_row(&info, i))
            .collect::<Vec<_>>(),
    ));
    page.set_songs(ModelRc::from(songs.clone()));
    let mut rows: Vec<ShelfData> = Vec::new();
    let mut models = Vec::new();
    let mut lists = Vec::new();
    for (row, m, items) in shelf_rows(&info.shelves) {
        rows.push(row);
        models.push(m);
        lists.push(items);
    }
    page.set_shelves(ModelRc::new(VecModel::from(rows)));
    page.set_busy(false);
    let image = info.image.clone();
    // the tab carries the artist's name
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let cur = s.cur_tab;
        if let Some(t) = s
            .tabs
            .get_mut(cur.wrapping_sub(1))
            .filter(|t| super::super::tabs::is_page_kind(&t.kind))
        {
            t.name.clone_from(&info.name);
        }
        s.artist.songs = Some(songs);
        s.artist.shelves = lists;
        s.artist.models = models;
        s.artist.info = Some(info);
    });
    super::super::tabs::save();
    super::super::table::refresh(ui);
    refresh_marks();
    // the banner / avatar picture (arrives as a cover under "<url>#card")
    if let (Some(url), Some(core)) = (image, STATE.with(|s| s.borrow().core.clone())) {
        let key = format!("{url}#card");
        let known = STATE.with(|s| {
            s.borrow()
                .artist
                .header
                .as_ref()
                .is_some_and(|h| h.0 == url)
        });
        if known {
            cover_ready(ui);
        } else if STATE.with(|s| s.borrow_mut().thumb_pending.insert(key)) {
            core.send(Command::LoadCovers(vec![url]));
        }
    }
    // thumbnails of the songs
    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
        let items = STATE.with(|s| s.borrow().artist.info.as_ref().map(|i| i.songs.clone()));
        if let Some(items) = items {
            super::super::panels::request_thumbs(&core, &items);
        }
    }
}

/// Playing marks of the top songs (cheap, in place). The state borrow is released before the
/// model is touched: a model change runs Slint code synchronously.
pub fn refresh_marks() {
    use slint::Model;
    let Some((model, marks)) = STATE.with(|s| {
        let s = s.borrow();
        let (Some(info), Some(model)) = (&s.artist.info, &s.artist.songs) else {
            return None;
        };
        let marks: Vec<bool> = info
            .songs
            .iter()
            .map(|it| s.snap.video_id.is_some() && it.video_id == s.snap.video_id)
            .collect();
        Some((model.clone(), marks))
    }) else {
        return;
    };
    for (n, p) in marks.into_iter().enumerate() {
        if let Some(mut r) = model.row_data(n)
            && r.playing != p
        {
            r.playing = p;
            model.set_row_data(n, r);
        }
    }
}

/// A cover or thumbnail arrived: patch the page.
pub fn patch_thumb(
    ui: &AppWindow,
    url: &str,
    image: &slint::Image,
    rgba: &std::sync::Arc<image::RgbaImage>,
) {
    use slint::Model;
    let (card_url, is_card) = match url.strip_suffix("#card") {
        Some(u) => (u, true),
        None => (url, false),
    };
    let is_header = STATE.with(|s| {
        s.borrow()
            .artist
            .info
            .as_ref()
            .and_then(|i| i.image.as_deref())
            == Some(card_url)
    });
    // find the rows under the borrow, set them after it is released (model changes run Slint code)
    let (cards, songs) = STATE.with(|s| {
        let s = s.borrow();
        let mut cards = Vec::new();
        let mut songs = Vec::new();
        if is_card {
            for (items, model) in s.artist.shelves.iter().zip(&s.artist.models) {
                for (n, it) in items.iter().enumerate() {
                    if it.thumbnail.as_deref() == Some(card_url) {
                        cards.push((model.clone(), n));
                    }
                }
            }
        } else if let (Some(info), Some(model)) = (&s.artist.info, &s.artist.songs) {
            for (n, it) in info.songs.iter().enumerate() {
                if it.thumbnail.as_deref() == Some(card_url) {
                    songs.push((model.clone(), n));
                }
            }
        }
        (cards, songs)
    });
    for (model, n) in cards {
        if let Some(mut c) = model.row_data(n) {
            (c.thumb, c.has_thumb) = (image.clone(), true);
            model.set_row_data(n, c);
        }
    }
    for (model, n) in songs {
        if let Some(mut r) = model.row_data(n) {
            (r.thumb, r.has_thumb) = (image.clone(), true);
            model.set_row_data(n, r);
        }
    }
    if is_card && is_header {
        STATE.with(|s| s.borrow_mut().artist.header = Some((card_url.to_string(), rgba.clone())));
        cover_ready(ui);
    }
}

/// The header picture is available: round avatar plus the blurred, tinted banner.
fn cover_ready(ui: &AppWindow) {
    let Some((url, img)) = STATE.with(|s| s.borrow().artist.header.clone()) else {
        return;
    };
    if STATE.with(|s| {
        s.borrow()
            .artist
            .info
            .as_ref()
            .and_then(|i| i.image.as_deref())
            != Some(url.as_str())
    }) {
        return;
    }
    let page = ui.global::<ArtistPage>();
    let mut avatar = (*img).clone();
    if !STATE.with(|s| s.borrow().artist.album) {
        crate::art::circle_mask(&mut avatar);
    }
    page.set_avatar(crate::art::to_slint(&avatar));
    page.set_has_avatar(true);
    // wide banner: the picture blown up, blurred and tinted with the window colour
    let c = ui.global::<crate::Theme>().get_window();
    let params = crate::core::background::Params {
        fit: crate::core::background::Fit::Cover,
        pos: crate::core::background::Pos::parse("center"),
        blur: 16.0,
        tint: crate::core::theme::Rgb(c.red(), c.green(), c.blue()),
        tint_alpha: 0.45,
    };
    let place = move |ui: &AppWindow, banner: image::RgbaImage| {
        // drop the result when another page was opened meanwhile
        if STATE.with(|s| s.borrow().artist.header.as_ref().map(|h| h.0.clone()))
            != Some(url.clone())
        {
            return;
        }
        let page = ui.global::<ArtistPage>();
        page.set_banner(crate::art::to_slint(&banner));
        page.set_has_banner(true);
    };
    if crate::config::NO_PERSIST.load(std::sync::atomic::Ordering::Relaxed) {
        // screenshot mode has no event loop
        place(
            ui,
            crate::core::background::render(&img, 1200, 260, &params),
        );
        return;
    }
    // blur and tint on a worker thread (the picture is blown up to the banner size)
    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let banner = crate::core::background::render(&img, 1200, 260, &params);
        let _ = weak.upgrade_in_event_loop(move |ui| place(&ui, banner));
    });
}

/// Screenshot fixture: show `info` as the artist tab without any network.
pub fn fixture(ui: &AppWindow, info: ArtistInfo, header: std::sync::Arc<image::RgbaImage>) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tabs.push(TabState {
            source: Some(info.id.clone()),
            uid: new_uid(),
            name: info.name.clone(),
            kind: "artist".into(),
            items: Vec::new(),
        });
        s.cur_tab = s.tabs.len();
        s.artist.current = info.id.clone();
        s.artist.header = info.image.clone().map(|u| (u, header));
    });
    ui.global::<ArtistPage>().set_active(true);
    let id = info.id.clone();
    arrived(ui, id, info);
    cover_ready(ui);
    super::super::table::refresh(ui);
}

/// Screenshot fixture: show an album page.
pub fn fixture_album(
    ui: &AppWindow,
    page: crate::core::api::browse::AlbumPage,
    header: std::sync::Arc<image::RgbaImage>,
) {
    let id = "MPREfixturealbum".to_string();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.tabs.push(TabState {
            source: Some(id.clone()),
            uid: new_uid(),
            name: page.title.clone(),
            kind: "album".into(),
            items: Vec::new(),
        });
        s.cur_tab = s.tabs.len();
        s.artist.current = id.clone();
        s.artist.album = true;
        s.artist.header = page.thumbnail.clone().map(|u| (u, header));
    });
    let p = ui.global::<ArtistPage>();
    p.set_active(true);
    p.set_album_mode(true);
    p.set_songs_title("Tracks".into());
    album_arrived(ui, id, page);
    cover_ready(ui);
    super::super::table::refresh(ui);
}
