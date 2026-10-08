//! Fake data for `--screenshot` views. Everything is generated: no copyrighted art, no network.
use crate::art;
use crate::{AppWindow, Panels, TrackData};
use slint::{ComponentHandle, Image, ModelRc, SharedString, VecModel};

pub fn cover(size: u32, hue: f32) -> Image {
    let mut img = art::fixture_cover(size, hue);
    art::circle_mask(&mut img);
    art::to_slint(&img)
}

// (title, artist, album, year, length)
pub(crate) const TRACKS: [(&str, &str, &str, &str, &str); 12] = [
    (
        "Instant Crush (feat. Julian Casablancas)",
        "Daft Punk, Julian Casablancas",
        "Random Access Memories",
        "2013",
        "5:38",
    ),
    (
        "「シキ」 パッパパラダイス",
        "宇多田ヒカル",
        "Fixture Album",
        "2008",
        "4:02",
    ),
    (
        "Кукла колдуна",
        "Король и Шут",
        "Акустический альбом",
        "1999",
        "3:24",
    ),
    (
        "Get Lucky (feat. Pharrell Williams)",
        "Daft Punk",
        "Random Access Memories",
        "2013",
        "6:09",
    ),
    (
        "A Very Long Track Title That Needs To Be Elided Somewhere Along The Row",
        "Some Artist With A Long Name",
        "An Album With A Rather Long Title Too",
        "2020",
        "12:34",
    ),
    (
        "Harder, Better, Faster, Stronger",
        "Daft Punk",
        "Discovery",
        "2001",
        "3:45",
    ),
    (
        "Лесник",
        "Король и Шут",
        "Будь как дома, путник",
        "2001",
        "3:14",
    ),
    ("Windowlicker", "Aphex Twin", "Windowlicker", "1999", "6:08"),
    ("One More Time", "Daft Punk", "Discovery", "2001", "5:20"),
    ("Digital Love", "Daft Punk", "Discovery", "2001", "4:58"),
    ("Veridis Quo", "Daft Punk", "Discovery", "2001", "5:45"),
    (
        "Something About Us",
        "Daft Punk",
        "Discovery",
        "2001",
        "3:51",
    ),
];

pub(crate) fn row(t: &(&str, &str, &str, &str, &str), playing: bool, liked: bool) -> TrackData {
    TrackData {
        title: t.0.into(),
        artist: t.1.into(),
        album: t.2.into(),
        year: t.3.into(),
        time: t.4.into(),
        playing,
        liked,
        selected: false,
        kind: "song".into(),
        thumb: Default::default(),
        has_thumb: false,
    }
}

pub(crate) fn model(rows: Vec<TrackData>) -> ModelRc<TrackData> {
    ModelRc::new(VecModel::from(rows))
}

/// Populate the window with fake data (no network). `view` picks the sidebar tab / dialog.
pub fn apply(ui: &AppWindow, view: &str, name: &str) {
    let queue: Vec<TrackData> = TRACKS
        .iter()
        .enumerate()
        .map(|(i, t)| row(t, i == 1, i == 1 || i == 5))
        .collect();
    let mut queue = queue;
    if name == "big" {
        queue = (0..5000)
            .map(|n| {
                let t = &TRACKS[n % TRACKS.len()];
                TrackData {
                    title: format!("{} #{n}", t.0).into(),
                    artist: t.1.into(),
                    album: t.2.into(),
                    year: t.3.into(),
                    time: t.4.into(),
                    playing: n == 2,
                    liked: n % 7 == 0,
                    selected: n == 3 || n == 4,
                    kind: "song".into(),
                    thumb: Default::default(),
                    has_thumb: false,
                }
            })
            .collect();
    } else {
        queue
            .iter_mut()
            .enumerate()
            .for_each(|(i, r)| r.selected = i == 3 || i == 4);
    }
    if name == "longtext" {
        // very long invented Japanese, Cyrillic and Latin texts
        let long = [
            (
                "これは非常に長い日本語のタイトルです、画面の端まで続いてしまうかもしれません",
                "架空のアーティスト名がとても長い場合のテスト用表示",
                "とても長い日本語のアルバム名、右端で切れてはいけません、省略記号で終わるはずです",
            ),
            (
                "Очень длинное название песни на русском языке, которое не должно вылезать за край окна",
                "Исполнитель с очень длинным именем, которое не помещается",
                "Очень длинное название альбома на русском языке, которое тоже должно обрезаться многоточием",
            ),
            (
                "An extremely long Latin title that goes on and on and must be elided at the column edge",
                "An artist whose name is far too long to fit in any reasonable column",
                "A very long album name that would run under the window edge if it were not elided",
            ),
        ];
        for (r, (t, a, b)) in queue.iter_mut().zip(long) {
            r.title = t.into();
            r.artist = a.into();
            r.album = b.into();
        }
    }
    crate::gui::install_fixture_rows(ui, queue);
    let mut tabs = vec![
        SharedString::from("Queue"),
        "Liked songs".into(),
        "Radio: Get Lucky".into(),
    ];
    if name == "longtext" {
        tabs.push("Очень длинное название вкладки плейлиста на русском языке".into());
        tabs.push("これは非常に長い日本語のプレイリスト名です、タブに収まりません".into());
        tabs.push("Another playlist tab with a rather long Latin name".into());
    }
    ui.set_playlist_tabs(ModelRc::new(VecModel::from(tabs)));
    ui.set_playlist_tab(0);
    crate::gui::table_fixture_columns(ui, name != "big");
    if name == "big" {
        ui.set_status_summary("5000 tracks · 5:23:41".into());
    }
    ui.global::<Panels>().set_results(model(
        TRACKS
            .iter()
            .take(9)
            .map(|t| row(t, false, false))
            .collect(),
    ));
    ui.set_cover(cover(320, 200.0));
    let mut flat = art::fixture_cover(320, 200.0);
    flat.pixels_mut().for_each(|p| p.0[3] = 255);
    ui.set_cover_flat(art::to_slint(&flat));
    ui.set_has_cover(name != "nocover");
    ui.set_playing(true);
    ui.set_has_track(true);
    ui.set_track_title(TRACKS[1].0.into());
    ui.set_artist_name(TRACKS[1].1.into());
    ui.set_album_name(TRACKS[1].2.into());
    ui.set_elapsed("1:48".into());
    ui.set_total("4:02".into());
    ui.set_position(0.45);
    ui.set_buffered(0.7);
    ui.set_liked(true);
    ui.set_shuffle(true);
    ui.set_repeat_mode("all".into());
    ui.set_status_summary("12 tracks · 57:47".into());
    ui.set_status_format("AAC 129 kbps".into());
    ui.set_status_account("signed in".into());
    ui.set_status_message(
        if name == "longtext" {
            "A very long status message that has to be elided before it reaches the format and account texts on the right"
        } else {
            "added 12 tracks"
        }
        .into(),
    );
    synthetic_analyzer(ui, name);
    let lyrics = [
        "Invented first line of a made-up song",
        "A second line, only for screenshots",
        "三行目 — 架空の歌詞です",
        "Четвёртая строка — выдуманный текст",
        "The fifth line is the one that glows",
        "",
        "Another invented line follows",
        "And one more to fill the view",
        "Penultimate made-up line",
        "Final invented line",
    ];
    let lines: Vec<SharedString> = lyrics.iter().map(|l| (*l).into()).collect();
    ui.global::<Panels>()
        .set_lyrics(ModelRc::new(VecModel::from(lines)));
    ui.global::<Panels>().set_lyrics_synced(true);
    ui.global::<Panels>().set_lyrics_source("LRCLIB".into());
    ui.global::<Panels>().set_lyrics_current(4);
    if name == "longtext" {
        ui.set_track_title("これは非常に長い日本語のタイトルです、画面の端まで続いてしまうかもしれません、本当に長いタイトル".into());
        ui.set_artist_name(
            "Исполнитель с очень длинным именем, которое не помещается в окно мини-плеера вообще"
                .into(),
        );
        ui.set_album_name("Очень длинное название альбома на русском языке, которое тоже должно обрезаться многоточием".into());
    }
    crate::fixtures_views::view(ui, view, name);
}

use crate::fixtures_extra::synthetic_analyzer;
