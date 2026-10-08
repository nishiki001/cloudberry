use super::*;

pub(super) fn song(id: &str) -> crate::core::model::Item {
    crate::core::model::Item {
        kind: crate::core::model::Kind::Song,
        title: format!("Song {id}"),
        video_id: Some(id.into()),
        browse_id: None,
        artists: Vec::new(),
        album: None,
        album_id: None,
        duration: None,
        duration_secs: None,
        thumbnail: None,
        subtitle: String::new(),
    }
}

fn target(id: &str, title: &str) -> crate::core::api::playlist_edit::AddTarget {
    crate::core::api::playlist_edit::AddTarget {
        id: id.into(),
        title: title.into(),
        thumbnail: None,
        detail: "Playlist • 3 songs".into(),
    }
}

fn drain(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::core::msg::Command>,
) -> Vec<crate::core::msg::Command> {
    let mut v = Vec::new();
    while let Ok(c) = rx.try_recv() {
        v.push(c);
    }
    v
}

/// "Add to playlist" → "New playlist…": name, privacy, Create sends the command with the songs.
#[test]
fn add_to_new_playlist_dialog() {
    use crate::core::msg::Command;
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    drain(&mut rx);
    crate::gui::playlist_add::choose(&t.ui, &core, "__new__", vec![song("a"), song("b")]);
    t.click_label("Playlist name");
    t.type_text("Road trip");
    t.click_label("Public");
    t.click_label("Create");
    let sent = drain(&mut rx);
    match sent.as_slice() {
        [
            Command::CreatePlaylist {
                title,
                privacy,
                items,
            },
        ] => {
            assert_eq!(
                (title.as_str(), privacy.as_str(), items.len()),
                ("Road trip", "PUBLIC", 2)
            );
        }
        other => panic!("unexpected commands: {other:?}"),
    }
    assert!(!t.ui.global::<crate::AddPlaylist>().get_new_visible());
}

/// "Existing playlist…": rows with counts, filter box, double-click and the Add button add.
#[test]
fn add_to_existing_playlist_dialog() {
    use crate::core::msg::Command;
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    drain(&mut rx);
    crate::gui::playlist_add::targets_arrived(
        &t.ui,
        vec![
            target("PL1", "Road trip"),
            target("PL2", "Chill"),
            target("PL3", "Workout"),
        ],
    );
    crate::gui::playlist_add::choose(&t.ui, &core, "__existing__", vec![song("a")]);
    assert_eq!(
        t.rows_visible(&["Road trip", "Chill", "Workout"]),
        [true, true, true]
    );
    // the filter narrows the list
    t.click_label("Filter playlists");
    t.type_text("chi");
    assert_eq!(
        t.rows_visible(&["Road trip", "Chill", "Workout"]),
        [false, true, false]
    );
    // "Add" needs a selection; select, then Add
    t.click_label("Chill");
    t.click_label("Add");
    let sent = drain(&mut rx);
    match sent.as_slice() {
        [
            Command::AddToPlaylist {
                playlist,
                title,
                items,
                force,
            },
        ] => {
            assert_eq!(
                (playlist.as_str(), title.as_str(), items.len(), *force),
                ("PL2", "Chill", 1, false)
            );
        }
        other => panic!("unexpected commands: {other:?}"),
    }
    assert!(!t.ui.global::<crate::AddPlaylist>().get_existing_visible());

    // double-click adds right away
    crate::gui::playlist_add::choose(&t.ui, &core, "__existing__", vec![song("b")]);
    let (x, y) = t.row("Workout");
    t.click(x, y, PointerEventButton::Left);
    t.click(x, y, PointerEventButton::Left);
    let sent = drain(&mut rx);
    assert!(
        matches!(sent.as_slice(), [Command::AddToPlaylist { playlist, .. }] if playlist == "PL3"),
        "{sent:?}"
    );
}

/// Before the server list has arrived the dialog shows a spinner text and asks for the list.
#[test]
fn existing_playlist_dialog_loads_when_empty() {
    use crate::core::msg::Command;
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    drain(&mut rx);
    crate::gui::playlist_add::choose(&t.ui, &core, "__existing__", vec![song("a")]);
    assert!(t.ui.global::<crate::AddPlaylist>().get_loading());
    assert!(
        drain(&mut rx)
            .iter()
            .any(|c| matches!(c, Command::LoadAddTargets))
    );
    crate::gui::playlist_add::targets_arrived(&t.ui, vec![target("PL1", "Road trip")]);
    assert!(!t.ui.global::<crate::AddPlaylist>().get_loading());
    assert!(t.item("Road trip").is_some());
}

/// A name of only spaces keeps the dialog open and the songs pending.
#[test]
fn blank_playlist_name_keeps_the_dialog() {
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    drain(&mut rx);
    crate::gui::playlist_add::choose(&t.ui, &core, "__new__", vec![song("a")]);
    t.click_label("Playlist name");
    t.type_text("   ");
    t.click_label("Create");
    assert!(drain(&mut rx).is_empty());
    assert!(t.ui.global::<crate::AddPlaylist>().get_new_visible());
}

/// A failed load ends in "No playlists found", not in an endless spinner.
#[test]
fn empty_answer_ends_loading() {
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    drain(&mut rx);
    crate::gui::playlist_add::choose(&t.ui, &core, "__existing__", vec![song("a")]);
    assert!(t.ui.global::<crate::AddPlaylist>().get_loading());
    crate::gui::playlist_add::targets_arrived(&t.ui, Vec::new());
    assert!(!t.ui.global::<crate::AddPlaylist>().get_loading());
}

/// An empty answer (failed start-up prefetch) is asked for again the next time the dialog opens.
#[test]
fn empty_list_is_reloaded_on_open() {
    use crate::core::msg::Command;
    let t = app();
    let (core, mut rx) = crate::core::runtime::CoreHandle::fake();
    crate::gui::playlist_add::wire(&t.ui, &core);
    crate::gui::playlist_add::targets_arrived(&t.ui, Vec::new());
    drain(&mut rx);
    crate::gui::playlist_add::choose(&t.ui, &core, "__existing__", vec![song("a")]);
    assert!(
        drain(&mut rx)
            .iter()
            .any(|c| matches!(c, Command::LoadAddTargets))
    );
    assert!(t.ui.global::<crate::AddPlaylist>().get_loading());
}
