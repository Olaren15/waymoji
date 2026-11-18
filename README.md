# waymoji

A small emoji picker for Wayland.

Tested on niri, but should work on sway, cosmic any compositor that supports
the following protocols:

- layer-shell
- zwp_input_method_v2
- zwp_virtual_keyboard_v1

## AI usage

Built with AI at the **EXECUTE** level of the [0rwx framework](https://0rwx.com/):
AI wrote the code, and I curated it.

## Usage

Bind `waymoji` to a key in your compositor config.

- Type to search by emoji name or shortcode.
- `Tab` toggles between search and the emoji list.
- Arrows navigate, `Enter` inserts, `Esc` closes.
- `Ctrl+Tab` and `Ctrl+Shift+Tab` cycle categories.
- Click a category or emoji; click outside to close.

## Building

Requirements:

- Rust, GTK 4.12+, gtk4-layer-shell, and xkbcommon

```sh
cargo build
cargo run
```

## License

Unlicense
