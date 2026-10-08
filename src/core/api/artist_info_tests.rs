use super::*;
use crate::core::model::Kind;

fn load() -> Value {
    serde_json::from_str(&std::fs::read_to_string("tests/fixtures/artist_info.json").unwrap())
        .unwrap()
}

#[test]
fn header_songs_and_shelves() {
    let a = parse_artist_info(&load(), "UCfakeartist0000000000");
    assert_eq!(a.name, "Invented Artist");
    assert!(a.description.starts_with("An invented act"));
    assert_eq!(a.subscribers.as_deref(), Some("250K subscribers"));
    assert_eq!(a.monthly.as_deref(), Some("1.5M monthly audience"));
    assert!(a.image.as_deref().unwrap().contains("w1080"));
    assert_eq!(a.play.as_deref(), Some("RDAOfakeshuffle"));
    assert_eq!(a.radio.as_deref(), Some("RDEMfakeradio"));
    // top songs: the unknown row is skipped and plays stay aligned with the songs
    assert_eq!(a.songs.len(), 3);
    assert_eq!(a.songs[2].video_id.as_deref(), Some("vidTOP00003"));
    assert_eq!(a.plays, ["12M plays", "3.4M plays", "870K plays"]);
    assert_eq!(a.songs_playlist.as_deref(), Some("OLAKfakealltop"));
    let titles: Vec<&str> = a.shelves.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Albums", "Singles & EPs", "Videos", "Fans might also like"]
    );
    assert_eq!(a.shelves[0].items[0].kind, Kind::Album);
    assert_eq!(a.shelves[3].items[0].kind, Kind::Artist);
    assert_eq!(
        a.shelves[3].items[1].browse_id.as_deref(),
        Some("UCrelated2")
    );
}

#[test]
fn missing_parts_never_panic() {
    for v in [
        Value::Null,
        serde_json::json!({}),
        serde_json::json!({"header": 3, "contents": []}),
    ] {
        let a = parse_artist_info(&v, "UCx");
        assert_eq!(a.id, "UCx");
        assert!(a.songs.is_empty() && a.shelves.is_empty());
    }
}
