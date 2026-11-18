use std::cell::RefCell;
use std::rc::Rc;

use gtk::glib;
use gtk::prelude::*;

use super::category::group_name;
use super::model::{RowData, RowKind};
use super::navigation::{apply_selection, emoji_buttons, selected_emoji, update_visible_selection};
use super::{COLUMNS, ROW_HEIGHT, Ui};

pub(in crate::ui) fn install_factory(ui: &Rc<RefCell<Ui>>) {
    let weak = Rc::downgrade(ui);
    let factory = gtk::SignalListItemFactory::new();

    {
        let weak = weak.clone();
        factory.connect_setup(move |_, object| {
            let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };

            let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
            root.set_size_request(-1, ROW_HEIGHT as i32);
            root.add_css_class("waymoji-row");

            let separator = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            separator.set_valign(gtk::Align::Center);
            separator.add_css_class("waymoji-separator-row");
            let label = gtk::Label::new(None);
            label.set_xalign(0.0);
            label.add_css_class("waymoji-separator-label");
            separator.append(&label);
            let line = gtk::Separator::new(gtk::Orientation::Horizontal);
            line.set_hexpand(true);
            line.set_valign(gtk::Align::Center);
            line.add_css_class("waymoji-separator-line");
            separator.append(&line);
            root.append(&separator);

            let emoji_row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .homogeneous(true)
                .build();
            emoji_row.set_hexpand(true);
            emoji_row.set_valign(gtk::Align::Center);
            emoji_row.add_css_class("waymoji-emoji-row");

            for index in 0..COLUMNS {
                let button = gtk::Button::new();
                button.set_focus_on_click(false);
                button.set_hexpand(true);
                button.add_css_class("waymoji-emoji");

                let list_item_ref = glib::WeakRef::new();
                list_item_ref.set(Some(list_item));
                let weak = weak.clone();
                button.connect_clicked(move |_| {
                    let Some(list_item) = list_item_ref.upgrade() else {
                        return;
                    };
                    let Some(item) = list_item.item().and_downcast::<glib::BoxedAnyObject>() else {
                        return;
                    };
                    let row = item.borrow::<RowData>();
                    let RowKind::Emojis { emojis, .. } = &row.kind else {
                        return;
                    };
                    let Some(emoji) = emojis.get(index) else {
                        return;
                    };
                    let text = emoji.as_str().to_string();
                    let position = list_item.position();
                    drop(row);
                    let Some(ui) = weak.upgrade() else {
                        return;
                    };
                    {
                        let mut state = ui.borrow_mut();
                        state.selected_row = position;
                        state.selected_col = index;
                        update_visible_selection(&state);
                    }
                    super::super::request_insert(&ui, &text);
                });

                emoji_row.append(&button);
            }

            root.append(&emoji_row);
            list_item.set_child(Some(&root));
        });
    }

    {
        let weak = weak.clone();
        factory.connect_bind(move |_, object| {
            let Some(list_item) = object.downcast_ref::<gtk::ListItem>() else {
                return;
            };
            let Some(root) = list_item.child().and_downcast::<gtk::Box>() else {
                return;
            };
            let Some(separator) = root.first_child().and_downcast::<gtk::Box>() else {
                return;
            };
            let Some(label) = separator.first_child().and_downcast::<gtk::Label>() else {
                return;
            };
            let Some(emoji_row) = separator.next_sibling().and_downcast::<gtk::Box>() else {
                return;
            };
            let Some(item) = list_item.item().and_downcast::<glib::BoxedAnyObject>() else {
                return;
            };
            let row = item.borrow::<RowData>();

            let buttons = emoji_buttons(&emoji_row);
            match &row.kind {
                RowKind::Separator { category } => {
                    separator.set_visible(true);
                    emoji_row.set_visible(false);
                    label.set_text(group_name(*category));
                }
                RowKind::Emojis { emojis, .. } => {
                    separator.set_visible(false);
                    emoji_row.set_visible(true);
                    for (index, button) in buttons.iter().enumerate() {
                        if let Some(emoji) = emojis.get(index) {
                            button.set_label(emoji.as_str());
                            button.set_tooltip_text(Some(emoji.name()));
                            button
                                .update_property(&[gtk::accessible::Property::Label(emoji.name())]);
                            button.set_opacity(1.0);
                            button.set_can_target(true);
                        } else {
                            button.set_label("");
                            button.set_tooltip_text(None);
                            button.update_property(&[gtk::accessible::Property::Label("")]);
                            button.set_opacity(0.0);
                            button.set_can_target(false);
                        }
                    }
                }
            }

            if let Some(ui) = weak.upgrade() {
                let state = ui.borrow();
                let selected = selected_emoji(&state);
                apply_selection(&buttons, selected);
            }
        });
    }

    ui.borrow().list_view.set_factory(Some(&factory));
}
