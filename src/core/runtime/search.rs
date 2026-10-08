//! Controller: search and its continuation pages.
use super::*;
use std::sync::atomic::Ordering;

impl Controller {
    pub(super) fn search(&self, query: String, filter: Option<String>) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let sink = self.sink.clone();
        let seq = self.search_seq.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = self.search_seq.clone();
        let next = self.search_next.clone();
        *next.lock().unwrap() = None;
        tokio::spawn(async move {
            let res = client.search_page(&query, filter.as_deref()).await;
            match res {
                Ok(page) => {
                    let more = page.continuation.is_some();
                    {
                        // check and store under one lock: an old search cannot overwrite the
                        // token of a newer one
                        let mut g = next.lock().unwrap();
                        if latest.load(Ordering::SeqCst) != seq {
                            return; // a newer search superseded this one
                        }
                        *g = page.continuation.map(|t| (seq, t));
                    }
                    sink(Event::Results {
                        items: page.items,
                        more,
                    });
                }
                Err(e) => {
                    if latest.load(Ordering::SeqCst) == seq {
                        sink(Event::Error(format!("search failed: {e:#}")));
                    }
                }
            }
        });
    }

    /// The next page of the newest search (the token is taken, so a repeated request while a
    /// page is in flight does nothing).
    pub(super) fn search_more(&self) {
        let Some(client) = self.svc.client.clone() else {
            return;
        };
        let Some((seq, token)) = self.search_next.lock().unwrap().take() else {
            return;
        };
        let (sink, latest, next) = (
            self.sink.clone(),
            self.search_seq.clone(),
            self.search_next.clone(),
        );
        tokio::spawn(async move {
            let res = client.search_more(&token).await;
            match res {
                Ok(page) => {
                    let more = page.continuation.is_some() && !page.items.is_empty();
                    {
                        let mut g = next.lock().unwrap();
                        if latest.load(Ordering::SeqCst) != seq {
                            return; // the user searched again; that search resets the list
                        }
                        if more {
                            *g = page.continuation.map(|t| (seq, t));
                        }
                    }
                    sink(Event::ResultsMore {
                        items: page.items,
                        more,
                    });
                }
                Err(e) => {
                    if latest.load(Ordering::SeqCst) == seq {
                        sink(Event::ResultsMore {
                            items: Vec::new(),
                            more: false,
                        });
                        sink(Event::Error(format!("search failed: {e:#}")));
                    }
                }
            }
        });
    }
}
