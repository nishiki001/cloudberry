//! Search responses: pages of results and the continuation token.
use super::nav::{nav, nav_str};
use super::parse::parse_list_item;
use crate::core::model::Item;
use serde_json::Value;

/// One page of search results and the token of the next page, if any.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct SearchPage {
    pub items: Vec<Item>,
    pub continuation: Option<String>,
}

fn shelf_items(shelf: &Value, out: &mut Vec<Item>) {
    for c in shelf
        .get("contents")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        match c
            .get("musicResponsiveListItemRenderer")
            .and_then(parse_list_item)
        {
            Some(i) => out.push(i),
            None => tracing::debug!("skipped unparseable search item"),
        }
    }
}

fn shelf_token(shelf: &Value) -> Option<String> {
    nav_str(
        shelf,
        &["continuations", "0", "nextContinuationData", "continuation"],
    )
    .map(String::from)
}

/// A first search response (every shelf) or a continuation response (one shelf).
pub fn parse_search_page(v: &Value) -> SearchPage {
    let mut page = SearchPage::default();
    if let Some(shelf) = nav(v, &["continuationContents", "musicShelfContinuation"]) {
        shelf_items(shelf, &mut page.items);
        page.continuation = shelf_token(shelf);
        return page;
    }
    let sections = nav(
        v,
        &[
            "contents",
            "tabbedSearchResultsRenderer",
            "tabs",
            "0",
            "tabRenderer",
            "content",
            "sectionListRenderer",
            "contents",
        ],
    )
    .and_then(Value::as_array);
    for sec in sections.into_iter().flatten() {
        let shelf = sec
            .get("musicShelfRenderer")
            .or_else(|| sec.get("musicCardShelfRenderer"));
        if let Some(shelf) = shelf {
            shelf_items(shelf, &mut page.items);
            if let Some(t) = shelf_token(shelf) {
                page.continuation = Some(t); // the main (last) shelf is the one that continues
            }
        }
    }
    page
}

/// All list items in a search response (every shelf).
pub fn parse_search(v: &Value) -> Vec<Item> {
    parse_search_page(v).items
}
