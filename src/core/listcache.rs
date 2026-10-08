//! Disk cache for opened playlists and liked songs (in the cache dir): the next open shows the
//! cached list at once while a fresh copy is fetched (stale-while-revalidate).
use super::model::Item;
use std::path::{Path, PathBuf};

pub(crate) fn file(dir: &Path, key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    dir.join(format!("{safe}.json"))
}

pub fn load(dir: &Path, key: &str) -> Option<Vec<Item>> {
    let bytes = std::fs::read(file(dir, key)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(dir: &Path, key: &str, items: &[Item]) {
    use std::io::Write;
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let _ = std::fs::create_dir_all(dir);
    let path = file(dir, key);
    // unique temp name: two writers of the same list must not share a file
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{n}.tmp", std::process::id()));
    let Ok(bytes) = serde_json::to_vec(items) else {
        return;
    };
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let ok = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(&bytes))
        .is_ok();
    if ok && std::fs::rename(&tmp, &path).is_ok() {
        return;
    }
    let _ = std::fs::remove_file(&tmp);
}

/// Where two lists first differ, for the log ("row 3: title \"a\" vs \"b\"").
pub fn first_difference(a: &[Item], b: &[Item]) -> Option<String> {
    if let Some(n) = a
        .iter()
        .zip(b)
        .position(|(x, y)| x.video_id != y.video_id || x.title != y.title)
    {
        return Some(format!(
            "row {n}: {:?} {:?} vs {:?} {:?}",
            a[n].video_id, a[n].title, b[n].video_id, b[n].title
        ));
    }
    (a.len() != b.len()).then(|| format!("length {} vs {}", a.len(), b.len()))
}

/// Same tracks in the same order (what the user sees); metadata-only changes do not count.
pub fn same_list(a: &[Item], b: &[Item]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| x.video_id == y.video_id && x.title == y.title)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::Kind;

    fn item(id: &str, title: &str) -> Item {
        Item {
            kind: Kind::Song,
            title: title.into(),
            video_id: Some(id.into()),
            browse_id: None,
            artists: vec![],
            album: None,
            album_id: None,
            duration: None,
            duration_secs: None,
            thumbnail: None,
            subtitle: String::new(),
        }
    }

    #[test]
    fn round_trip_unsafe_keys_and_comparison() {
        let dir = std::env::temp_dir().join(format!("cb-listcache-{}", std::process::id()));
        let items = vec![item("a", "One"), item("b", "Two")];
        assert!(load(&dir, "PL/../x").is_none());
        save(&dir, "PL/../x", &items);
        let back = load(&dir, "PL/../x").unwrap();
        assert!(same_list(&items, &back));
        // the key cannot escape the directory
        assert!(dir.join("PL____x.json").exists());
        assert!(!same_list(&items, &items[..1]));
        let mut renamed = items.clone();
        renamed[1].title = "Changed".into();
        assert!(!same_list(&items, &renamed));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
