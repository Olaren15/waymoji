pub(super) fn install_css() {
    let Some(display) = gtk::gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_string(
        r#"
        .waymoji-window,
        .waymoji-background {
            background-color: transparent;
        }
        .waymoji-menu {
            background-color: rgba(24, 24, 28, 0.97);
            border-radius: 12px;
            color: #f5f5f8;
            border: 1px solid rgba(255, 255, 255, 0.12);
        }
        .waymoji-search-box {
            background: transparent;
            padding: 2px 6px;
            border-radius: 6px;
        }
        .waymoji-search-box.search-focus {
            background-color: rgba(255, 255, 255, 0.08);
        }
        .waymoji-search-box.list-focus {
            opacity: 0.55;
        }
        .waymoji-search {
            font-size: 15px;
            color: #f5f5f8;
            font-family: monospace;
            background: transparent;
            border: none;
            box-shadow: none;
            padding: 0;
        }
        .waymoji-caret {
            background-color: #f5f5f8;
            min-width: 1px;
        }
        text.waymoji-search selection {
            background-color: rgba(120, 170, 255, 0.45);
            color: #ffffff;
        }
        .waymoji-footer {
            font-size: 13px;
            color: #c9c9d2;
        }
        .waymoji-close {
            min-width: 28px;
            min-height: 28px;
            padding: 0;
            font-size: 18px;
        }
        .waymoji-listview,
        .waymoji-row,
        .waymoji-emoji-row {
            background: transparent;
        }
        .waymoji-listview > row {
            padding: 0;
            margin: 0;
            background: transparent;
            background-image: none;
            box-shadow: none;
            outline: none;
        }
        .waymoji-listview > row:hover {
            background: transparent;
            background-image: none;
            box-shadow: none;
        }
        .waymoji-listview > row:hover > .waymoji-row {
            background: transparent;
        }
        .waymoji-separator-row {
            margin-top: 14px;
            margin-bottom: 4px;
            padding: 0 2px;
        }
        .waymoji-separator-label {
            font-size: 11px;
            font-weight: bold;
            color: #aeb0bd;
        }
        .waymoji-separator-line {
            background-color: rgba(255, 255, 255, 0.14);
            min-height: 1px;
        }
        .waymoji-emoji {
            font-family: "Noto Color Emoji";
            font-size: 24px;
            min-width: 38px;
            min-height: 38px;
            padding: 0;
            background: transparent;
            border: none;
            box-shadow: none;
            margin: 1px;
        }
        .waymoji-emoji:hover {
            background-color: rgba(255, 255, 255, 0.10);
        }
        .waymoji-category-rail {
            background: transparent;
        }
        .waymoji-category {
            min-width: 30px;
            min-height: 28px;
            padding: 0;
            font-size: 17px;
            font-family: "Noto Color Emoji";
            background: transparent;
            border: none;
            box-shadow: none;
            border-radius: 6px;
        }
        .waymoji-category.active {
            background-color: rgba(255, 255, 255, 0.18);
        }
        .waymoji-emoji.selected {
            background-color: rgba(255, 255, 255, 0.20);
        }
        "#,
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}
