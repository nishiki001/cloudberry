//! Discover pages (home feed, new releases, charts, moods & genres): shelves of cards or songs.
//! Parsers never panic; unknown shelf or item types are skipped (debug log).
use super::nav::{nav, nav_str};
use super::parse::parse_list_item;
use crate::core::model::{Item, Kind};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Cards per shelf kept (the moods pages carry 50–80).
const MAX_ITEMS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ShelfKind {
    /// Covers with a title and subtitle.
    Cards,
    /// Tracks (shown as a compact list).
    Songs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Shelf {
    pub title: String,
    pub kind: ShelfKind,
    pub items: Vec<Item>,
}

/// A "moods & genres" category; `params` opens its playlists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mood {
    pub title: String,
    pub params: String,
    pub group: String,
}

/// What a Discover section shows: shelves, or (moods) the category list.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DiscoverData {
    pub shelves: Vec<Shelf>,
    pub moods: Vec<Mood>,
}

/// One page of shelves plus the token of the next page (home feed).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub shelves: Vec<Shelf>,
    pub continuation: Option<String>,
}

const SECTION_LIST: [&str; 7] = [
    "contents",
    "singleColumnBrowseResultsRenderer",
    "tabs",
    "0",
    "tabRenderer",
    "content",
    "sectionListRenderer",
];

fn text(v: &Value, path: &[&str]) -> Option<String> {
    nav_str(v, path).map(|s| s.trim().to_string())
}

pub use super::discover_card::parse_card;

fn shelf_title(s: &Value) -> String {
    for h in ["header", "title"] {
        let Some(h) = s.get(h) else { continue };
        for path in [
            [
                "musicCarouselShelfBasicHeaderRenderer",
                "title",
                "runs",
                "0",
            ]
            .as_slice(),
            &[
                "musicCarouselShelfBasicHeaderRenderer",
                "strapline",
                "runs",
                "0",
            ],
            &["musicShelfBasicHeaderRenderer", "title", "runs", "0"],
            &["gridHeaderRenderer", "title", "runs", "0"],
        ] {
            let mut p = path.to_vec();
            p.push("text");
            if let Some(t) = text(h, &p).filter(|t| !t.is_empty()) {
                return t;
            }
        }
        if let Some(t) = text(h, &["runs", "0", "text"]) {
            return t;
        }
    }
    String::new()
}

fn parse_items(contents: &[Value]) -> Vec<Item> {
    contents
        .iter()
        .filter_map(|c| {
            if let Some(r) = c.get("musicTwoRowItemRenderer") {
                parse_card(r)
            } else if let Some(r) = c.get("musicResponsiveListItemRenderer") {
                parse_list_item(r)
            } else {
                tracing::debug!(
                    "discover: skipped item type {:?}",
                    c.as_object().and_then(|o| o.keys().next())
                );
                None
            }
        })
        .take(MAX_ITEMS)
        .collect()
}

/// Shelves of a section list (carousels, plain shelves, grids); other section types are skipped.
pub fn parse_shelves(sections: &[Value]) -> Vec<Shelf> {
    let mut out = Vec::new();
    for sec in sections {
        let Some((key, body)) = sec.as_object().and_then(|o| o.iter().next()) else {
            continue;
        };
        let contents = match key.as_str() {
            "musicCarouselShelfRenderer" | "musicShelfRenderer" => body.get("contents"),
            "gridRenderer" => body.get("items"),
            _ => {
                tracing::debug!("discover: skipped section {key}");
                None
            }
        };
        let Some(contents) = contents.and_then(Value::as_array) else {
            continue;
        };
        let items = parse_items(contents);
        if items.is_empty() {
            continue;
        }
        let songs = items
            .iter()
            .filter(|i| matches!(i.kind, Kind::Song | Kind::Video))
            .count();
        let from_list_rows = contents
            .first()
            .is_some_and(|c| c.get("musicResponsiveListItemRenderer").is_some());
        let kind = if from_list_rows && songs * 2 >= items.len() {
            ShelfKind::Songs
        } else {
            ShelfKind::Cards
        };
        let title = shelf_title(body);
        out.push(Shelf { title, kind, items });
    }
    out
}

fn continuation_token(section_list: &Value) -> Option<String> {
    nav_str(
        section_list,
        &["continuations", "0", "nextContinuationData", "continuation"],
    )
    .map(String::from)
}

/// A first page (`browse` response) or a continuation page.
pub fn parse_page(v: &Value) -> Page {
    let list = nav(v, &SECTION_LIST)
        .or_else(|| nav(v, &["continuationContents", "sectionListContinuation"]))
        .or_else(|| nav(v, &["contents", "sectionListRenderer"]));
    let Some(list) = list else {
        return Page::default();
    };
    Page {
        shelves: list
            .get("contents")
            .and_then(Value::as_array)
            .map(|c| parse_shelves(c))
            .unwrap_or_default(),
        continuation: continuation_token(list),
    }
}

/// Moods & genres categories (grids of navigation buttons), with the group heading.
pub fn parse_moods(v: &Value) -> Vec<Mood> {
    let mut out = Vec::new();
    let sections = nav(v, &SECTION_LIST)
        .and_then(|l| l.get("contents"))
        .and_then(Value::as_array);
    for sec in sections.into_iter().flatten() {
        let Some(grid) = sec.get("gridRenderer") else {
            continue;
        };
        let group = text(
            grid,
            &["header", "gridHeaderRenderer", "title", "runs", "0", "text"],
        )
        .unwrap_or_default();
        for item in grid
            .get("items")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(b) = item.get("musicNavigationButtonRenderer") else {
                continue;
            };
            let (Some(title), Some(params)) = (
                text(b, &["buttonText", "runs", "0", "text"]),
                text(b, &["clickCommand", "browseEndpoint", "params"]),
            ) else {
                continue;
            };
            out.push(Mood {
                title,
                params,
                group: group.clone(),
            });
        }
    }
    out
}
