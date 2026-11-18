mod ime;
mod lock;
mod messages;
mod ui;

use gtk::prelude::*;

use crate::lock::acquire_lock;

fn main() {
    let _lock = match acquire_lock() {
        Ok(Some(lock)) => lock,
        Ok(None) => return,
        Err(err) => {
            eprintln!("waymoji: failed to acquire instance lock: {err}");
            std::process::exit(1);
        }
    };

    let app = gtk::Application::builder()
        .application_id("dev.olaren.Waymoji")
        .build();
    app.connect_activate(ui::start);
    app.run();
}
