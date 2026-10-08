#![allow(dead_code)] // parsers land in M3
//! InnerTube (WEB_REMIX) client. Modeled on ytmusicapi.
pub mod artist_info;
pub mod browse;
pub mod client;
pub mod discover_card;
pub mod discover_parse;
pub mod discover_raw;
#[cfg(test)]
mod discover_tests;
pub mod library;
mod library_client;
pub mod nav;
pub mod parse;
pub mod parse_search;
pub mod playlist_edit;
pub mod search;
pub mod watch;
