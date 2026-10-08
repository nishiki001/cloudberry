use super::discover_parse::*;
use crate::core::model::Kind;
use serde_json::Value;

fn load(name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!("tests/fixtures/{name}.json")).expect("fixture"),
    )
    .expect("json")
}

#[test]
fn home_shelves_cards_songs_and_skipping() {
    let page = parse_page(&load("discover_home"));
    let titles: Vec<&str> = page.shelves.iter().map(|s| s.title.as_str()).collect();
    // the description shelf and the item-less shelf are skipped
    assert_eq!(
        titles,
        ["Quick picks", "Mixed for you", "Recommended albums"]
    );
    assert_eq!(page.continuation.as_deref(), Some("TOKEN_PAGE_2"));

    let quick = &page.shelves[0];
    assert_eq!(quick.kind, ShelfKind::Songs);
    assert_eq!(quick.items.len(), 3);
    assert_eq!(quick.items[0].video_id.as_deref(), Some("vidAAAAAAA1"));
    assert_eq!(quick.items[0].duration.as_deref(), Some("3:21"));

    let mixed = &page.shelves[1];
    assert_eq!(mixed.kind, ShelfKind::Cards);
    // a mix, a playlist (the "VL" prefix is dropped) and a radio start
    assert_eq!(mixed.items[0].kind, Kind::Playlist);
    assert_eq!(mixed.items[0].browse_id.as_deref(), Some("RDTMAKfakemix1"));
    assert_eq!(
        mixed.items[1].browse_id.as_deref(),
        Some("PLfakeplaylist0001")
    );
    assert_eq!(mixed.items[1].artists[0].name, "Fake Curator");
    assert_eq!(mixed.items[2].video_id.as_deref(), Some("vidRADIO0001"));
    assert!(
        mixed.items[1]
            .thumbnail
            .as_deref()
            .unwrap()
            .contains("w544")
    );

    let albums = &page.shelves[2];
    // the podcast is skipped; album and artist keep their kind
    assert_eq!(albums.items.len(), 2);
    assert_eq!(albums.items[0].kind, Kind::Album);
    assert_eq!(
        albums.items[0].browse_id.as_deref(),
        Some("MPREfake000000001")
    );
    assert_eq!(albums.items[1].kind, Kind::Artist);
}

#[test]
fn continuation_pages_chain() {
    let more = parse_page(&load("discover_home_more"));
    assert_eq!(more.shelves.len(), 1);
    assert_eq!(more.shelves[0].title, "Listen again");
    assert_eq!(more.continuation.as_deref(), Some("TOKEN_PAGE_3"));
}

#[test]
fn new_releases_grid_has_a_title_and_albums() {
    let page = parse_page(&load("discover_new"));
    assert_eq!(page.shelves.len(), 1);
    assert_eq!(page.shelves[0].title, ""); // the client names it
    assert_eq!(page.shelves[0].items.len(), 5);
    assert!(page.shelves[0].items.iter().all(|i| i.kind == Kind::Album));
}

#[test]
fn charts_have_playlists_and_top_artists() {
    let page = parse_page(&load("discover_charts"));
    // the empty first shelf (country selector) is dropped
    assert_eq!(page.shelves.len(), 2);
    assert_eq!(page.shelves[0].title, "Daily charts");
    assert_eq!(page.shelves[0].items[0].kind, Kind::Playlist);
    let artists = &page.shelves[1];
    assert_eq!(artists.title, "Top artists");
    assert_eq!(
        artists.kind,
        ShelfKind::Cards,
        "artists are cards, not a song list"
    );
    assert_eq!(artists.items[0].kind, Kind::Artist);
}

#[test]
fn moods_and_a_mood_category() {
    let moods = parse_moods(&load("discover_moods"));
    // the button without a destination is skipped
    let names: Vec<(&str, &str, &str)> = moods
        .iter()
        .map(|m| (m.group.as_str(), m.title.as_str(), m.params.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("For you", "Pop", "PARAMS_POP"),
            ("For you", "Chill", "PARAMS_CHILL"),
            ("Genres", "Jazz", "PARAMS_JAZZ")
        ]
    );
    let page = parse_page(&load("discover_mood"));
    assert_eq!(page.shelves.len(), 2);
    assert_eq!(page.shelves[0].items.len(), 2);
    assert_eq!(page.shelves[1].kind, ShelfKind::Songs);
}

#[test]
fn garbage_never_panics() {
    for v in [
        Value::Null,
        serde_json::json!({}),
        serde_json::json!({"contents": 5}),
        serde_json::json!({"continuationContents": {"sectionListContinuation": {"contents": [1, "x", {"musicCarouselShelfRenderer": {"contents": [null, {"musicTwoRowItemRenderer": 3}]}}]}}}),
    ] {
        assert!(parse_page(&v).shelves.is_empty());
        assert!(parse_moods(&v).is_empty());
    }
}

#[test]
fn related_tab_and_its_page() {
    use super::discover_raw::related_browse_id;
    let next = serde_json::json!({"contents": {"singleColumnMusicWatchNextResultsRenderer": {"tabbedRenderer": {"watchNextTabbedResultsRenderer": {"tabs": [
        {"tabRenderer": {"title": "Up next"}},
        {"tabRenderer": {"title": "Lyrics", "endpoint": {"browseEndpoint": {"browseId": "MPLYtX"}}}},
        {"tabRenderer": {"title": "Related", "endpoint": {"browseEndpoint": {"browseId": "MPTRtX"}}}}]}}}}});
    assert_eq!(related_browse_id(&next).as_deref(), Some("MPTRtX"));
    assert!(related_browse_id(&serde_json::json!({})).is_none());
    // the related page has no singleColumn wrapper
    let page = load("discover_mood");
    let inner =
        page["contents"]["singleColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
            .clone();
    let related = serde_json::json!({"contents": inner});
    assert_eq!(parse_page(&related).shelves.len(), 2);
}
