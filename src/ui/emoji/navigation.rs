use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use super::Ui;
use super::model::{next_emoji_row, row_emojis, row_has_emojis};

pub(in crate::ui) fn selected_text(ui: &Rc<RefCell<Ui>>) -> Option<String> {
    let state = ui.borrow();
    selected_emoji(&state).map(|emoji| emoji.to_string())
}

pub(in crate::ui) fn move_horizontal(ui: &Rc<RefCell<Ui>>, delta: isize) {
    let target = {
        let mut state = ui.borrow_mut();
        let Some(emojis) = row_emojis(&state, state.selected_row) else {
            return;
        };
        if delta < 0 {
            if state.selected_col > 0 {
                state.selected_col -= 1;
            } else if let Some(row) = next_emoji_row(&state, state.selected_row, false) {
                state.selected_row = row;
                state.selected_col =
                    row_emojis(&state, row).map_or(0, |e| e.len().saturating_sub(1));
            } else {
                return;
            }
        } else if state.selected_col + 1 < emojis.len() {
            state.selected_col += 1;
        } else if let Some(row) = next_emoji_row(&state, state.selected_row, true) {
            state.selected_row = row;
            state.selected_col = 0;
        } else {
            return;
        }
        update_visible_selection(&state);
        state.selected_row
    };
    ui.borrow()
        .list_view
        .scroll_to(target, gtk::ListScrollFlags::NONE, None);
    let state = ui.borrow();
    update_visible_selection(&state);
}

pub(in crate::ui) fn move_vertical(ui: &Rc<RefCell<Ui>>, forward: bool) -> bool {
    let target = {
        let mut state = ui.borrow_mut();
        let Some(row) = next_emoji_row(&state, state.selected_row, forward) else {
            return false;
        };
        let max = row_emojis(&state, row).map_or(0, |emojis| emojis.len().saturating_sub(1));
        state.selected_row = row;
        state.selected_col = state.selected_col.min(max);
        update_visible_selection(&state);
        row
    };
    ui.borrow()
        .list_view
        .scroll_to(target, gtk::ListScrollFlags::NONE, None);
    let state = ui.borrow();
    update_visible_selection(&state);
    true
}

pub(in crate::ui) fn select_edge(ui: &Rc<RefCell<Ui>>, end: bool) {
    let target = {
        let mut state = ui.borrow_mut();
        let row = if end {
            (0..state.model.n_items())
                .rev()
                .find(|position| row_has_emojis(&state, *position))
        } else {
            (0..state.model.n_items()).find(|position| row_has_emojis(&state, *position))
        };
        let Some(row) = row else {
            return;
        };
        let max = row_emojis(&state, row).map_or(0, |emojis| emojis.len().saturating_sub(1));
        state.selected_row = row;
        state.selected_col = if end { max } else { 0 };
        update_visible_selection(&state);
        row
    };
    ui.borrow()
        .list_view
        .scroll_to(target, gtk::ListScrollFlags::NONE, None);
    let state = ui.borrow();
    update_visible_selection(&state);
}

pub(in crate::ui) fn ensure_selection(ui: &Rc<RefCell<Ui>>) {
    let mut state = ui.borrow_mut();
    if row_has_emojis(&state, state.selected_row) {
        return;
    }
    if let Some(row) = (0..state.model.n_items()).find(|position| row_has_emojis(&state, *position))
    {
        state.selected_row = row;
        state.selected_col = 0;
    } else {
        state.selected_row = gtk::INVALID_LIST_POSITION;
        state.selected_col = 0;
    }
    update_visible_selection(&state);
}

pub(super) fn update_visible_selection(state: &Ui) {
    let selected = selected_emoji(state);
    visit_buttons(state.list_view.upcast_ref(), &mut |button| {
        set_button_selected(button, selected == button.label().as_deref());
    });
}

pub(super) fn apply_selection(buttons: &[gtk::Button], selected: Option<&str>) {
    for button in buttons {
        set_button_selected(button, selected == button.label().as_deref());
    }
}

fn set_button_selected(button: &gtk::Button, selected: bool) {
    if selected {
        button.add_css_class("selected");
    } else {
        button.remove_css_class("selected");
    }
    button.update_state(&[gtk::accessible::State::Selected(Some(selected))]);
}

pub(super) fn selected_emoji(state: &Ui) -> Option<&'static str> {
    let emojis = row_emojis(state, state.selected_row)?;
    emojis.get(state.selected_col).map(|emoji| emoji.as_str())
}

pub(super) fn emoji_buttons(emoji_row: &gtk::Box) -> Vec<gtk::Button> {
    let mut buttons = Vec::new();
    let mut child = emoji_row.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            buttons.push(button.clone());
        }
        child = widget.next_sibling();
    }
    buttons
}

fn visit_buttons(widget: &gtk::Widget, callback: &mut impl FnMut(&gtk::Button)) {
    let mut child = widget.first_child();
    while let Some(widget) = child {
        if let Some(button) = widget.downcast_ref::<gtk::Button>() {
            callback(button);
        }
        visit_buttons(&widget, callback);
        child = widget.next_sibling();
    }
}
