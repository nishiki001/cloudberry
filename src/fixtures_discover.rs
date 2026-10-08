//! Screenshot fixtures for the Discover page: `--view discover --fixture discover-home|discover-charts|discover-moods`.
use crate::AppWindow;
use crate::Discover;
use crate::core::api::discover_parse::{DiscoverData, Mood, Shelf, ShelfKind};
use crate::core::model::{Artist, Item, Kind};
use crate::fixtures::cover;
use crate::gui::state::STATE;
use slint::ComponentHandle;

fn item(n: usize, kind: Kind, title: &str, sub: &str) -> Item {
    Item {
        kind,
        title: title.into(),
        video_id: matches!(kind, Kind::Song | Kind::Video).then(|| format!("fx{n}")),
        browse_id: (!matches!(kind, Kind::Song | Kind::Video)).then(|| format!("fxb{n}")),
        artists: vec![Artist {
            name: sub.into(),
            id: None,
        }],
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail: Some(format!("fixture://{n}")),
        subtitle: sub.into(),
    }
}

fn shelf(title: &str, kind: Kind, names: &[&str], base: usize, songs: bool) -> Shelf {
    Shelf {
        title: title.into(),
        kind: if songs {
            ShelfKind::Songs
        } else {
            ShelfKind::Cards
        },
        items: names
            .iter()
            .enumerate()
            .map(|(i, n)| item(base + i, kind, n, "Invented Artist"))
            .collect(),
    }
}

pub fn apply(ui: &AppWindow, name: &str) {
    let section = name.strip_prefix("discover-").unwrap_or("home");
    let names = [
        "Neon Harbour",
        "Paper Lanterns",
        "Glass Orchard",
        "Midnight Tram",
        "Soft Static",
        "Violet Hours",
        "Salt and Signal",
        "Quiet Engines",
        "Northern Lights EP",
        "Slow Comet",
    ];
    let data = match section {
        "moods" => DiscoverData {
            shelves: vec![],
            moods: [
                "Pop",
                "Chill",
                "Focus",
                "Workout",
                "Party",
                "Sleep",
                "Romance",
                "Commute",
                "Feel good",
                "Sad",
                "Energy boost",
                "Rock",
                "Jazz",
                "Hip-Hop",
                "Classical",
                "Electronic",
                "Metal",
                "Country",
            ]
            .iter()
            .map(|t| Mood {
                title: (*t).into(),
                params: (*t).into(),
                group: "Moods".into(),
            })
            .collect(),
        },
        "charts" => DiscoverData {
            shelves: vec![
                shelf("Daily charts", Kind::Playlist, &names[..4], 100, false),
                shelf("Weekly charts", Kind::Playlist, &names[3..8], 120, false),
                shelf("Top artists", Kind::Artist, &names, 140, false),
            ],
            moods: vec![],
        },
        _ => DiscoverData {
            shelves: vec![
                shelf("Quick picks", Kind::Song, &names[..8], 0, true),
                shelf("Mixed for you", Kind::Playlist, &names, 20, false),
                shelf("Recommended albums", Kind::Album, &names[2..], 40, false),
            ],
            moods: vec![],
        },
    };
    let (key, section) = (format!("{section}-ZZ-"), section.to_string());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for n in 0..200 {
            s.thumbs.insert(
                format!("fixture://{n}#card"),
                cover(140, (n * 37 % 360) as f32),
            );
        }
        s.discover.key = key.clone();
    });
    crate::gui::discover::init(ui);
    let d = ui.global::<Discover>();
    d.set_opened(true);
    d.set_active(true);
    d.set_title(
        match section.as_str() {
            "charts" => "Charts",
            "moods" => "Moods & genres",
            _ => "Home",
        }
        .into(),
    );
    d.set_section(section.into());
    ui.set_sidebar_tab(4);
    crate::gui::discover::arrived(ui, key, data);
}
