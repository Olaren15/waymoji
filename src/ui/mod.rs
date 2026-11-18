use std::cell::RefCell;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use gtk::gio;
use gtk::prelude::*;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::ime;
use crate::messages::{UiCommand, UiEvent};

mod emoji;
mod input;
mod style;

const MENU_WIDTH: i32 = 420;

#[derive(Clone)]
struct CommandSender {
    tx: Sender<UiCommand>,
    wake: Arc<UnixStream>,
}

impl CommandSender {
    fn send(&self, command: UiCommand) {
        let _ = self.tx.send(command);
        let mut wake = &*self.wake;
        let _ = wake.write(&[1]);
    }
}

pub(super) struct Ui {
    search: gtk::Text,
    search_box: gtk::Box,
    caret: gtk::Box,
    footer: gtk::Label,
    model: gio::ListStore,
    list_view: gtk::ListView,
    scrolled: gtk::ScrolledWindow,
    selected_row: u32,
    selected_col: usize,
    list_state: Rc<RefCell<emoji::ListState>>,
    category_buttons: Vec<(emojis::Group, gtk::Button)>,
    list_focused: bool,
    pending: bool,
    grabbed: bool,
    active: bool,
    cmd_tx: CommandSender,
}

pub fn start(app: &gtk::Application) {
    let window = gtk::Window::builder().decorated(false).build();
    window.set_application(Some(app));
    window.add_css_class("waymoji-window");
    window.init_layer_shell();
    window.set_namespace(Some("waymoji"));
    window.set_layer(Layer::Overlay);
    window.set_keyboard_mode(KeyboardMode::None);
    window.set_anchor(Edge::Top, true);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, true);

    let background = gtk::DrawingArea::new();
    background.set_hexpand(true);
    background.set_vexpand(true);
    background.add_css_class("waymoji-background");

    let menu = gtk::Box::new(gtk::Orientation::Vertical, 8);
    menu.add_css_class("waymoji-menu");
    menu.set_size_request(MENU_WIDTH, -1);
    menu.set_halign(gtk::Align::Center);
    menu.set_valign(gtk::Align::Center);
    menu.set_vexpand(false);

    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);

    let search_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    search_box.set_hexpand(true);
    search_box.add_css_class("waymoji-search-box");
    search_box.add_css_class("search-focus");

    let search = gtk::Text::new();
    search.set_hexpand(true);
    search.set_placeholder_text(Some("Type to search…"));
    search.set_enable_undo(true);
    search.add_css_class("waymoji-search");

    let search_overlay = gtk::Overlay::new();
    search_overlay.set_hexpand(true);
    search_overlay.set_child(Some(&search));

    let caret = gtk::Box::new(gtk::Orientation::Vertical, 0);
    caret.set_size_request(1, 18);
    caret.set_halign(gtk::Align::Start);
    caret.set_valign(gtk::Align::Center);
    caret.set_can_target(false);
    caret.add_css_class("waymoji-caret");
    search_overlay.add_overlay(&caret);

    search_box.append(&search_overlay);
    header.append(&search_box);

    let close = gtk::Button::with_label("×");
    close.set_focus_on_click(false);
    close.add_css_class("flat");
    close.add_css_class("waymoji-close");
    close.set_tooltip_text(Some("Close"));
    header.append(&close);
    content.append(&header);

    let emoji_list = emoji::build();
    let (category_rail, category_buttons) = emoji::category_rail();

    let scrolled = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vscrollbar_policy(gtk::PolicyType::Automatic)
        .propagate_natural_height(false)
        .propagate_natural_width(false)
        .child(&emoji_list.view)
        .build();
    scrolled.set_size_request(-1, 360);
    scrolled.set_vexpand(false);
    scrolled.set_hexpand(true);

    let body = gtk::Box::new(gtk::Orientation::Horizontal, 4);
    body.set_hexpand(true);
    body.append(&scrolled);
    body.append(&category_rail);
    content.append(&body);

    let footer = gtk::Label::new(Some("Starting…"));
    footer.set_xalign(0.0);
    footer.add_css_class("waymoji-footer");
    content.append(&footer);
    menu.append(&content);

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&background));
    overlay.add_overlay(&menu);
    window.set_child(Some(&overlay));

    let (event_tx, event_rx) = async_channel::unbounded();
    let (raw_cmd_tx, cmd_rx) = mpsc::channel();
    let Ok((wake_rx, wake_tx)) = UnixStream::pair() else {
        eprintln!("failed to create IME wake pipe");
        return;
    };
    let _ = wake_rx.set_nonblocking(true);
    let _ = wake_tx.set_nonblocking(true);
    let cmd_tx = CommandSender {
        tx: raw_cmd_tx,
        wake: Arc::new(wake_tx),
    };

    let ui = Rc::new(RefCell::new(Ui {
        search: search.clone(),
        search_box: search_box.clone(),
        caret: caret.clone(),
        footer: footer.clone(),
        model: emoji_list.model.clone(),
        list_view: emoji_list.view.clone(),
        scrolled: scrolled.clone(),
        selected_row: 0,
        selected_col: 0,
        list_state: emoji_list.state.clone(),
        category_buttons: category_buttons.clone(),
        list_focused: false,
        pending: false,
        grabbed: false,
        active: false,
        cmd_tx: cmd_tx.clone(),
    }));

    style::install_css();
    emoji::watch_scroll(&ui);

    {
        for (category, button) in category_buttons {
            let ui = ui.clone();
            button.connect_clicked(move |_| {
                emoji::jump_to_category(&ui, category);
            });
        }
    }

    emoji::install_factory(&ui);

    {
        let ui_changed = ui.clone();
        search.connect_changed(move |_| {
            {
                let mut state = ui_changed.borrow_mut();
                state.list_focused = false;
                state.search_box.remove_css_class("list-focus");
                state.search_box.add_css_class("search-focus");
            }
            let query = ui_changed.borrow().search.text().to_string();
            emoji::set_query(&ui_changed, &query);
        });
    }

    {
        let ui_caret = ui.clone();
        search.connect_notify_local(Some("cursor-position"), move |_, _| {
            input::schedule_caret(&ui_caret);
        });
        let ui_caret = ui.clone();
        search.connect_notify_local(Some("selection-bound"), move |_, _| {
            input::schedule_caret(&ui_caret);
        });
    }

    {
        let ui_click = ui.clone();
        let click = gtk::GestureClick::new();
        click.set_propagation_phase(gtk::PropagationPhase::Capture);
        click.connect_pressed(move |_, _, _, _| {
            if ui_click.borrow().list_focused {
                input::set_list_focus(&ui_click, false);
            }
        });
        search.add_controller(click);
    }

    {
        let app = app.clone();
        let cmd_tx = cmd_tx.clone();
        close.connect_clicked(move |_| {
            cmd_tx.send(UiCommand::Cancel);
            app.quit();
        });
    }

    {
        let app = app.clone();
        let cmd_tx = cmd_tx.clone();
        let click = gtk::GestureClick::new();
        click.connect_pressed(move |_, _, _, _| {
            cmd_tx.send(UiCommand::Cancel);
            app.quit();
        });
        background.add_controller(click);
    }

    thread::spawn(move || {
        ime::run(event_tx, cmd_rx, wake_rx);
    });

    window.present();

    {
        let search = search.clone();
        let ui_caret = ui.clone();
        gtk::glib::idle_add_local_once(move || {
            search.grab_focus_without_selecting();
            input::update_caret(&ui_caret);
            emoji::update_current_category(&ui_caret);
        });
    }

    {
        let app = app.clone();
        let ui = ui.clone();
        gtk::glib::spawn_future_local(async move {
            while let Ok(event) = event_rx.recv().await {
                let mut quit = false;
                match event {
                    UiEvent::Key(action) => {
                        if let Some(command) = input::apply_key(&ui, action) {
                            match command {
                                UiCommand::Insert(text) => request_insert(&ui, &text),
                                UiCommand::Cancel => {
                                    ui.borrow().cmd_tx.send(UiCommand::Cancel);
                                    quit = true;
                                }
                            }
                        }
                    }
                    UiEvent::Inserted => {
                        let active = {
                            let mut state = ui.borrow_mut();
                            state.pending = false;
                            state.active
                        };
                        if ui.borrow().grabbed {
                            set_mode_footer(&ui, active);
                        }
                    }
                    UiEvent::Error(err) => {
                        ui.borrow_mut().pending = false;
                        show_error(&ui, &err);
                    }
                    UiEvent::Ready { active, grabbed } => {
                        {
                            let mut state = ui.borrow_mut();
                            state.grabbed = grabbed;
                            state.active = active;
                        }
                        if grabbed {
                            set_mode_footer(&ui, active);
                        } else {
                            show_error(&ui, "keyboard grab unavailable");
                        }
                    }
                    UiEvent::Mode { active } => {
                        ui.borrow_mut().active = active;
                        if ui.borrow().grabbed {
                            set_mode_footer(&ui, active);
                        }
                    }
                }

                if quit {
                    app.quit();
                    return;
                }
            }

            ui.borrow_mut().pending = false;
            show_error(&ui, "input method connection closed");
            gtk::glib::timeout_add_local_once(Duration::from_secs(2), move || {
                app.quit();
            });
        });
    }
}

pub(super) fn request_insert(ui: &Rc<RefCell<Ui>>, text: &str) {
    let mut ui = ui.borrow_mut();
    if ui.pending {
        return;
    }
    ui.pending = true;
    ui.footer.set_text("Inserting…");
    ui.cmd_tx.send(UiCommand::Insert(text.to_string()));
}

fn set_mode_footer(ui: &Rc<RefCell<Ui>>, active: bool) {
    let ui = ui.borrow();
    let text = if active {
        "IME active · Enter to insert · Esc to close"
    } else {
        "Keyboard mode · Enter to insert · Esc to close"
    };
    ui.footer.set_text(text);
}

fn show_error(ui: &Rc<RefCell<Ui>>, message: &str) {
    let ui = ui.borrow();
    ui.footer.set_text(message);
}
