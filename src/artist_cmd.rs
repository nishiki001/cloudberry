//! Headless `artist`.
use crate::config;
use crate::core::api::client::Client;
use anyhow::{Result, bail};

pub fn run(target: &str, json: bool) -> Result<i32> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let c = Client::new(Some(config::cookies_path()))?;
    let info = rt.block_on(async {
        // a channel id is used as is; anything else is looked up by name
        let id = if target.starts_with("UC") && target.len() == 24 {
            target.to_string()
        } else {
            let found = c.search_page(target, Some("artists")).await?;
            match found.items.first().and_then(|i| i.browse_id.clone()) {
                Some(id) => id,
                None => bail!("no artist found for \"{target}\""),
            }
        };
        c.artist_info(&id).await
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
    } else {
        println!(
            "{} — {}",
            info.name,
            info.subscribers.as_deref().unwrap_or("")
        );
        println!("{}", info.description.chars().take(200).collect::<String>());
        for (s, p) in info.songs.iter().zip(&info.plays) {
            println!("  {}  {}", s.title, p);
        }
        for sh in &info.shelves {
            println!("== {} ({} items)", sh.title, sh.items.len());
        }
    }
    Ok(if info.name.is_empty() { 1 } else { 0 })
}
