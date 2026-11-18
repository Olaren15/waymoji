use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

use super::Ui;
use super::emoji;
use crate::messages::{KeyAction, UiCommand};

pub(super) fn apply_key(ui: &Rc<RefCell<Ui>>, action: KeyAction) -> Option<UiCommand> {
    if matches!(action, KeyAction::Escape) {
        return Some(UiCommand::Cancel);
    }
    if ui.borrow().pending {
        return None;
    }

    match action {
        KeyAction::Enter => {
            return emoji::selected_text(ui).map(UiCommand::Insert);
        }
        KeyAction::ToggleList => {
            let focused = ui.borrow().list_focused;
            set_list_focus(ui, !focused);
        }
        KeyAction::NextCategory => emoji::cycle_category(ui, true),
        KeyAction::PreviousCategory => emoji::cycle_category(ui, false),
        KeyAction::Up => {
            if !ui.borrow().list_focused {
                set_list_focus(ui, true);
            } else if !emoji::move_vertical(ui, false) {
                set_list_focus(ui, false);
            }
        }
        KeyAction::Down => {
            if !ui.borrow().list_focused {
                set_list_focus(ui, true);
            } else {
                emoji::move_vertical(ui, true);
            }
        }
        KeyAction::MoveLeft { extend } => {
            if ui.borrow().list_focused {
                emoji::move_horizontal(ui, -1);
            } else {
                let search = leave_grid_and_search(ui);
                search.emit_move_cursor(gtk::MovementStep::VisualPositions, -1, extend);
            }
        }
        KeyAction::MoveRight { extend } => {
            if ui.borrow().list_focused {
                emoji::move_horizontal(ui, 1);
            } else {
                let search = leave_grid_and_search(ui);
                search.emit_move_cursor(gtk::MovementStep::VisualPositions, 1, extend);
            }
        }
        KeyAction::MoveWordLeft { extend } => {
            let search = leave_grid_and_search(ui);
            search.emit_move_cursor(gtk::MovementStep::Words, -1, extend);
        }
        KeyAction::MoveWordRight { extend } => {
            let search = leave_grid_and_search(ui);
            search.emit_move_cursor(gtk::MovementStep::Words, 1, extend);
        }
        KeyAction::MoveHome { extend } => {
            if ui.borrow().list_focused {
                emoji::select_edge(ui, false);
            } else {
                let search = leave_grid_and_search(ui);
                search.emit_move_cursor(gtk::MovementStep::DisplayLineEnds, -1, extend);
            }
        }
        KeyAction::MoveEnd { extend } => {
            if ui.borrow().list_focused {
                emoji::select_edge(ui, true);
            } else {
                let search = leave_grid_and_search(ui);
                search.emit_move_cursor(gtk::MovementStep::DisplayLineEnds, 1, extend);
            }
        }
        KeyAction::Char(text) => {
            let search = leave_grid_and_search(ui);
            if let Some((start, end)) = search.selection_bounds() {
                search.delete_text(start, end);
                search.set_position(start);
            }
            search.emit_insert_at_cursor(text.as_str());
        }
        KeyAction::DeleteBackward => {
            leave_grid_and_search(ui).emit_backspace();
        }
        KeyAction::DeleteForward => {
            leave_grid_and_search(ui).emit_delete_from_cursor(gtk::DeleteType::Chars, 1);
        }
        KeyAction::DeleteWordBackward => {
            leave_grid_and_search(ui).emit_delete_from_cursor(gtk::DeleteType::WordEnds, -1);
        }
        KeyAction::DeleteWordForward => {
            leave_grid_and_search(ui).emit_delete_from_cursor(gtk::DeleteType::WordEnds, 1);
        }
        KeyAction::DeleteToStart => {
            leave_grid_and_search(ui).emit_delete_from_cursor(gtk::DeleteType::ParagraphEnds, -1);
        }
        KeyAction::DeleteToEnd => {
            leave_grid_and_search(ui).emit_delete_from_cursor(gtk::DeleteType::ParagraphEnds, 1);
        }
        KeyAction::SelectAll => {
            let search = leave_grid_and_search(ui);
            let end = search.text_length() as i32;
            search.select_region(0, end);
        }
        KeyAction::Copy => leave_grid_and_search(ui).emit_copy_clipboard(),
        KeyAction::Cut => leave_grid_and_search(ui).emit_cut_clipboard(),
        KeyAction::Paste => leave_grid_and_search(ui).emit_paste_clipboard(),
        KeyAction::Undo => {
            let search = leave_grid_and_search(ui);
            let _ = search.activate_action("text.undo", None);
        }
        KeyAction::Redo => {
            let search = leave_grid_and_search(ui);
            let _ = search.activate_action("text.redo", None);
        }
        KeyAction::Escape => unreachable!(),
    }

    None
}

fn leave_grid_and_search(ui: &Rc<RefCell<Ui>>) -> gtk::Text {
    if ui.borrow().list_focused {
        let mut state = ui.borrow_mut();
        state.list_focused = false;
        state.search_box.remove_css_class("list-focus");
        state.search_box.add_css_class("search-focus");
        state.search.grab_focus_without_selecting();
    }
    update_caret(ui);
    ui.borrow().search.clone()
}

pub(super) fn set_list_focus(ui: &Rc<RefCell<Ui>>, focused: bool) {
    let mut state = ui.borrow_mut();
    state.list_focused = focused;
    if focused {
        state.search_box.remove_css_class("search-focus");
        state.search_box.add_css_class("list-focus");
    } else {
        state.search_box.remove_css_class("list-focus");
        state.search_box.add_css_class("search-focus");
        state.search.grab_focus();
        let end = state.search.text_length() as i32;
        state.search.select_region(0, end);
    }
    drop(state);
    if focused {
        emoji::ensure_selection(ui);
    }
    update_caret(ui);
}

pub(super) fn schedule_caret(ui: &Rc<RefCell<Ui>>) {
    let ui = ui.clone();
    gtk::glib::idle_add_local_once(move || {
        update_caret(&ui);
    });
}

pub(super) fn update_caret(ui: &Rc<RefCell<Ui>>) {
    let state = ui.borrow();
    let has_selection = state
        .search
        .selection_bounds()
        .is_some_and(|(start, end)| start != end);

    if state.list_focused || has_selection {
        state.caret.set_visible(false);
        return;
    }

    let text = state.search.text();
    let position = state.search.position().max(0) as usize;
    let prefix: String = text.chars().take(position).collect();
    let layout = state.search.create_pango_layout(Some(&prefix));
    let (width, height) = layout.pixel_size();
    let height = height.max(16).min(state.search.height().max(16));
    let max_x = state.search.width().saturating_sub(1).max(0);

    state.caret.set_margin_start(width.min(max_x));
    state.caret.set_size_request(1, height);
    state.caret.set_visible(true);
}
