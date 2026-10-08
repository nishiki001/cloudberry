//! Client methods for playlists, liked songs and the library pages.
use super::client::Client;
use super::library::*;
use crate::core::model::Item;
use anyhow::Result;
use serde_json::json;

const MAX_PAGES: usize = 40;

impl Client {
    /// All tracks of a playlist id (no "VL" prefix), following continuations.
    pub async fn playlist(&self, id: &str) -> Result<Vec<Item>> {
        self.playlist_pages(id, MAX_PAGES).await
    }

    /// Like `playlist` but follows at most `max_pages` continuation pages.
    pub async fn playlist_pages(&self, id: &str, max_pages: usize) -> Result<Vec<Item>> {
        let mut all = Vec::new();
        self.playlist_stream(id, max_pages, |page| all.extend(page))
            .await?;
        Ok(all)
    }

    /// Fetch a playlist page by page, handing each parsed page to `on_page` as soon as it has
    /// arrived (continuation tokens chain, so the pages themselves are fetched one after the
    /// other, each as soon as the previous one has been parsed).
    pub async fn playlist_stream(
        &self,
        id: &str,
        max_pages: usize,
        mut on_page: impl FnMut(Vec<Item>),
    ) -> Result<()> {
        let t0 = std::time::Instant::now();
        let first = self
            .post("browse", json!({"browseId": format!("VL{id}")}))
            .await?;
        let t_req = t0.elapsed();
        let (items, mut token) = parse_playlist_page(&first);
        tracing::info!(
            request_ms = t_req.as_millis() as u64,
            parse_ms = (t0.elapsed() - t_req).as_millis() as u64,
            rows = items.len(),
            "playlist first page"
        );
        on_page(items);
        for _ in 0..max_pages {
            let Some(t) = token.take() else { break };
            let t1 = std::time::Instant::now();
            let page = self.post("browse", json!({"continuation": t})).await?;
            let t_req = t1.elapsed();
            let (more, next) = parse_playlist_page(&page);
            tracing::info!(
                request_ms = t_req.as_millis() as u64,
                parse_ms = (t1.elapsed() - t_req).as_millis() as u64,
                rows = more.len(),
                "playlist page"
            );
            if more.is_empty() {
                break;
            }
            on_page(more);
            token = next;
        }
        if token.is_some() {
            tracing::debug!(max_pages, "playlist truncated at page cap");
        }
        Ok(())
    }

    /// Like or unlike a track.
    pub async fn set_like(&self, video_id: &str, like: bool) -> Result<()> {
        let ep = if like { "like/like" } else { "like/removelike" };
        self.post(ep, json!({"target": {"videoId": video_id}}))
            .await?;
        Ok(())
    }

    pub async fn liked(&self) -> Result<Vec<Item>> {
        self.playlist("LM").await
    }

    pub async fn library_playlists(&self) -> Result<Vec<Item>> {
        Ok(parse_library_playlists(
            &self
                .post("browse", json!({"browseId": "FEmusic_liked_playlists"}))
                .await?,
        ))
    }

    pub async fn library_albums(&self) -> Result<Vec<Item>> {
        Ok(parse_library_playlists(
            &self
                .post("browse", json!({"browseId": "FEmusic_liked_albums"}))
                .await?,
        ))
    }

    pub async fn library_artists(&self) -> Result<Vec<Item>> {
        Ok(parse_library_artists(
            &self
                .post(
                    "browse",
                    json!({"browseId": "FEmusic_library_corpus_artists"}),
                )
                .await?,
        ))
    }

    pub async fn history(&self) -> Result<Vec<Item>> {
        Ok(parse_history(
            &self
                .post("browse", json!({"browseId": "FEmusic_history"}))
                .await?,
        ))
    }
}
