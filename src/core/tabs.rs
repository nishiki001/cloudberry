//! Open playlist tabs, persisted so they survive a restart (stored with their items, so
//! restoring needs no network). The Queue tab is not stored: the queue is rebuilt by playback.
use super::model::Item;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabSave {
    pub name: String,
    /// "list" (opened playlist/album/artist), "radio" or "empty" (user-created).
    pub kind: String,
    pub items: Vec<Item>,
    /// The playlist this tab was opened from (so edits of that playlist reach the tab).
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TabsFile {
    pub tabs: Vec<TabSave>,
    /// Index into `tabs` of the tab that was showing (0 = Queue is stored as `None`).
    pub current: Option<usize>,
}

pub fn load(path: &Path) -> TabsFile {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(path: &Path, f: &TabsFile) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).context("create data dir")?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec(f)?).context("write tabs")?;
    std::fs::rename(&tmp, path).context("replace tabs file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::Kind;

    fn item(t: &str) -> Item {
        Item {
            kind: Kind::Song,
            title: t.into(),
            video_id: Some("v".into()),
            browse_id: None,
            artists: vec![],
            album: None,
            album_id: None,
            duration: None,
            duration_secs: Some(10),
            thumbnail: None,
            subtitle: String::new(),
        }
    }

    #[test]
    fn roundtrip_and_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tabs.json");
        assert_eq!(load(&p), TabsFile::default());
        let f = TabsFile {
            tabs: vec![TabSave {
                name: "Radio: シキ".into(),
                kind: "radio".into(),
                items: vec![item("a"), item("b")],
                source: Some("PLx".into()),
            }],
            current: Some(0),
        };
        save(&p, &f).unwrap();
        assert_eq!(load(&p), f);
    }

    #[test]
    fn corrupt_file_falls_back_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tabs.json");
        std::fs::write(&p, "{ not json").unwrap();
        assert_eq!(load(&p), TabsFile::default());
    }

    #[test]
    fn old_files_without_a_source_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("tabs.json");
        std::fs::write(
            &p,
            r#"{"tabs":[{"name":"A","kind":"list","items":[]}],"current":0}"#,
        )
        .unwrap();
        let f = load(&p);
        assert_eq!(f.tabs.len(), 1);
        assert_eq!(f.tabs[0].source, None);
    }
}
