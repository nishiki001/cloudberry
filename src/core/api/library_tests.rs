use super::*;
use serde_json::json;

fn load(name: &str) -> Value {
    serde_json::from_str(&std::fs::read_to_string(format!("tests/fixtures/{name}.json")).unwrap())
        .unwrap()
}

#[test]
fn playlist_first_page_and_token() {
    let (items, token) = parse_playlist_page(&load("playlist_page1"));
    assert_eq!(items.len(), 3);
    assert_eq!(token.as_deref(), Some("TOKEN_PAGE_2"));
    let i = &items[0];
    assert_eq!(i.title, "The Game of Love");
    assert_eq!(i.artists[0].name, "Daft Punk");
    assert_eq!(i.album.as_deref(), Some("Random Access Memories"));
    assert_eq!(i.duration_secs, Some(323));
    assert!(i.video_id.is_some());
}

#[test]
fn continuation_page_has_no_further_token() {
    let (items, token) = parse_playlist_page(&load("playlist_page2"));
    assert_eq!(items.len(), 2);
    assert!(token.is_none());
}

#[test]
fn library_grid_skips_new_playlist() {
    let p = parse_library_playlists(&load("library_playlists"));
    assert_eq!(p.len(), 2);
    assert_eq!(p[0].browse_id.as_deref(), Some("PLfixture0001"));
    assert_eq!(p[0].subtitle, "Playlist • 42 songs");
    assert!(p[0].thumbnail.as_deref().unwrap().contains("w544"));
    assert!(p[1].title.contains("Король"));
}

#[test]
fn albums_use_the_same_grid_with_album_kind() {
    let v = json!({"contents":{"singleColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"gridRenderer":{"items":[
        {"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"Test Album"}]},"subtitle":{"runs":[{"text":"Album"},{"text":" • "},{"text":"Someone"}]},"navigationEndpoint":{"browseEndpoint":{"browseId":"MPREbTEST"}}}},
        {"musicTwoRowItemRenderer":{"title":{"runs":[{"text":"A playlist"}]},"navigationEndpoint":{"browseEndpoint":{"browseId":"VLPLTEST"}}}}
    ]}}]}}}}]}}});
    let a = parse_library_playlists(&v);
    assert_eq!(
        (a[0].kind, a[0].browse_id.as_deref()),
        (Kind::Album, Some("MPREbTEST"))
    );
    assert_eq!(
        (a[1].kind, a[1].browse_id.as_deref()),
        (Kind::Playlist, Some("PLTEST"))
    );
}

#[test]
fn library_artists_shelf() {
    let v = json!({"contents":{"singleColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"contents":[
        {"musicResponsiveListItemRenderer":{"navigationEndpoint":{"browseEndpoint":{"browseId":"UCtest"}},
          "flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"宇多田ヒカル"}]}}},{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"12 songs"}]}}}]}},
        {"musicResponsiveListItemRenderer":{"flexColumns":[]}}
    ]}}]}}}}]}}});
    let a = parse_library_artists(&v);
    assert_eq!(a.len(), 1);
    assert_eq!(
        (a[0].kind, a[0].title.as_str(), a[0].subtitle.as_str()),
        (Kind::Artist, "宇多田ヒカル", "12 songs")
    );
}

#[test]
fn empty_and_garbage() {
    assert!(parse_playlist_page(&json!({})).0.is_empty());
    assert!(parse_history(&json!({"contents": 1})).is_empty());
}
