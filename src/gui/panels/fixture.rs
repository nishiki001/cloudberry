//! Screenshot fixture for the library tree.
use super::super::state::*;
use super::tree::{install_models, refresh_tree};
use crate::AppWindow;
use crate::core::model::{Item, Kind};
use crate::core::msg::LibraryKind;

/// Screenshot fixture: Liked and Playlists expanded, with generated thumbnails.
pub fn fixture_tree(ui: &AppWindow) {
    use crate::core::model::Artist;
    let mk = |title: &str, sub: &str, kind: Kind, n: usize| Item {
        kind,
        title: title.into(),
        video_id: matches!(kind, Kind::Song).then(|| format!("v{n}")),
        browse_id: (!matches!(kind, Kind::Song)).then(|| format!("b{n}")),
        artists: if matches!(kind, Kind::Song) {
            vec![Artist {
                name: sub.into(),
                id: None,
            }]
        } else {
            vec![]
        },
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail: Some(format!("fixture://{n}")),
        subtitle: if matches!(kind, Kind::Song) {
            String::new()
        } else {
            sub.into()
        },
    };
    let liked = vec![
        mk("Instant Crush", "Daft Punk", Kind::Song, 1),
        mk("「シキ」 パッパパラダイス", "宇多田ヒカル", Kind::Song, 2),
        mk("Кукла колдуна", "Король и Шут", Kind::Song, 3),
    ];
    let lists = vec![
        mk(
            "Road trip 「ドライブ」",
            "Playlist • 42 songs",
            Kind::Playlist,
            4,
        ),
        mk("Focus", "Playlist • 120 songs", Kind::Playlist, 5),
    ];
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for (kind, items) in [(LibraryKind::Liked, liked), (LibraryKind::Playlists, lists)] {
            if let Some(sec) = s.tree.iter_mut().find(|t| t.kind == kind) {
                sec.items = Some(items);
                sec.expanded = true;
            }
        }
        for n in 1..=5usize {
            s.thumbs.insert(
                format!("fixture://{n}"),
                crate::fixtures::cover(64, n as f32 * 61.0),
            );
        }
    });
    install_models(ui);
    refresh_tree(ui);
}
