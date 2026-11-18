use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use super::Ui;
use super::model::{row_category, row_emojis, row_has_emojis, row_height};
use super::navigation::update_visible_selection;

const CATEGORY_OPTIONS: [(emojis::Group, &str, &str); 9] = [
    (emojis::Group::SmileysAndEmotion, "😀", "Smileys & Emotion"),
    (emojis::Group::PeopleAndBody, "🧑", "People & Body"),
    (emojis::Group::AnimalsAndNature, "🐻", "Animals & Nature"),
    (emojis::Group::FoodAndDrink, "🍔", "Food & Drink"),
    (emojis::Group::TravelAndPlaces, "✈️", "Travel & Places"),
    (emojis::Group::Activities, "🎮", "Activities"),
    (emojis::Group::Objects, "💡", "Objects"),
    (emojis::Group::Symbols, "❤️", "Symbols"),
    (emojis::Group::Flags, "🚩", "Flags"),
];

pub(in crate::ui) fn category_rail() -> (gtk::Box, Vec<(emojis::Group, gtk::Button)>) {
    let rail = gtk::Box::new(gtk::Orientation::Vertical, 2);
    rail.set_valign(gtk::Align::Fill);
    rail.set_width_request(30);
    rail.add_css_class("waymoji-category-rail");

    let mut buttons = Vec::new();
    for (index, (category, label, tooltip)) in CATEGORY_OPTIONS.iter().enumerate() {
        let button = gtk::Button::with_label(label);
        button.set_focus_on_click(false);
        button.set_vexpand(true);
        button.add_css_class("waymoji-category");
        button.set_tooltip_text(Some(tooltip));
        button.update_property(&[gtk::accessible::Property::Label(group_name(*category))]);
        if index == 0 {
            button.add_css_class("active");
        }
        rail.append(&button);
        buttons.push((*category, button));
    }

    (rail, buttons)
}

pub(in crate::ui) fn jump_to_category(ui: &Rc<RefCell<Ui>>, category: emojis::Group) {
    let target = {
        let state = ui.borrow();
        set_active_category(&state, Some(category));
        (0..state.model.n_items()).find(|position| {
            row_category(&state, *position).is_some_and(|group| group == category)
                && row_has_emojis(&state, *position)
        })
    };
    let Some(target) = target else {
        return;
    };

    let height = {
        let mut state = ui.borrow_mut();
        if row_emojis(&state, target).is_none() {
            return;
        }
        state.selected_row = target;
        state.selected_col = 0;
        update_visible_selection(&state);
        row_height(&state)
    };
    ui.borrow()
        .scrolled
        .vadjustment()
        .set_value(target as f64 * height);
}

pub(in crate::ui) fn cycle_category(ui: &Rc<RefCell<Ui>>, forward: bool) {
    let next = {
        let state = ui.borrow();
        let current = state.list_state.borrow().active_category;
        let index = CATEGORY_OPTIONS
            .iter()
            .position(|(category, _, _)| Some(*category) == current)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % CATEGORY_OPTIONS.len()
        } else {
            (index + CATEGORY_OPTIONS.len() - 1) % CATEGORY_OPTIONS.len()
        };
        CATEGORY_OPTIONS[next].0
    };
    jump_to_category(ui, next);
}

pub(in crate::ui) fn watch_scroll(ui: &Rc<RefCell<Ui>>) {
    let adjustment = ui.borrow().scrolled.vadjustment();
    let ui = ui.clone();
    adjustment.connect_value_changed(move |_| {
        update_current_category(&ui);
    });
}

pub(in crate::ui) fn update_current_category(ui: &Rc<RefCell<Ui>>) {
    let Ok(state) = ui.try_borrow() else {
        let ui = ui.clone();
        gtk::glib::idle_add_local_once(move || update_current_category(&ui));
        return;
    };
    let count = state.model.n_items();
    if count == 0 {
        set_active_category(&state, None);
        return;
    }
    let value = state.scrolled.vadjustment().value();
    let height = row_height(&state);
    let row = ((value / height).floor() as i64).clamp(0, count as i64 - 1) as u32;
    if let Some(category) = row_category(&state, row) {
        set_active_category(&state, Some(category));
    }
}

pub(super) fn set_active_category(state: &Ui, category: Option<emojis::Group>) {
    state.list_state.borrow_mut().active_category = category;
    for (button_category, button) in &state.category_buttons {
        let selected = category == Some(*button_category);
        if selected {
            button.add_css_class("active");
        } else {
            button.remove_css_class("active");
        }
        button.update_state(&[gtk::accessible::State::Selected(Some(selected))]);
    }
}

pub(super) fn group_name(group: emojis::Group) -> &'static str {
    CATEGORY_OPTIONS
        .iter()
        .find(|(candidate, _, _)| *candidate == group)
        .map_or("Emoji", |(_, _, name)| *name)
}
