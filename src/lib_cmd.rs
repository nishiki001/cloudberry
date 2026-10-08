//! Headless `liked` and `playlist`.
use crate::config;
use crate::core::api::client::Client;
use crate::core::model::Item;
use anyhow::{Result, bail};

fn print(items: &[Item], json: bool) -> Result<i32> {
    if json {
        println!("{}", serde_json::to_string_pretty(items)?);
    } else {
        for i in items {
            let artists: Vec<&str> = i.artists.iter().map(|a| a.name.as_str()).collect();
            println!(
                "{:12} {} — {} {}",
                i.video_id
                    .as_deref()
                    .or(i.browse_id.as_deref())
                    .unwrap_or("-"),
                i.title,
                artists.join(", "),
                i.duration.as_deref().unwrap_or("")
            );
        }
    }
    Ok(if items.is_empty() { 1 } else { 0 })
}

fn client() -> Result<Client> {
    Client::new(Some(config::cookies_path()))
}

fn rt() -> Result<tokio::runtime::Runtime> {
    Ok(tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?)
}

pub fn liked(json: bool) -> Result<i32> {
    let c = client()?;
    if !c.has_cookies() {
        bail!("not signed in: run `auth setup` first");
    }
    print(&rt()?.block_on(c.liked())?, json)
}

/// With an id: the playlist's tracks. Without: your library playlists (needs sign-in).
pub fn playlist(id: Option<&str>, json: bool) -> Result<i32> {
    let c = client()?;
    match id {
        Some(id) => print(
            &rt()?.block_on(c.playlist(id.strip_prefix("VL").unwrap_or(id)))?,
            json,
        ),
        None => {
            if !c.has_cookies() {
                bail!("not signed in: run `auth setup` first (or pass a playlist id)");
            }
            print(&rt()?.block_on(c.library_playlists())?, json)
        }
    }
}
