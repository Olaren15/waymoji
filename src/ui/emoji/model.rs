use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::category::{set_active_category, update_current_category};
use super::navigation::update_visible_selection;
use super::{COLUMNS, ROW_HEIGHT, Ui};

pub(in crate::ui) struct ListState {
    pub(super) query: String,
    pub(super) active_category: Option<emojis::Group>,
}

pub(super) struct RowData {
    pub(super) kind: RowKind,
}

pub(super) enum RowKind {
    Separator {
        category: emojis::Group,
    },
    Emojis {
        emojis: Vec<&'static emojis::Emoji>,
        category: emojis::Group,
    },
}

pub(in crate::ui) fn set_query(ui: &Rc<RefCell<Ui>>, query: &str) {
    {
        let state = ui.borrow();
        state.list_state.borrow_mut().query = query.to_lowercase();
    }
    rebuild_model(ui);
}

fn rebuild_model(ui: &Rc<RefCell<Ui>>) {
    let rows = {
        let state = ui.borrow();
        build_rows(&state.list_state.borrow().query)
    };
    let model = ui.borrow().model.clone();
    model.remove_all();
    for row in rows {
        model.append(&glib::BoxedAnyObject::new(row));
    }

    let target = {
        let mut state = ui.borrow_mut();
        let target = (0..state.model.n_items())
            .find(|position| row_has_emojis(&state, *position))
            .unwrap_or(gtk::INVALID_LIST_POSITION);
        state.selected_row = target;
        state.selected_col = 0;
        update_visible_selection(&state);
        target
    };

    if target != gtk::INVALID_LIST_POSITION {
        if let Some(category) = {
            let state = ui.borrow();
            row_category(&state, target)
        } {
            let state = ui.borrow();
            set_active_category(&state, Some(category));
        }
        ui.borrow()
            .list_view
            .scroll_to(target, gtk::ListScrollFlags::NONE, None);
    }
    update_current_category(ui);

    let ui = ui.clone();
    gtk::glib::idle_add_local_once(move || {
        update_current_category(&ui);
    });
}

pub(super) fn build_rows(query: &str) -> Vec<RowData> {
    if query.is_empty() {
        let mut rows = Vec::new();
        let groups: Vec<_> = emojis::Group::iter().collect();
        for (index, group) in groups.iter().enumerate() {
            if index > 0 {
                rows.push(RowData {
                    kind: RowKind::Separator { category: *group },
                });
            }
            let emojis: Vec<_> = group.emojis().collect();
            for chunk in emojis.chunks(COLUMNS) {
                rows.push(RowData {
                    kind: RowKind::Emojis {
                        emojis: chunk.to_vec(),
                        category: *group,
                    },
                });
            }
        }
        rows
    } else {
        emojis::iter()
            .filter(|emoji| {
                emoji.name().to_lowercase().contains(query)
                    || emoji
                        .shortcodes()
                        .any(|shortcode| shortcode.contains(query))
            })
            .collect::<Vec<_>>()
            .chunks(COLUMNS)
            .map(|chunk| RowData {
                kind: RowKind::Emojis {
                    emojis: chunk.to_vec(),
                    category: chunk[0].group(),
                },
            })
            .collect()
    }
}

pub(super) fn row_height(state: &Ui) -> f64 {
    let count = state.model.n_items();
    let upper = state.scrolled.vadjustment().upper();
    if count > 0 && upper > 0.0 {
        upper / count as f64
    } else {
        ROW_HEIGHT
    }
}

pub(super) fn row_category(state: &Ui, position: u32) -> Option<emojis::Group> {
    let item = state.model.item(position)?;
    let item = item.downcast::<glib::BoxedAnyObject>().ok()?;
    let row = item.borrow::<RowData>();
    Some(match &row.kind {
        RowKind::Separator { category } => *category,
        RowKind::Emojis { category, .. } => *category,
    })
}

pub(super) fn row_has_emojis(state: &Ui, position: u32) -> bool {
    let Some(item) = state.model.item(position) else {
        return false;
    };
    let Some(item) = item.downcast::<glib::BoxedAnyObject>().ok() else {
        return false;
    };
    let row = item.borrow::<RowData>();
    matches!(row.kind, RowKind::Emojis { .. })
}

pub(super) fn row_emojis(state: &Ui, position: u32) -> Option<Vec<&'static emojis::Emoji>> {
    let item = state.model.item(position)?;
    let item = item.downcast::<glib::BoxedAnyObject>().ok()?;
    let row = item.borrow::<RowData>();
    match &row.kind {
        RowKind::Emojis { emojis, .. } => Some(emojis.clone()),
        RowKind::Separator { .. } => None,
    }
}

pub(super) fn next_emoji_row(state: &Ui, from: u32, forward: bool) -> Option<u32> {
    let mut position = from as isize;
    loop {
        position += if forward { 1 } else { -1 };
        if position < 0 || position >= state.model.n_items() as isize {
            return None;
        }
        if row_has_emojis(state, position as u32) {
            return Some(position as u32);
        }
    }
}
