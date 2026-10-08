//! The GUI against the real core runtime and a fake InnerTube server: the Library tree must show
//! the counts of what the account has.
use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");

const ARTISTS: &str = r#"{"contents":{"singleColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicShelfRenderer":{"contents":[{"musicResponsiveListItemRenderer":{"navigationEndpoint":{"browseEndpoint":{"browseId":"UCfake"}},"flexColumns":[{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"Fake Artist"}]}}},{"musicResponsiveListItemFlexColumnRenderer":{"text":{"runs":[{"text":"1M subscribers"}]}}}]}}]}}]}}}}]}}}"#;

pub(super) fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{name}.json")).unwrap()
}

/// How the fake service behaves.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Api {
    /// A signed-in account.
    Ok,
    /// Cookies the service no longer accepts: library pages come back empty (HTTP 200), the
    /// account menu has no signed-in header.
    SignedOut,
    /// Everything works but the artists endpoint is down.
    ArtistsDown,
}

/// What the fake server answers for a request body.
fn answer(api: Api, endpoint: &str, body: &str) -> Option<String> {
    if api == Api::SignedOut {
        return Some("{}".into());
    }
    if endpoint != "browse" {
        return None;
    }
    let b: serde_json::Value = serde_json::from_str(body).ok()?;
    if b.get("continuation").is_some() {
        return Some(fixture("playlist_page2"));
    }
    let browse = b.get("browseId")?.as_str()?;
    if browse.starts_with("VLPLtest") {
        return Some(fixture("playlist_page1")); // the tests' own playlists (one id per test)
    }
    Some(match browse {
        "FEmusic_liked_playlists" | "FEmusic_liked_albums" => fixture("library_playlists"),
        "FEmusic_library_corpus_artists" if api == Api::Ok => ARTISTS.to_string(),
        "VLLM" => fixture("playlist_page1"),
        _ => return None,
    })
}

/// A minimal HTTP server (one thread per connection); returns the InnerTube base URL.
pub(super) fn fake_api(api: Api) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for conn in listener.incoming().flatten() {
            std::thread::spawn(move || serve(conn, api));
        }
    });
    format!("http://127.0.0.1:{port}/youtubei/v1/")
}

fn serve(mut conn: std::net::TcpStream, api: Api) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let (head_end, len) = loop {
        let n = conn.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            return;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..p]).to_lowercase();
            let len = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            break (p + 4, len);
        }
    };
    while buf.len() < head_end + len {
        let n = conn.read(&mut chunk).unwrap_or(0);
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let path = head.split_whitespace().nth(1).unwrap_or("");
    let endpoint = path
        .trim_start_matches("/youtubei/v1/")
        .split('?')
        .next()
        .unwrap_or("");
    let body = String::from_utf8_lossy(&buf[head_end..]).to_string();
    let (status, text) = match answer(api, endpoint, &body) {
        Some(t) => ("200 OK", t),
        None => ("404 Not Found", "{}".to_string()),
    };
    let _ = write!(
        conn,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        text.len()
    );
}

/// "Liked songs (12)" → 12
fn count_of(label: &str) -> Option<usize> {
    let (_, rest) = label.rsplit_once('(')?;
    rest.trim_end_matches(')')
        .trim_end_matches('+')
        .parse()
        .ok()
}

/// Starts the GUI wiring on a core talking to `api` (None = the real YouTube Music), expands the
/// four library sections like a user would and returns their labels once `done` is satisfied
/// (or after 20 s) together with the last status-bar message.
fn library_run(api: Option<String>, done: impl Fn(&[String]) -> bool) -> (Vec<String>, String) {
    use slint::Model;
    let (core, events) = crate::core::runtime::tests::start_with_api(api);
    let t = app();
    crate::gui::state::STATE.with(|s| s.borrow_mut().core = Some(core.clone()));
    crate::gui::panels::init_tree();
    crate::gui::panels::wire(&t.ui, &core);
    crate::gui::panels::refresh_tree(&t.ui);
    t.ui.set_sidebar_tab(1);
    // expand Artists, Albums, Playlists, Liked (bottom-up so the row numbers stay valid)
    for row in [3, 2, 1, 0] {
        t.ui.global::<crate::Panels>().invoke_tree_clicked(row);
    }
    let sections = |ui: &AppWindow| -> Vec<String> {
        ui.global::<crate::Panels>()
            .get_tree()
            .iter()
            .filter(|r| r.kind == "section")
            .map(|r| r.label.to_string())
            .collect()
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let last = loop {
        while let Ok(ev) = events.try_recv() {
            if std::env::var_os("DBG_EV").is_some() {
                eprintln!(
                    "EV {}",
                    format!("{ev:?}").chars().take(160).collect::<String>()
                );
            }
            crate::gui::handle_event(&t.ui, ev);
        }
        let s = sections(&t.ui);
        if done(&s) || std::time::Instant::now() > deadline {
            break s;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    core.send(crate::core::msg::Command::Quit);
    let message = t.ui.get_status_message().to_string();
    (last, message)
}

/// Every expanded section has finished (loaded or failed).
fn none_loading(_: &[String]) -> bool {
    crate::gui::state::STATE.with(|st| st.borrow().tree.iter().take(4).all(|t| !t.loading))
}

fn all_counted(s: &[String]) -> bool {
    s.iter().take(4).all(|l| count_of(l).is_some_and(|n| n > 0))
}

pub(super) fn write_fake_cookies() {
    let root = crate::config::use_test_dirs();
    std::fs::create_dir_all(root.join("data")).unwrap();
    std::fs::write(
        crate::config::cookies_path(),
        ".youtube.com\tTRUE\t/\tTRUE\t0\tSAPISID\tfake\n.youtube.com\tTRUE\t/\tTRUE\t0\tSID\tfake\n",
    )
    .unwrap();
}

/// Signed-in account: the four library sections show their counts after they are expanded.
#[test]
fn library_tree_shows_counts_from_the_api() {
    write_fake_cookies();
    let (s, _) = library_run(Some(fake_api(Api::Ok)), all_counted);
    assert!(all_counted(&s), "library counts missing: {s:?}");
}

/// Expired session: the service answers 200 with nothing. The tree must say so instead of showing
/// "(0)" for every section (and must not keep the sections as "loaded").
#[test]
fn expired_session_is_reported_not_shown_as_zero() {
    write_fake_cookies();
    let (s, message) = library_run(Some(fake_api(Api::SignedOut)), none_loading);
    assert!(
        s.iter().take(4).all(|l| !l.contains("(0")),
        "sections show an empty library: {s:?}"
    );
    assert!(
        message.contains("expired") || message.contains("sign"),
        "message: {message:?}"
    );
    // a failed section can be tried again
    crate::gui::state::STATE.with(|st| {
        assert!(
            st.borrow()
                .tree
                .iter()
                .take(4)
                .all(|t| t.items.is_none() && !t.loading)
        );
    });
}

/// One endpoint failing must not take the other sections down with it.
#[test]
fn one_failing_section_leaves_the_others_alone() {
    write_fake_cookies();
    let (s, _) = library_run(Some(fake_api(Api::ArtistsDown)), none_loading);
    assert!(
        s.iter().take(3).all(|l| count_of(l).is_some_and(|n| n > 0)),
        "other sections lost: {s:?}"
    );
    assert!(!s[3].contains("(0"), "{s:?}");
}

/// Against the real service with the real cookie file:
/// `CLOUDBERRY_TEST_REAL_AUTH=1 cargo test real_library -- --ignored --nocapture`
#[test]
#[ignore]
fn real_library_counts() {
    crate::config::use_test_dirs(); // scratch cache; the real cookies come from the env switch
    let (s, _) = library_run(None, all_counted);
    eprintln!("sections: {s:?}");
    assert!(all_counted(&s), "{s:?}");
}
