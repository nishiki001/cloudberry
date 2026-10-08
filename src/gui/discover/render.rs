//! Discover shelves: building the Slint models, Related shelves, lazy cover patching.
use super::*;
use crate::core::api::discover_parse::ShelfKind;

pub(crate) fn card(i: &Item, songs: bool) -> CardData {
    let artists: Vec<&str> = i.artists.iter().map(|a| a.name.as_str()).collect();
    let subtitle = if songs && !artists.is_empty() {
        artists.join(", ")
    } else {
        i.subtitle.clone()
    };
    let thumb = i
        .thumbnail
        .as_ref()
        .and_then(|u| STATE.with(|s| s.borrow().thumbs.get(&format!("{u}#card")).cloned()));
    CardData {
        title: i.title.clone().into(),
        subtitle: subtitle.into(),
        kind: match i.kind {
            Kind::Song => "song",
            Kind::Video => "video",
            Kind::Album => "album",
            Kind::Artist => "artist",
            Kind::Playlist => "playlist",
        }
        .into(),
        has_thumb: thumb.is_some(),
        thumb: thumb.unwrap_or_default(),
    }
}

/// Content for `key` arrived (cached first, fresh later).
pub fn arrived(ui: &AppWindow, key: String, data: DiscoverData) {
    if STATE.with(|s| s.borrow().discover.key != key) {
        return; // the user moved on
    }
    STATE.with(|s| s.borrow_mut().discover.last = data);
    render(ui);
}

/// Related shelves of the playing track arrived.
pub fn related_arrived(
    ui: &AppWindow,
    video_id: String,
    shelves: Vec<crate::core::api::discover_parse::Shelf>,
) {
    let current = STATE.with(|s| s.borrow().snap.video_id.clone());
    if current.as_deref() != Some(video_id.as_str()) {
        return;
    }
    STATE.with(|s| s.borrow_mut().discover.related = Some((video_id, shelves)));
    if STATE.with(|s| s.borrow().discover.section == "home") && ui.global::<Discover>().get_active()
    {
        refresh_related();
    }
}

pub(crate) fn shelf_rows(
    shelves: &[crate::core::api::discover_parse::Shelf],
) -> Vec<(ShelfData, Rc<VecModel<CardData>>, Vec<Item>)> {
    shelves
        .iter()
        .map(|sh| {
            let songs = sh.kind == ShelfKind::Songs;
            let m = Rc::new(VecModel::from(
                sh.items.iter().map(|i| card(i, songs)).collect::<Vec<_>>(),
            ));
            (
                ShelfData {
                    title: sh.title.clone().into(),
                    songs,
                    items: ModelRc::from(m.clone()),
                },
                m,
                sh.items.clone(),
            )
        })
        .collect()
}

/// Swap only the "Related" rows at the bottom of Home (the shelves above keep their scroll).
/// No `STATE` borrow is held while building rows (`card` reads it) or touching the Slint model.
fn refresh_related() {
    use slint::Model;
    let (model, base, related) = STATE.with(|s| {
        let s = s.borrow();
        (
            s.discover.shelves_model.clone(),
            s.discover.base_len,
            s.discover.related.clone(),
        )
    });
    let Some(model) = model else { return };
    let shelves: Vec<_> = related
        .map(|(_, v)| v)
        .unwrap_or_default()
        .into_iter()
        .filter(|sh| !sh.title.is_empty())
        .map(|mut sh| {
            sh.title = format!("Related: {}", sh.title);
            sh
        })
        .collect();
    let built = shelf_rows(&shelves);
    let mut rows = Vec::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.discover.shelves.truncate(base);
        s.discover.models.truncate(base);
        for (row, m, items) in built {
            rows.push(row);
            s.discover.models.push(m);
            s.discover.shelves.push(items);
        }
    });
    while model.row_count() > base {
        model.remove(model.row_count() - 1);
    }
    for row in rows {
        model.push(row);
    }
}

/// Ask for the related shelves of the playing track (once per track) while Home is shown.
pub fn want_related(core: &CoreHandle) {
    let vid = STATE.with(|s| {
        let s = s.borrow();
        let vid = s.snap.video_id.clone()?;
        let have = s.discover.related.as_ref().is_some_and(|(v, _)| *v == vid);
        (s.discover.section == "home" && !have).then_some(vid)
    });
    if let Some(vid) = vid {
        core.send(Command::LoadRelated(vid));
    }
}

/// The playing track changed: refresh the related shelves if Home is open.
pub fn track_changed(ui: &AppWindow) {
    if !ui.global::<Discover>().get_active() {
        return;
    }
    if let Some(core) = STATE.with(|s| s.borrow().core.clone()) {
        want_related(&core);
    }
}

/// Build the models from `discover.last`; Home gets the Related shelves appended.
fn render(ui: &AppWindow) {
    let d = ui.global::<Discover>();
    let data = STATE.with(|s| s.borrow().discover.last.clone());
    let (mut rows, mut models, mut lists) = (Vec::new(), Vec::new(), Vec::new());
    for (row, m, items) in shelf_rows(&data.shelves) {
        rows.push(row);
        models.push(m);
        lists.push(items);
    }
    let moods: Vec<MoodChip> = data
        .moods
        .iter()
        .map(|m| MoodChip {
            title: m.title.clone().into(),
            group: m.group.clone().into(),
        })
        .collect();
    let model = Rc::new(VecModel::from(rows));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.discover.base_len = lists.len();
        s.discover.shelves = lists;
        s.discover.models = models;
        s.discover.shelves_model = Some(model.clone());
        s.discover.mood_params = data.moods.iter().map(|m| m.params.clone()).collect();
        s.discover_moods = data.moods.iter().map(|m| m.title.clone()).collect();
    });
    d.set_shelves(ModelRc::from(model));
    d.set_moods(ModelRc::new(VecModel::from(moods)));
    d.set_busy(false);
    if STATE.with(|s| s.borrow().discover.section == "home") {
        refresh_related();
    }
}

/// A cover arrived: patch the cards that use it, in place.
pub fn patch_thumb(url: &str, image: &slint::Image) {
    use slint::Model;
    let Some(url) = url.strip_suffix("#card") else {
        return; // a small list thumbnail
    };
    STATE.with(|s| {
        let s = s.borrow();
        for (items, model) in s.discover.shelves.iter().zip(&s.discover.models) {
            for (n, it) in items.iter().enumerate() {
                if it.thumbnail.as_deref() == Some(url)
                    && let Some(mut c) = model.row_data(n)
                {
                    (c.thumb, c.has_thumb) = (image.clone(), true);
                    model.set_row_data(n, c);
                }
            }
        }
    });
}
