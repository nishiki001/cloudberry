use super::*;

fn fixture(name: &str) -> Vec<Item> {
    let s = std::fs::read_to_string(format!("tests/fixtures/{name}.json")).unwrap();
    parse_search(&serde_json::from_str(&s).unwrap())
}

#[test]
fn songs() {
    let v = fixture("search_songs");
    assert!(!v.is_empty());
    let s = &v[0];
    assert_eq!(s.kind, Kind::Song);
    assert!(s.video_id.is_some() && !s.artists.is_empty());
    assert!(s.duration_secs.is_some());
    assert!(
        s.thumbnail
            .as_deref()
            .unwrap()
            .ends_with("=w544-h544-l90-rj")
    );
}

#[test]
fn videos() {
    let v = fixture("search_videos");
    assert_eq!(v[0].kind, Kind::Video);
    assert!(v[0].video_id.is_some());
    assert_eq!(v[0].artists[0].name, "Proximity");
}

#[test]
fn albums_artists_playlists() {
    assert_eq!(fixture("search_albums")[0].kind, Kind::Album);
    let a = &fixture("search_artists")[0];
    assert_eq!(a.kind, Kind::Artist);
    assert_eq!(a.title, "Daft Punk");
    let p = &fixture("search_playlists")[0];
    assert_eq!(p.kind, Kind::Playlist);
    assert!(p.browse_id.as_deref().unwrap().starts_with("PL"));
}

#[test]
fn cjk_and_cyrillic() {
    assert!(fixture("search_cjk").iter().any(|i| {
        i.title
            .chars()
            .any(|c| ('\u{3040}'..='\u{30ff}').contains(&c))
    }));
    assert!(fixture("search_cyrillic").iter().any(|i| {
        i.title
            .chars()
            .any(|c| ('\u{400}'..='\u{4ff}').contains(&c))
    }));
}

#[test]
fn garbage_does_not_panic() {
    assert!(parse_search(&serde_json::json!({"contents": 5})).is_empty());
    assert!(parse_list_item(&serde_json::json!([1, 2])).is_none());
}

#[test]
fn upscale_rules() {
    assert_eq!(
        upscale("https://lh3.googleusercontent.com/x=w60-h60-l90-rj"),
        "https://lh3.googleusercontent.com/x=w544-h544-l90-rj"
    );
    assert_eq!(
        upscale("https://i.ytimg.com/vi/a/hq.jpg"),
        "https://i.ytimg.com/vi/a/hq.jpg"
    );
}

#[test]
fn search_pages_and_continuations() {
    // first page: songs fixture; give its shelf a continuation token like the live response
    let mut first: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("tests/fixtures/search_songs.json").unwrap())
            .unwrap();
    let n_first = parse_search_page(&first).items.len();
    assert!(n_first > 0);
    let sections = first["contents"]["tabbedSearchResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
        ["sectionListRenderer"]["contents"]
        .as_array_mut()
        .unwrap();
    let shelf = sections
        .iter_mut()
        .find_map(|s| s.get_mut("musicShelfRenderer"))
        .unwrap();
    shelf["continuations"] =
        serde_json::json!([{"nextContinuationData": {"continuation": "TOK2"}}]);
    let items = shelf["contents"].clone();
    assert_eq!(
        parse_search_page(&first).continuation.as_deref(),
        Some("TOK2")
    );
    // continuation response: one shelf, and the end of the results has no token
    let more = serde_json::json!({"continuationContents": {"musicShelfContinuation": {"contents": items, "continuations": [{"nextContinuationData": {"continuation": "TOK3"}}]}}});
    let p = parse_search_page(&more);
    assert_eq!(p.items.len(), n_first);
    assert_eq!(p.continuation.as_deref(), Some("TOK3"));
    let last =
        serde_json::json!({"continuationContents": {"musicShelfContinuation": {"contents": []}}});
    assert_eq!(parse_search_page(&last), SearchPage::default());
    assert_eq!(
        parse_search_page(&serde_json::json!({})),
        SearchPage::default()
    );
}
