use super::*;

#[test]
fn lrc_basic_and_multi_stamp() {
    let l = parse_lrc(
        "[ar:Someone]\n[00:12.50] first\n[01:02.00][00:30.25] twice\n\nnot a lyric line\n[00:99.00] bad",
    );
    assert_eq!(l.len(), 3);
    assert_eq!(
        l[0],
        Line {
            t: 12.5,
            text: "first".into()
        }
    );
    assert_eq!(l[1].t, 30.25);
    assert_eq!(l[2].t, 62.0);
    assert!(l.iter().all(|x| x.text != "bad"));
}

#[test]
fn lrc_instrumental_gap_keeps_empty_line() {
    let l = parse_lrc("[00:01.00] a\n[00:05.00]\n[00:09.00] b");
    assert_eq!(l[1].text, "");
}

#[test]
fn current_line_tracks_position() {
    let ly = Lyrics {
        synced: true,
        source: "t".into(),
        lines: parse_lrc("[00:10.00] a\n[00:20.00] b"),
    };
    assert_eq!(ly.current(5.0), None);
    assert_eq!(ly.current(10.0), Some(0));
    assert_eq!(ly.current(25.0), Some(1));
}

#[test]
fn lrclib_prefers_synced_then_plain() {
    let v =
        json!({"syncedLyrics": "[00:01.00] a\n[00:02.00] b\n[00:03.00] c", "plainLyrics": "hi"});
    assert!(from_lrclib(&v, Some(200)).unwrap().synced);
    let p = json!({"syncedLyrics": null, "plainLyrics": "one\ntwo\nthree"});
    let l = from_lrclib(&p, Some(200)).unwrap();
    assert!(!l.synced);
    assert_eq!(l.lines.len(), 3);
    assert!(from_lrclib(&json!({"plainLyrics": " "}), Some(200)).is_none());
    // placeholder entries on a long track are rejected
    assert!(from_lrclib(&json!({"syncedLyrics": "[00:00.00] probe"}), Some(200)).is_none());
}

#[test]
fn ytm_fixture_and_tab_lookup() {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string("tests/fixtures/ytm_lyrics.json").unwrap())
            .unwrap();
    let l = from_ytm(&v).unwrap();
    assert!(!l.synced && l.lines.len() >= 3);
    assert_eq!(l.source, "Source: Test Fixture");
    let next = json!({"contents":{"singleColumnMusicWatchNextResultsRenderer":{"tabbedRenderer":{"watchNextTabbedResultsRenderer":{"tabs":[
        {"tabRenderer":{"title":"Up next"}},
        {"tabRenderer":{"title":"Lyrics","endpoint":{"browseEndpoint":{"browseId":"MPLYtX"}}}}]}}}}});
    assert_eq!(lyrics_browse_id(&next).as_deref(), Some("MPLYtX"));
    assert!(lyrics_browse_id(&json!({})).is_none());
}
