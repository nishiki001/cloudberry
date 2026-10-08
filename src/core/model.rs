//! Plain data types shared by API, CLI and GUI.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Song,
    Video,
    Album,
    Artist,
    Playlist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artist {
    pub name: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub kind: Kind,
    pub title: String,
    pub video_id: Option<String>,
    pub browse_id: Option<String>,
    pub artists: Vec<Artist>,
    pub album: Option<String>,
    pub album_id: Option<String>,
    pub duration: Option<String>,
    pub duration_secs: Option<u32>,
    pub thumbnail: Option<String>,
    /// Raw secondary line ("Album • Daft Punk • 2013", "126K views", …).
    pub subtitle: String,
}

pub fn parse_duration(s: &str) -> Option<u32> {
    let mut total = 0u32;
    let mut n = 0;
    for part in s.split(':') {
        if part.is_empty() || part.len() > 2 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        total = total * 60 + part.parse::<u32>().ok()?;
        n += 1;
    }
    (2..=3).contains(&n).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn durations() {
        assert_eq!(parse_duration("5:38"), Some(338));
        assert_eq!(parse_duration("1:02:03"), Some(3723));
        assert_eq!(parse_duration("2013"), None);
        assert_eq!(parse_duration("1.2B plays"), None);
    }
}
