//! Headless `search`.
use crate::config;
use crate::core::api::client::Client;
use anyhow::Result;

pub fn run(query: &str, filter: Option<&str>, limit: Option<usize>, json: bool) -> Result<i32> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let items = rt.block_on(async {
        Client::new(Some(config::cookies_path()))?
            .search_limit(query, filter, limit.unwrap_or(0))
            .await
    })?;
    if json {
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        for i in &items {
            let artists: Vec<&str> = i.artists.iter().map(|a| a.name.as_str()).collect();
            println!(
                "{:8} {:12} {} — {} {}",
                format!("{:?}", i.kind).to_lowercase(),
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
