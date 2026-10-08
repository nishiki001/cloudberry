#![allow(dead_code)] // remove/clear helpers are used from M6/M10 on
//! Thumbnail pipeline: fetch (never with cookies) → disk cache → decode → square crop → LRU.
use anyhow::{Context, Result, bail};
use image::{RgbaImage, imageops::FilterType};
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Edge length of decoded covers. Memory per entry = EDGE² × 4 bytes (~400 KB).
pub const EDGE: u32 = 320;
/// Edge of list-row thumbnails.
pub const SMALL_EDGE: u32 = 64;

/// Smaller variant of an upscaled googleusercontent URL (for list rows).
pub fn small_url(url: &str) -> String {
    match url.rfind("=w544-h544") {
        Some(i) => format!("{}=w96-h96-l90-rj", &url[..i]),
        None => url.to_string(),
    }
}
const LRU_CAP: usize = 64;
static PART_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn host_allowed(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("");
    ["googleusercontent.com", "ytimg.com", "ggpht.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

pub struct Thumbs {
    http: reqwest::Client, // deliberately no cookie store, no auth headers
    dir: PathBuf,
    lru: Mutex<VecDeque<(String, Arc<RgbaImage>)>>,
}

/// Center-crop to a square and resize to `edge`.
pub fn square(img: image::DynamicImage, edge: u32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let s = w.min(h);
    let cropped = img.crop_imm((w - s) / 2, (h - s) / 2, s, s);
    image::imageops::resize(&cropped.to_rgba8(), edge, edge, FilterType::Triangle)
}

impl Thumbs {
    pub fn new(dir: PathBuf) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("cloudberry")
            .build()?;
        Ok(Self {
            http,
            dir,
            lru: Mutex::new(VecDeque::new()),
        })
    }

    fn path_for(&self, url: &str) -> PathBuf {
        self.dir.join(hex::encode(Sha1::digest(url.as_bytes())))
    }

    fn lru_get(&self, url: &str) -> Option<Arc<RgbaImage>> {
        let mut l = self.lru.lock().unwrap();
        let i = l.iter().position(|(u, _)| u == url)?;
        let e = l.remove(i)?;
        l.push_back(e.clone());
        Some(e.1)
    }

    fn lru_put(&self, url: &str, img: Arc<RgbaImage>) {
        let mut l = self.lru.lock().unwrap();
        l.retain(|(u, _)| u != url);
        l.push_back((url.to_string(), img));
        while l.len() > LRU_CAP {
            l.pop_front();
        }
    }

    pub async fn get(&self, url: &str) -> Result<Arc<RgbaImage>> {
        self.get_edge(url, EDGE).await
    }

    /// Square thumbnail decoded at `edge` px (small ones for list rows); same disk cache.
    pub async fn get_edge(&self, url: &str, edge: u32) -> Result<Arc<RgbaImage>> {
        let key = format!("{edge}:{url}");
        if let Some(i) = self.lru_get(&key) {
            return Ok(i);
        }
        if !host_allowed(url) {
            bail!("thumbnail host not allowed");
        }
        let path = self.path_for(url);
        let bytes = match tokio::fs::read(&path).await {
            Ok(b) => b,
            Err(_) => {
                let resp = self
                    .http
                    .get(url)
                    .send()
                    .await
                    .context("thumbnail request")?
                    .error_for_status()?;
                let b = resp.bytes().await?.to_vec();
                let _ = tokio::fs::create_dir_all(&self.dir).await;
                let part = path.with_extension(format!(
                    "part{}",
                    PART_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                if tokio::fs::write(&part, &b).await.is_ok() {
                    let _ = tokio::fs::rename(&part, &path).await;
                }
                b
            }
        };
        let img = tokio::task::spawn_blocking(move || -> Result<RgbaImage> {
            Ok(square(
                image::load_from_memory(&bytes).context("decode thumbnail")?,
                EDGE,
            ))
        })
        .await??;
        let img = Arc::new(img);
        self.lru_put(&key, img.clone());
        Ok(img)
    }

    /// Delete the oldest cache files until the directory is under `max_bytes`.
    pub fn prune(&self, max_bytes: u64) {
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<_> = rd
            .flatten()
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                Some((m.modified().ok()?, m.len(), e.path()))
            })
            .collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        for (_, len, path) in files {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total -= len;
            }
        }
    }

    pub fn clear_disk(&self) -> u64 {
        let mut n = 0;
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for e in rd.flatten() {
                if let Ok(m) = e.metadata() {
                    n += m.len();
                }
                let _ = std::fs::remove_file(e.path());
            }
        }
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn small_urls() {
        assert_eq!(
            small_url("https://lh3.googleusercontent.com/x=w544-h544-l90-rj"),
            "https://lh3.googleusercontent.com/x=w96-h96-l90-rj"
        );
        assert_eq!(
            small_url("https://i.ytimg.com/vi/a/hq.jpg"),
            "https://i.ytimg.com/vi/a/hq.jpg"
        );
    }

    #[test]
    fn host_rules() {
        assert!(host_allowed("https://lh3.googleusercontent.com/x=w1"));
        assert!(host_allowed("https://i.ytimg.com/vi/a/b.jpg"));
        assert!(!host_allowed("http://i.ytimg.com/vi/a/b.jpg"));
        assert!(!host_allowed("https://evil.com/ytimg.com"));
        assert!(!host_allowed("https://notytimg.com/x"));
    }
    #[test]
    fn crops_wide_to_square() {
        let img = image::DynamicImage::ImageRgba8(RgbaImage::new(160, 90));
        let s = square(img, 64);
        assert_eq!((s.width(), s.height()), (64, 64));
    }
    #[tokio::test]
    async fn disk_cache_roundtrip_without_network() {
        let dir = tempfile::tempdir().unwrap();
        let t = Thumbs::new(dir.path().to_path_buf()).unwrap();
        let url = "https://i.ytimg.com/vi/test/hq.png";
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(RgbaImage::new(40, 30))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(t.path_for(url), png).unwrap();
        let img = t.get(url).await.unwrap();
        assert_eq!(img.width(), EDGE);
        assert!(t.get(url).await.is_ok()); // LRU hit
    }
}
