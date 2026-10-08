//! Cookie file handling and SAPISIDHASH. No Slint, no network.
use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use std::path::Path;

pub const ORIGIN: &str = "https://music.youtube.com";

#[derive(Clone, PartialEq, Eq)]
pub struct Cookie {
    pub domain: String,
    pub name: String,
    pub value: String,
}

// Never print values.
impl std::fmt::Debug for Cookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Cookie({} {} <redacted>)", self.domain, self.name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Cookies(pub Vec<Cookie>);

/// Parse Netscape cookie file text; keeps only `.youtube.com` cookies.
pub fn parse_netscape(text: &str) -> Cookies {
    let mut out = Vec::new();
    for line in text.lines() {
        let line = line.trim_end();
        let line = match line.strip_prefix("#HttpOnly_") {
            Some(rest) => rest,
            None if line.starts_with('#') || line.is_empty() => continue,
            None => line,
        };
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 7 {
            tracing::debug!("skipping malformed cookie line");
            continue;
        }
        let domain = f[0];
        if !is_youtube_domain(domain) {
            continue;
        }
        out.push(Cookie {
            domain: domain.into(),
            name: f[5].into(),
            value: f[6].into(),
        });
    }
    Cookies(out)
}

fn is_youtube_domain(d: &str) -> bool {
    let d = d.trim_start_matches('.');
    d == "youtube.com" || d.ends_with(".youtube.com")
}

pub fn load_file(path: &Path) -> Result<Cookies> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    Ok(parse_netscape(&text))
}

impl Cookies {
    pub fn get(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.value.as_str())
    }

    pub fn sapisid(&self) -> Option<&str> {
        self.get("SAPISID")
            .or_else(|| self.get("__Secure-3PAPISID"))
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `name=value; name=value` header value.
    pub fn header(&self) -> String {
        self.0
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub fn authorization(&self, now_secs: u64) -> Option<String> {
        self.sapisid().map(|s| sapisidhash(s, now_secs))
    }
}

pub fn sapisidhash(sapisid: &str, ts: u64) -> String {
    let mut h = Sha1::new();
    h.update(format!("{ts} {sapisid} {ORIGIN}").as_bytes());
    format!("SAPISIDHASH {ts}_{}", hex::encode(h.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Netscape HTTP Cookie File\n\
.youtube.com\tTRUE\t/\tTRUE\t1900000000\tPREF\tf6=1\n\
#HttpOnly_.youtube.com\tTRUE\t/\tTRUE\t1900000000\tSAPISID\tfakesapisid\n\
.google.com\tTRUE\t/\tTRUE\t1900000000\tNID\tother\n\
.example.com\tTRUE\t/\tFALSE\t0\tx\ty\n\
broken line\n";

    #[test]
    fn parses_and_filters() {
        let c = parse_netscape(SAMPLE);
        assert_eq!(c.0.len(), 2);
        assert_eq!(c.sapisid(), Some("fakesapisid"));
        assert_eq!(c.header(), "PREF=f6=1; SAPISID=fakesapisid");
    }

    #[test]
    fn falls_back_to_3papisid() {
        let c = parse_netscape(".youtube.com\tTRUE\t/\tTRUE\t0\t__Secure-3PAPISID\tabc\n");
        assert_eq!(c.sapisid(), Some("abc"));
    }

    #[test]
    fn hash_is_stable() {
        let h = sapisidhash("abc", 1_700_000_000);
        assert_eq!(
            h,
            "SAPISIDHASH 1700000000_2f3ec011e870f3fbd0238c090c2062c208cead32"
        );
        assert!(h.starts_with("SAPISIDHASH 1700000000_"));
        assert_eq!(h.len(), "SAPISIDHASH 1700000000_".len() + 40);
        assert_eq!(h, sapisidhash("abc", 1_700_000_000));
        assert_ne!(h, sapisidhash("abd", 1_700_000_000));
    }

    #[test]
    fn debug_hides_values() {
        let c = parse_netscape(SAMPLE);
        assert!(!format!("{c:?}").contains("fakesapisid"));
    }
}
