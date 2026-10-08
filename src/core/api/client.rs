use crate::core::auth::{self, Cookies, ORIGIN};
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const BASE: &str = "https://music.youtube.com/youtubei/v1/";
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:88.0) Gecko/20100101 Firefox/88.0";
const RETRIES: usize = 3;

pub struct Client {
    base: String,
    http: reqwest::Client,
    cookie_path: Option<PathBuf>,
    // (mtime, cookies) so a yt-dlp rewrite is picked up.
    cookies: Mutex<(Option<SystemTime>, Cookies)>,
}

pub fn client_version(now_secs: u64) -> String {
    // civil date from unix days (UTC)
    let z = (now_secs / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("1.{y:04}{m:02}{d:02}.01.00")
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Client {
    pub fn new(cookie_path: Option<PathBuf>) -> Result<Self> {
        Self::with_base(cookie_path, BASE.to_string())
    }

    /// A client talking to another InnerTube base URL (a fake server in tests).
    pub(crate) fn with_base(cookie_path: Option<PathBuf>, base: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(UA)
            .build()?;
        let c = Self {
            base,
            http,
            cookie_path,
            cookies: Mutex::new((None, Cookies::default())),
        };
        c.reload_cookies();
        Ok(c)
    }

    fn reload_cookies(&self) {
        let Some(p) = &self.cookie_path else { return };
        let Ok(mtime) = p.metadata().and_then(|m| m.modified()) else {
            *self.cookies.lock().unwrap() = (None, Cookies::default());
            return;
        };
        let mut g = self.cookies.lock().unwrap();
        if g.0 != Some(mtime) {
            *g = (Some(mtime), auth::load_file(p).unwrap_or_default());
        }
    }

    pub fn has_cookies(&self) -> bool {
        self.reload_cookies();
        !self.cookies.lock().unwrap().1.is_empty()
    }

    pub fn context() -> Value {
        json!({"client": {"clientName": "WEB_REMIX", "clientVersion": client_version(now()),
            "hl": "en", "gl": "US"}, "user": {}})
    }

    /// POST an endpoint with `extra` merged into the body next to `context`.
    pub async fn post(&self, endpoint: &str, extra: Value) -> Result<Value> {
        self.post_q(endpoint, extra, "").await
    }

    /// Like `post` with extra URL query parameters (`&a=b`), as search continuations need.
    pub async fn post_q(&self, endpoint: &str, extra: Value, query: &str) -> Result<Value> {
        let mut body = json!({"context": Self::context()});
        if let (Some(b), Value::Object(e)) = (body.as_object_mut(), extra) {
            b.extend(e);
        }
        let url = format!("{}{endpoint}?alt=json&prettyPrint=false{query}", self.base);
        let mut last = anyhow!("no attempt");
        for attempt in 0..RETRIES {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(400 * (1 << attempt))).await;
            }
            match self.try_post(&url, &body).await {
                Ok(v) => return Ok(v),
                Err((e, retry)) => {
                    tracing::debug!("{endpoint} attempt {attempt}: {e}");
                    last = e;
                    if !retry {
                        break;
                    }
                }
            }
        }
        Err(last)
    }

    async fn try_post(
        &self,
        url: &str,
        body: &Value,
    ) -> std::result::Result<Value, (anyhow::Error, bool)> {
        self.reload_cookies();
        let mut req = self
            .http
            .post(url)
            .header("Accept-Language", "en-US,en;q=0.5")
            .header("Origin", ORIGIN)
            .header("X-Origin", ORIGIN)
            .json(body);
        {
            let g = self.cookies.lock().unwrap();
            if let Some(a) = g.1.authorization(now()) {
                req = req
                    .header("Authorization", a)
                    .header("X-Goog-AuthUser", "0")
                    .header("Cookie", g.1.header());
            }
        }
        let resp = req
            .send()
            .await
            .map_err(|e| (anyhow!("request failed: {}", e.without_url()), true))?;
        let status = resp.status();
        if !status.is_success() {
            let retry = status.is_server_error() || status.as_u16() == 429;
            return Err((anyhow!("HTTP {status}"), retry));
        }
        resp.json::<Value>()
            .await
            .map_err(|e| (anyhow!("bad JSON: {e}"), false))
    }

    /// True if the account menu reports a signed-in user.
    pub async fn signed_in(&self) -> Result<bool> {
        if !self.has_cookies() {
            return Ok(false);
        }
        let v = self.post("account/account_menu", json!({})).await?;
        Ok(crate::core::api::nav::nav(
            &v,
            &[
                "actions",
                "0",
                "openPopupAction",
                "popup",
                "multiPageMenuRenderer",
                "header",
            ],
        )
        .is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_from_date() {
        assert_eq!(client_version(0), "1.19700101.01.00");
        assert_eq!(client_version(1_700_000_000), "1.20231114.01.00");
        assert_eq!(client_version(1_709_164_800), "1.20240229.01.00");
    }
}
