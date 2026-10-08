//! Seekable HTTP source for symphonia: chunked Range requests with one chunk of read-ahead.
use anyhow::{Context, Result, bail};
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;
use symphonia::core::io::MediaSource;

const CHUNK: u64 = 2 * 1024 * 1024;
const RETRIES: usize = 3;

type Headers = Vec<(String, String)>;
/// Re-resolves the stream (new URL + headers) after an HTTP 403/410.
pub type Refresher = Box<dyn Fn() -> Result<(String, Headers)> + Send + Sync>;

struct Fetch {
    start: u64,
    rx: Mutex<Receiver<Result<Vec<u8>, String>>>, // Mutex only to make the source Sync
}

pub struct HttpSource {
    client: reqwest::blocking::Client,
    url: String,
    headers: Headers,
    refresh: Option<Refresher>,
    len: u64,
    pos: u64,
    chunk: Vec<u8>,
    chunk_start: u64,
    ahead: Option<Fetch>,
}

fn build_request(
    client: &reqwest::blocking::Client,
    url: &str,
    headers: &Headers,
    start: u64,
    end: u64,
) -> reqwest::blocking::RequestBuilder {
    let mut req = client
        .get(url)
        .header("Range", format!("bytes={start}-{end}"));
    for (k, v) in headers {
        // only what yt-dlp told us to send; never add cookies of our own
        if !k.eq_ignore_ascii_case("range") {
            req = req.header(k.as_str(), v.as_str());
        }
    }
    req
}

fn fetch(
    client: &reqwest::blocking::Client,
    url: &str,
    headers: &Headers,
    start: u64,
    end: u64,
) -> Result<(Vec<u8>, Option<u64>), u16> {
    let resp = build_request(client, url, headers, start, end)
        .send()
        .map_err(|_| 0u16)?;
    let status = resp.status().as_u16();
    // a server that ignores Range (plain 200) would hand us the wrong bytes for start > 0
    if status != 206 && !(status == 200 && start == 0) {
        return Err(status);
    }
    let total = resp
        .headers()
        .get("content-range")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit('/').next()?.parse::<u64>().ok());
    let bytes = resp.bytes().map_err(|_| 0u16)?.to_vec();
    Ok((bytes, total))
}

impl HttpSource {
    pub fn open(url: String, headers: Headers, refresh: Option<Refresher>) -> Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        let mut me = Self {
            client,
            url,
            headers,
            refresh,
            len: 0,
            pos: 0,
            chunk: Vec::new(),
            chunk_start: 0,
            ahead: None,
        };
        let (bytes, total) = me.fetch_with_retry(0, CHUNK - 1)?;
        me.len = total.context("server did not report the stream length")?;
        me.chunk = bytes;
        Ok(me)
    }

    fn fetch_with_retry(&mut self, start: u64, end: u64) -> Result<(Vec<u8>, Option<u64>)> {
        let mut last = 0u16;
        for attempt in 0..RETRIES {
            match fetch(&self.client, &self.url, &self.headers, start, end) {
                Ok(r) => return Ok(r),
                Err(status) => {
                    last = status;
                    if matches!(status, 403 | 410) {
                        if let Some(r) = &self.refresh {
                            let (u, h) = r().context("re-resolving expired stream")?;
                            self.url = u;
                            self.headers = h;
                            continue;
                        }
                        bail!("stream URL rejected (HTTP {status})");
                    }
                    std::thread::sleep(Duration::from_millis(300 * (attempt as u64 + 1)));
                }
            }
        }
        bail!("download failed (HTTP {last})")
    }

    fn start_ahead(&mut self, start: u64) {
        if start >= self.len || self.ahead.as_ref().is_some_and(|a| a.start == start) {
            return;
        }
        let (client, url, headers) = (self.client.clone(), self.url.clone(), self.headers.clone());
        let end = (start + CHUNK - 1).min(self.len - 1);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let r = fetch(&client, &url, &headers, start, end)
                .map(|(b, _)| b)
                .map_err(|s| format!("HTTP {s}"));
            let _ = tx.send(r);
        });
        self.ahead = Some(Fetch {
            start,
            rx: Mutex::new(rx),
        });
    }

    /// Make `pos` fall inside `self.chunk`.
    fn load_chunk(&mut self) -> io::Result<()> {
        let want = self.pos / CHUNK * CHUNK;
        // a read-ahead for another region is simply dropped
        if let Some(a) = self.ahead.take().filter(|a| a.start == want)
            && let Ok(Ok(bytes)) = a.rx.lock().unwrap().recv()
        {
            self.chunk = bytes;
            self.chunk_start = want;
            return Ok(());
        }
        let end = (want + CHUNK - 1).min(self.len - 1);
        let (bytes, _) = self
            .fetch_with_retry(want, end)
            .map_err(|e| io::Error::other(e.to_string()))?;
        self.chunk = bytes;
        self.chunk_start = want;
        Ok(())
    }
}

impl Read for HttpSource {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len || out.is_empty() {
            return Ok(0);
        }
        let in_chunk =
            self.pos >= self.chunk_start && self.pos < self.chunk_start + self.chunk.len() as u64;
        if !in_chunk {
            self.load_chunk()?;
        }
        let off = (self.pos - self.chunk_start) as usize;
        let n = out.len().min(self.chunk.len().saturating_sub(off));
        if n == 0 {
            return Ok(0);
        }
        out[..n].copy_from_slice(&self.chunk[off..off + n]);
        self.pos += n as u64;
        // read-ahead once we are past the middle of the chunk
        if self.pos - self.chunk_start > CHUNK / 2 {
            let next = self.chunk_start + CHUNK;
            self.start_ahead(next);
        }
        Ok(n)
    }
}

impl Seek for HttpSource {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        let np = match to {
            SeekFrom::Start(p) => p as i128,
            SeekFrom::Current(d) => self.pos as i128 + d as i128,
            SeekFrom::End(d) => self.len as i128 + d as i128,
        };
        if np < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "seek before start",
            ));
        }
        self.pos = (np as u64).min(self.len);
        Ok(self.pos)
    }
}

impl MediaSource for HttpSource {
    fn is_seekable(&self) -> bool {
        true
    }
    fn byte_len(&self) -> Option<u64> {
        Some(self.len)
    }
}
