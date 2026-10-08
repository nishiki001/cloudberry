//! Discover page glue: sections, shelf models, card actions, lazy covers.
mod actions;
pub(crate) use actions::play as play_card;
pub(crate) use actions::{card_action, is_track, request_cover};
mod render;
pub(crate) use render::shelf_rows;
use render::want_related;
pub use render::{arrived, patch_thumb, related_arrived, track_changed};

use super::state::*;
use crate::core::api::discover_parse::DiscoverData;
use crate::core::model::{Item, Kind};
use crate::core::msg::Command;
use crate::core::runtime::CoreHandle;
use crate::core::runtime::discover_key;
use crate::{AppWindow, CardData, Discover, MoodChip, ShelfData};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;

const COUNTRIES: [(&str, &str); 15] = [
    ("ZZ", "Global"),
    ("US", "United States"),
    ("JP", "Japan"),
    ("GB", "United Kingdom"),
    ("DE", "Germany"),
    ("FR", "France"),
    ("BR", "Brazil"),
    ("IN", "India"),
    ("KR", "South Korea"),
    ("MX", "Mexico"),
    ("CA", "Canada"),
    ("AU", "Australia"),
    ("ES", "Spain"),
    ("IT", "Italy"),
    ("RU", "Russia"),
];

/// What the Discover page currently shows.
#[derive(Default)]
pub struct DiscoverState {
    /// `discover_key` of the section on screen (events for other keys are stale).
    pub key: String,
    pub section: String,
    pub params: Option<String>,
    pub shelves: Vec<Vec<Item>>,
    models: Vec<Rc<VecModel<CardData>>>,
    mood_params: Vec<String>,
    /// Last content of the section on screen (re-rendered when related shelves arrive).
    last: DiscoverData,
    /// Related shelves of the playing track (shown at the bottom of Home).
    related: Option<(String, Vec<crate::core::api::discover_parse::Shelf>)>,
    /// The model behind `Discover.shelves` and how many of its rows are not "Related".
    shelves_model: Option<Rc<VecModel<ShelfData>>>,
    base_len: usize,
}

fn title_of(section: &str) -> &'static str {
    match section {
        "home" => "Home",
        "new" => "New releases",
        "charts" => "Charts",
        "moods" => "Moods & genres",
        _ => "",
    }
}

pub fn init(ui: &AppWindow) {
    let countries: Vec<(SharedString, SharedString)> = COUNTRIES
        .iter()
        .map(|(c, n)| ((*c).into(), (*n).into()))
        .collect();
    // the anonymous struct {code, name} is a tuple in field order
    ui.global::<Discover>()
        .set_countries(ModelRc::new(VecModel::from(countries)));
}

/// Open (or switch) a section: show the Discover tab and ask the core for the content.
pub fn choose(
    ui: &AppWindow,
    core: &CoreHandle,
    section: &str,
    params: Option<String>,
    force: bool,
) {
    let d = ui.global::<Discover>();
    let country = if section == "charts" {
        d.get_country().to_string()
    } else {
        "ZZ".to_string()
    };
    let key = discover_key(section, &country, params.as_deref());
    let title = match section {
        "mood" => d.get_title().to_string(),
        _ => title_of(section).to_string(),
    };
    let changed = STATE.with(|s| {
        let mut s = s.borrow_mut();
        let changed = s.discover.key != key;
        s.discover.key = key;
        s.discover.section = section.to_string();
        s.discover.params.clone_from(&params);
        changed
    });
    d.set_opened(true);
    d.set_active(true);
    d.set_section(section.into());
    d.set_title(title.into());
    d.set_can_back(section == "mood");
    d.set_busy(true);
    if changed {
        clear(ui);
    }
    if section == "home" {
        want_related(core);
    }
    core.send(Command::LoadDiscover {
        section: section.to_string(),
        country,
        params,
        force,
    });
}

fn clear(ui: &AppWindow) {
    let d = ui.global::<Discover>();
    d.set_shelves(ModelRc::new(VecModel::default()));
    d.set_moods(ModelRc::new(VecModel::default()));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.discover.shelves.clear();
        s.discover.models.clear();
    });
}

pub fn wire(ui: &AppWindow, core: &CoreHandle) {
    init(ui);
    let d = ui.global::<Discover>();
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_choose_section(move |id| {
        if let Some(ui) = weak.upgrade() {
            choose(&ui, &c, &id, None, false);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_back(move || {
        if let Some(ui) = weak.upgrade() {
            choose(&ui, &c, "moods", None, false);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_refresh(move || {
        if let Some(ui) = weak.upgrade() {
            let (section, params) = STATE.with(|s| {
                (
                    s.borrow().discover.section.clone(),
                    s.borrow().discover.params.clone(),
                )
            });
            if !section.is_empty() {
                choose(&ui, &c, &section, params, true);
            }
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_country_picked(move |code| {
        if let Some(ui) = weak.upgrade() {
            ui.global::<Discover>().set_country(code);
            choose(&ui, &c, "charts", None, false);
        }
    });
    let (weak, c) = (ui.as_weak(), core.clone());
    d.on_mood_clicked(move |i| {
        let (Some(ui), Some((params, title))) = (
            weak.upgrade(),
            STATE.with(|s| {
                let s = s.borrow();
                Some((
                    s.discover.mood_params.get(i as usize)?.clone(),
                    s.discover_moods.get(i as usize)?.clone(),
                ))
            }),
        ) else {
            return;
        };
        ui.global::<Discover>().set_title(title.into());
        choose(&ui, &c, "mood", Some(params), false);
    });
    actions::wire(ui, core);
}
