//! Headless `lyrics <videoId>`.
use crate::config::{self, Config};
use crate::core::api::client::Client;
use crate::core::lyrics::{self, LyricsClient};
use anyhow::{Result, bail};

fn stamp(t: f64) -> String {
    format!("[{:02}:{:05.2}]", (t / 60.0) as u32, t % 60.0)
}

pub fn run(id: &str, json: bool) -> Result<i32> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(async {
        let client = Client::new(Some(config::cookies_path()))?;
        // the seed of a radio is the track itself: title/artist/album/duration for matching
        let Some(item) = client.radio(id).await?.into_iter().next() else {
            bail!("track not found")
        };
        let lc = LyricsClient::new(Config::load().lyrics_lrclib)?;
        match lyrics::fetch(&lc, Some(&client), &item).await? {
            None => {
                eprintln!(
                    "no lyrics found for {} — {}",
                    item.title,
                    item.artists.first().map(|a| a.name.as_str()).unwrap_or("")
                );
                Ok(1)
            }
            Some(l) => {
                if json {
                    println!("{}", serde_json::to_string_pretty(&l)?);
                } else {
                    eprintln!(
                        "# {} ({}, {})",
                        item.title,
                        l.source,
                        if l.synced { "synced" } else { "plain" }
                    );
                    for line in &l.lines {
                        if l.synced {
                            println!("{} {}", stamp(line.t), line.text)
                        } else {
                            println!("{}", line.text)
                        }
                    }
                }
                Ok(0)
            }
        }
    })
}
