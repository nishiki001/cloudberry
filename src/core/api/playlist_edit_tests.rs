use super::*;

#[test]
fn request_bodies() {
    let ids = vec!["a".to_string(), "b".to_string()];
    let b = add_body("PL1", &ids, false);
    assert_eq!(b["playlistId"], "PL1");
    assert_eq!(b["actions"][1]["action"], "ACTION_ADD_VIDEO");
    assert_eq!(b["actions"][1]["addedVideoId"], "b");
    assert!(b["actions"][0].get("dedupeOption").is_none());
    let b = add_body("PL1", &ids, true);
    assert_eq!(b["actions"][0]["dedupeOption"], "DEDUPE_OPTION_SKIP");
    let r = remove_body("PL1", &[("a".into(), "SET1".into())]);
    assert_eq!(r["actions"][0]["action"], "ACTION_REMOVE_VIDEO");
    assert_eq!(r["actions"][0]["setVideoId"], "SET1");
    assert_eq!(r["actions"][0]["removedVideoId"], "a");
    let c = create_body("Mix", &ids, "PRIVATE");
    assert_eq!(c["title"], "Mix");
    assert_eq!(c["privacyStatus"], "PRIVATE");
    assert_eq!(c["videoIds"][0], "a");
}

#[test]
fn edit_results_and_targets() {
    let resp = json!({"status": "STATUS_SUCCEEDED", "playlistEditResults": [
        {"playlistEditVideoAddedResultData": {"videoId": "a", "setVideoId": "S1"}},
        {"other": 1},
        {"playlistEditVideoAddedResultData": {"videoId": "b", "setVideoId": "S2"}}]});
    assert!(succeeded(&resp));
    assert_eq!(
        parse_added(&resp),
        [
            ("a".to_string(), "S1".to_string()),
            ("b".to_string(), "S2".to_string())
        ]
    );
    assert!(!succeeded(&json!({"status": "STATUS_FAILED"})));
    let dialog = json!({"contents": [{"addToPlaylistRenderer": {"playlists": [
        {"playlistAddToOptionRenderer": {"playlistId": "LM", "title": {"runs": [{"text": "Liked Music"}]}}},
        {"playlistAddToOptionRenderer": {"playlistId": "PL9", "title": {"runs": [{"text": " Mine "}]}}},
        {"playlistAddToOptionRenderer": {"title": {"runs": [{"text": "no id"}]}}}]}}]});
    let t = parse_targets(&dialog);
    assert_eq!(t.len(), 2);
    assert_eq!(
        t[1],
        AddTarget {
            id: "PL9".into(),
            title: "Mine".into(),
            thumbnail: None,
            detail: String::new(),
        }
    );
    assert!(parse_targets(&json!({})).is_empty());
    // YouTube's own dialog uses simpleText titles
    let plain = json!({"addToPlaylistRenderer": {"playlists": [{"playlistAddToOptionRenderer": {"playlistId": "PL1", "title": {"simpleText": "Plain"}}}]}});
    assert_eq!(parse_targets(&plain)[0].title, "Plain");
}

#[test]
fn library_fills_thumbnails_and_stands_in() {
    use crate::core::model::{Item, Kind};
    let lib = |id: &str, title: &str| Item {
        kind: Kind::Playlist,
        title: title.into(),
        browse_id: Some(id.into()),
        thumbnail: Some(format!("https://img/{id}")),
        subtitle: "Playlist • 12 songs".into(),
        video_id: None,
        artists: Vec::new(),
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
    };
    let targets = vec![AddTarget {
        id: "PL9".into(),
        title: "Mine".into(),
        thumbnail: None,
        detail: String::new(),
    }];
    let merged = merge_library(targets, vec![lib("PL9", "Mine"), lib("PL2", "Other")]);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].thumbnail.as_deref(), Some("https://img/PL9"));
    assert_eq!(merged[0].detail, "Playlist • 12 songs");
    // nothing usable from the add-to-playlist call: the library list stands in
    let fallback = merge_library(
        Vec::new(),
        vec![lib("PL9", "Mine"), lib("PL2", "Other"), lib("LM", "Liked")],
    );
    assert_eq!(fallback.len(), 2, "Liked Music cannot be added to");
    assert_eq!(fallback[1].id, "PL2");
}

#[test]
fn create_body_privacy() {
    assert_eq!(create_body("x", &[], "PUBLIC")["privacyStatus"], "PUBLIC");
    assert_eq!(
        create_body("x", &[], "UNLISTED")["privacyStatus"],
        "UNLISTED"
    );
    assert_eq!(create_body("x", &[], "weird")["privacyStatus"], "PRIVATE");
}
