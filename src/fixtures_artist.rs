//! Screenshot fixture for the artist page: `--view artist`.
use crate::AppWindow;
use crate::core::api::artist_info::ArtistInfo;
use crate::core::api::discover_parse::{Shelf, ShelfKind};
use crate::core::model::{Artist, Item, Kind};
use crate::fixtures::cover;
use crate::gui::state::STATE;
use image::{Rgba, RgbaImage};
use std::sync::Arc;

fn item(n: usize, kind: Kind, title: &str, sub: &str) -> Item {
    Item {
        kind,
        title: title.into(),
        video_id: matches!(kind, Kind::Song | Kind::Video).then(|| format!("ax{n}")),
        browse_id: (!matches!(kind, Kind::Song | Kind::Video)).then(|| format!("axb{n}")),
        artists: vec![Artist {
            name: "Invented Artist".into(),
            id: Some("UCfixtureartist000000000".into()),
        }],
        album: Some("Imaginary Album".into()),
        album_id: Some("MPREfixture".into()),
        duration: Some("3:41".into()),
        duration_secs: Some(221),
        thumbnail: Some(format!("fixture://{n}")),
        subtitle: sub.into(),
    }
}

pub fn apply(ui: &AppWindow, album: bool) {
    let names = [
        "Neon Harbour",
        "Paper Lanterns",
        "Glass Orchard",
        "Midnight Tram",
        "Soft Static",
        "Violet Hours",
        "Salt and Signal",
        "Quiet Engines",
        "Slow Comet",
        "Northern Lights",
    ];
    let shelf = |title: &str, kind: Kind, from: usize, sub: &str| Shelf {
        title: title.into(),
        kind: ShelfKind::Cards,
        items: names
            .iter()
            .enumerate()
            .map(|(i, n)| item(from + i, kind, n, sub))
            .collect(),
    };
    let info = ArtistInfo {
        id: "UCfixtureartist000000000".into(),
        name: "Invented Artist".into(),
        description: "An invented act that exists only in screenshots. Their made-up records mix glass-like synths with slow, hand-drawn drums, and every word of this description was written for the test fixture, not taken from anywhere. Click to read more.".into(),
        subscribers: Some("2.4M subscribers".into()),
        monthly: Some("18.2M monthly audience".into()),
        image: Some("fixture://hdr".into()),
        songs: names[..5]
            .iter()
            .enumerate()
            .map(|(i, n)| item(100 + i, Kind::Song, n, ""))
            .collect(),
        plays: ["1.2B plays", "851M plays", "398M plays", "297M plays", "88M plays"]
            .map(String::from)
            .to_vec(),
        songs_playlist: Some("OLAKfixture".into()),
        play: Some("RDAOfixture".into()),
        radio: Some("RDEMfixture".into()),
        shelves: vec![
            shelf("Albums", Kind::Album, 0, "Album • 2021"),
            shelf("Singles & EPs", Kind::Album, 20, "Single • 2023"),
            shelf("Videos", Kind::Video, 40, "1.2M views"),
            shelf("Fans might also like", Kind::Artist, 60, "1.1M subscribers"),
        ],
    };
    let header = Arc::new(RgbaImage::from_fn(540, 540, |x, y| {
        let (fx, fy) = (x as f32 / 540.0, y as f32 / 540.0);
        Rgba([
            (60.0 + 160.0 * fx) as u8,
            (80.0 + 100.0 * (1.0 - fy)) as u8,
            (200.0 - 90.0 * fx) as u8,
            255,
        ])
    }));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for n in 0..200 {
            s.thumbs
                .insert(format!("fixture://{n}"), cover(140, (n * 37 % 360) as f32));
            s.thumbs.insert(
                format!("fixture://{n}#card"),
                cover(140, (n * 37 % 360) as f32),
            );
        }
    });
    crate::gui::discover::init(ui);
    if album {
        let page = crate::core::api::browse::AlbumPage {
            title: "Imaginary Album".into(),
            artist: "Invented Artist".into(),
            artist_id: Some("UCfixtureartist000000000".into()),
            year: Some("2021".into()),
            thumbnail: Some("fixture://hdr".into()),
            tracks: info
                .songs
                .iter()
                .cloned()
                .chain(info.shelves[2].items.iter().take(5).cloned().map(|mut i| {
                    i.kind = Kind::Song;
                    i
                }))
                .collect(),
        };
        crate::gui::artist::fixture_album(ui, page, header);
    } else {
        crate::gui::artist::fixture(ui, info, header);
    }
}
