//! Stream URL resolution through yt-dlp (`-J`). We never touch signatures or challenges:
//! yt-dlp returns a direct URL plus the exact headers to send with it.
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// m4a/AAC first: symphonia decodes it and YouTube serves it up to 256 kbps for Premium.
pub const FORMAT: &str = "bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]";

#[derive(Debug, Clone)]
pub struct Resolved {
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub duration: Option<f64>,
    pub bitrate_kbps: Option<u32>,
    pub codec: String,
    /// Unix seconds after which the URL stops working.
    pub expires: u64,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn video_id(target: &str) -> String {
    match target.split_once("v=") {
        Some((_, rest)) => rest.split('&').next().unwrap_or(rest).to_string(),
        None => target.rsplit('/').next().unwrap_or(target).to_string(),
    }
}

/// Parse `yt-dlp -J` output (one video, one selected audio format).
pub fn parse(json: &Value) -> Result<Resolved> {
    let url = json
        .get("url")
        .and_then(Value::as_str)
        .context("yt-dlp output has no stream url")?
        .to_string();
    let headers = json
        .get("http_headers")
        .and_then(Value::as_object)
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
                .collect()
        })
        .unwrap_or_default();
    let expires = url
        .split(['?', '&'])
        .find_map(|kv| kv.strip_prefix("expire=")?.parse::<u64>().ok())
        .unwrap_or_else(|| now() + 3600);
    Ok(Resolved {
        url,
        headers,
        duration: json.get("duration").and_then(Value::as_f64),
        bitrate_kbps: json
            .get("abr")
            .and_then(Value::as_f64)
            .map(|a| a.round() as u32),
        codec: json
            .get("acodec")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        expires,
    })
}

pub struct Resolver {
    ytdlp: PathBuf,
    cookies: Mutex<Option<PathBuf>>,
    cache: Mutex<HashMap<String, Resolved>>,
}

impl Resolver {
    pub fn new(ytdlp: PathBuf, cookies: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            ytdlp,
            cookies: Mutex::new(cookies),
            cache: Mutex::new(HashMap::new()),
        })
    }

    pub fn set_cookies(&self, c: Option<PathBuf>) {
        // new cookies may unlock better formats: forget resolved URLs
        self.cache.lock().unwrap().clear();
        *self.cookies.lock().unwrap() = c;
    }

    /// Blocking: runs yt-dlp unless a still-valid URL is cached (60 s safety margin).
    pub fn resolve(&self, id: &str, force: bool) -> Result<Resolved> {
        if !force
            && let Some(r) = self
                .cache
                .lock()
                .unwrap()
                .get(id)
                .filter(|r| r.expires > now() + 60)
        {
            return Ok(r.clone());
        }
        let mut cmd = std::process::Command::new(&self.ytdlp);
        cmd.args([
            "-J",
            "--no-playlist",
            "--no-warnings",
            "--socket-timeout",
            "15",
            "-f",
            FORMAT,
        ]);
        if let Some(c) = self.cookies.lock().unwrap().clone().filter(|c| c.exists()) {
            cmd.arg("--cookies").arg(&c);
        }
        cmd.arg(format!("https://music.youtube.com/watch?v={id}"));
        let out = cmd
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .context("run yt-dlp")?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            bail!(
                "yt-dlp failed: {}",
                err.lines().last().unwrap_or("unknown error")
            );
        }
        let json: Value = serde_json::from_slice(&out.stdout).context("yt-dlp JSON")?;
        let r = parse(&json)?;
        let mut cache = self.cache.lock().unwrap();
        cache.retain(|_, v| v.expires > now()); // drop expired entries
        cache.insert(id.to_string(), r.clone());
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(
            video_id("https://music.youtube.com/watch?v=abc123&list=x"),
            "abc123"
        );
        assert_eq!(video_id("abc123"), "abc123");
    }

    #[test]
    fn parses_ytdlp_json() {
        let j = serde_json::json!({
            "url": "https://rr1.googlevideo.com/videoplayback?expire=1791333893&itag=140",
            "http_headers": {"User-Agent": "UA", "Accept": "*/*"},
            "duration": 338, "abr": 129.546, "acodec": "mp4a.40.2"
        });
        let r = parse(&j).unwrap();
        assert_eq!(r.expires, 1_791_333_893);
        assert_eq!(r.bitrate_kbps, Some(130));
        assert_eq!(r.duration, Some(338.0));
        assert!(
            r.headers
                .iter()
                .any(|(k, v)| k == "User-Agent" && v == "UA")
        );
        assert!(parse(&serde_json::json!({})).is_err());
    }
}
