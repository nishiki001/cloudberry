use super::client::Client;
use super::parse::{SearchPage, parse_search_page};
use anyhow::{Result, bail};
use serde_json::json;

/// Search filter params (ytmusicapi `get_search_params`, spelling suggestions on).
pub fn filter_params(filter: &str) -> Result<&'static str> {
    Ok(match filter {
        "songs" => "EgWKAQIIAWoMEA4QChADEAQQCRAF",
        "videos" => "EgWKAQIQAWoMEA4QChADEAQQCRAF",
        "albums" => "EgWKAQIYAWoMEA4QChADEAQQCRAF",
        "artists" => "EgWKAQIgAWoMEA4QChADEAQQCRAF",
        "playlists" => "Eg-KAQwIABAAGAAgACgBMABqChAEEAMQCRAFEAo%3D",
        other => bail!("unknown filter '{other}' (songs, videos, albums, artists, playlists)"),
    })
}

impl Client {
    /// First page of results and the token for the next one.
    pub async fn search_page(&self, query: &str, filter: Option<&str>) -> Result<SearchPage> {
        let mut body = json!({"query": query});
        if let Some(f) = filter {
            body["params"] = json!(filter_params(f)?);
        }
        Ok(parse_search_page(&self.post("search", body).await?))
    }

    /// The page after the one that returned `token`.
    pub async fn search_more(&self, token: &str) -> Result<SearchPage> {
        let q = format!("&ctoken={token}&continuation={token}&type=next");
        let page = parse_search_page(&self.post_q("search", json!({}), &q).await?);
        Ok(page)
    }

    /// Up to `limit` results (0 = the first page only), following continuation pages.
    pub async fn search_limit(
        &self,
        query: &str,
        filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<crate::core::model::Item>> {
        let mut page = self.search_page(query, filter).await?;
        let mut items = std::mem::take(&mut page.items);
        while items.len() < limit {
            let Some(t) = page.continuation.take() else {
                break;
            };
            page = self.search_more(&t).await?;
            if page.items.is_empty() {
                break;
            }
            items.append(&mut page.items);
        }
        Ok(items)
    }
}
