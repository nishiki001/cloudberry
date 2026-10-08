//! Headless `discover`.
use crate::config;
use crate::core::api::client::Client;
use anyhow::{Result, bail};

pub fn run(
    section: &str,
    country: &str,
    params: Option<&str>,
    raw: bool,
    json: bool,
) -> Result<i32> {
    let c = Client::new(Some(config::cookies_path()))?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    if raw {
        let v = rt.block_on(c.discover_raw(section, country, params))?;
        println!("{}", serde_json::to_string(&v)?);
        return Ok(0);
    }
    if section == "related" {
        let id = params.ok_or_else(|| anyhow::anyhow!("--params <videoId> is required"))?;
        let shelves = rt.block_on(c.related(id))?;
        println!("{}", serde_json::to_string_pretty(&shelves)?);
        return Ok(if shelves.is_empty() { 1 } else { 0 });
    }
    if section == "moods" {
        let moods = rt.block_on(c.discover_moods())?;
        if json {
            println!("{}", serde_json::to_string_pretty(&moods)?);
        } else {
            for m in &moods {
                println!("{:12} {} ({})", m.group, m.title, m.params);
            }
        }
        return Ok(if moods.is_empty() { 1 } else { 0 });
    }
    if !matches!(section, "home" | "new" | "charts" | "mood") {
        bail!("section must be home, new, charts, moods, mood or related");
    }
    let page = rt.block_on(c.discover(section, country, params, 3))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&page.shelves)?);
    } else {
        for s in &page.shelves {
            println!("== {} ({:?}, {} items)", s.title, s.kind, s.items.len());
            for i in s.items.iter().take(5) {
                println!("   {:?} {} — {}", i.kind, i.title, i.subtitle);
            }
        }
    }
    Ok(if page.shelves.is_empty() { 1 } else { 0 })
}

/// `raw <endpoint> <json>`: debugging aid, prints the response of an authenticated request.
pub fn raw(endpoint: &str, body: &str) -> Result<i32> {
    let c = Client::new(Some(config::cookies_path()))?;
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let v = rt.block_on(c.post(endpoint, serde_json::from_str(body)?))?;
    println!("{}", serde_json::to_string(&v)?);
    Ok(0)
}
