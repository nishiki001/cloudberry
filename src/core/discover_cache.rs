//! Disk cache with a timestamp for Discover sections and artist pages: the cached copy
//! is shown at once; a fresh one is fetched in the background when it is older than 30 minutes.
use super::listcache::file;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

/// Older than this → refresh in the background.
pub const MAX_AGE_SECS: u64 = 30 * 60;

#[derive(Serialize, Deserialize)]
struct Entry<T> {
    fetched_at: u64,
    data: T,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The cached data and its age in seconds.
pub fn load<T: serde::de::DeserializeOwned>(dir: &Path, key: &str) -> Option<(T, u64)> {
    let e: Entry<T> = serde_json::from_slice(&std::fs::read(file(dir, key)).ok()?).ok()?;
    Some((e.data, now().saturating_sub(e.fetched_at)))
}

pub fn save<T: Serialize + Clone>(dir: &Path, key: &str, data: &T) {
    save_at(dir, key, data, now());
}

fn save_at<T: Serialize + Clone>(dir: &Path, key: &str, data: &T, fetched_at: u64) {
    let _ = std::fs::create_dir_all(dir);
    let entry = Entry {
        fetched_at,
        data: data.clone(),
    };
    let path = file(dir, key);
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = path.with_extension(format!("{}.{n}.tmp", std::process::id()));
    if let Ok(bytes) = serde_json::to_vec(&entry)
        && std::fs::write(&tmp, bytes).is_ok()
    {
        let _ = std::fs::rename(&tmp, &path);
    }
}

pub fn is_stale(age_secs: u64) -> bool {
    age_secs > MAX_AGE_SECS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::api::discover_parse::{DiscoverData, Mood, Shelf, ShelfKind};

    #[test]
    fn round_trip_age_and_staleness() {
        let dir = std::env::temp_dir().join(format!("cb-discover-{}", std::process::id()));
        let data = DiscoverData {
            shelves: vec![Shelf {
                title: "T".into(),
                kind: ShelfKind::Cards,
                items: vec![],
            }],
            moods: vec![Mood {
                title: "Pop".into(),
                params: "p".into(),
                group: "g".into(),
            }],
        };
        assert!(load::<DiscoverData>(&dir, "home:ZZ").is_none());
        save(&dir, "home:ZZ", &data);
        let (back, age): (DiscoverData, u64) = load(&dir, "home:ZZ").unwrap();
        assert_eq!(back, data);
        assert!(age < 5 && !is_stale(age));
        save_at(&dir, "old", &data, now() - 31 * 60);
        assert!(is_stale(load::<DiscoverData>(&dir, "old").unwrap().1));
        assert!(!is_stale(29 * 60));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
