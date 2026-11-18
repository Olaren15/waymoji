mod category;
mod model;
mod navigation;
mod view;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

use super::Ui;

pub(super) use category::{
    category_rail, cycle_category, jump_to_category, update_current_category, watch_scroll,
};
pub(super) use model::{ListState, set_query};
pub(super) use navigation::{
    ensure_selection, move_horizontal, move_vertical, select_edge, selected_text,
};
pub(super) use view::install_factory;

pub(super) const COLUMNS: usize = 8;
const ROW_HEIGHT: f64 = 44.0;

pub(super) struct EmojiList {
    pub(super) model: gio::ListStore,
    pub(super) view: gtk::ListView,
    pub(super) state: Rc<RefCell<ListState>>,
}

pub(super) fn build() -> EmojiList {
    let state = Rc::new(RefCell::new(ListState {
        query: String::new(),
        active_category: Some(emojis::Group::SmileysAndEmotion),
    }));

    let model = gio::ListStore::new::<glib::BoxedAnyObject>();
    for row in model::build_rows("") {
        model.append(&glib::BoxedAnyObject::new(row));
    }

    let selection = gtk::NoSelection::new(Some(model.clone()));
    let view = gtk::ListView::new(Some(selection), None::<gtk::SignalListItemFactory>);
    view.set_can_focus(false);
    view.set_vexpand(true);
    view.add_css_class("waymoji-listview");

    EmojiList { model, view, state }
}
