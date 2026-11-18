# waymoji — agent notes

## What this is

A Wayland emoji picker: full-screen layer-shell overlay, centered menu, search, virtualized emoji list, right-side category rail. Emoji insertion uses `zwp_input_method_v2`; `zwp_virtual_keyboard_v1` is a fallback. Tested on niri only.

## Build and verify

```sh
cargo fmt --check
cargo clippy --release --all-targets
cargo build --release
```

There are no tests by user preference. Manual smoke testing matters more than adding test scaffolding unless asked.

Debug logging: `WAYMOJI_DEBUG=1`.

## Architecture

- `src/main.rs`: lock acquisition, GTK application startup.
- `src/lock.rs`: single-instance lock.
- `src/messages.rs`: `UiEvent`, `KeyAction`, `UiCommand`.
- `src/ime/mod.rs`: IME thread, Wayland event loop, command/wake handling.
- `src/ime/state.rs`: input state, key handling, IME/virtual-keyboard insertion.
- `src/ime/protocol.rs`: Wayland dispatch implementations.
- `src/ime/keymap.rs`: virtual-keyboard keymap generation.
- `src/ui/mod.rs`: window/menu construction and UI event loop.
- `src/ui/input.rs`: search/list focus and key actions.
- `src/ui/style.rs`: CSS.
- `src/ui/emoji/`: list model, view/factory, navigation/selection, category rail.

Thread model:

- GTK stays on the main thread.
- IME runs on its own thread.
- IME → UI uses `async-channel`.
- UI → IME uses `mpsc` plus a UnixStream wake pipe.
- The IME thread must never touch GTK or `Rc<RefCell<Ui>>`.

## Invariants

- **Never use the clipboard for emoji insertion.** Clipboard use is limited to search copy/cut/paste.
- Keep `KeyboardMode::None`; input reaches the UI through the IME keyboard grab.
- No niri IPC. The layer surface uses the active output so niri chooses placement.
- No `unsafe`. Use safe std/rustix/xkbcommon APIs.
- Keep the emoji list virtualized with `GtkListView`; do not reintroduce a full grid or result cap.
- Search spans all emoji and matches names/shortcodes.
- Keep real category separators, the right-side rail, and scroll-driven category indication.
- Selection is custom (`NoSelection` model). When selection changes, update both the CSS `selected` class and accessible `Selected` state.
- Emoji rows are 8 columns, 44 px high. Category jumps and indicator math depend on that.
- Preserve single-instance behavior.
- License is Unlicense (`Cargo.toml` and `LICENSE`). Do not change without being asked.

## Working style

- Do exactly what is asked; no opportunistic refactors, renames, dependency changes, or formatting passes.
- Keep the README simple. Protocols first, then Usage, then Building.
- No code comments unless requested.
- Prefer small focused modules. `ui/emoji.rs` was split because it grew too large; do not merge it back.
- Prefer `Rc::downgrade()` in long-lived GTK signal closures to avoid strong cycles.
- Do not hold a `RefCell` borrow across GTK calls that may re-enter UI code.
- Do not commit, push, or change license/README beyond the requested scope.

## Known review findings

These were identified but not all are fixed. Verify before relying on or changing them:

- `ui/emoji/category.rs`: jumping to the last categories can highlight the wrong rail item because scrolling clamps and the indicator uses the top row.
- `ui/mod.rs`: footer can remain “Inserting…” when `grabbed == false`.
- `ime/mod.rs` / `state.rs`: a pending `done` event can leave the commit serial stale.
- `ime/protocol.rs`: `repeat_info` is ignored, so key repeat depends on compositor behavior.
- `ime/state.rs`: keyboard grab is not released on IME deactivation.
- `ui/mod.rs` and emoji callbacks can form strong `Rc<Ui>` cycles.
- The custom caret may duplicate/misalign the native caret, and ignores horizontal scroll/RTL.
- Empty search results can leave a clicked category highlighted.
- Lock fallback uses `/tmp` when `XDG_RUNTIME_DIR` is unset.

## Manual smoke test

1. Launch and confirm the menu is centered on the active niri output.
2. Type a query; confirm filtering and empty-result handling.
3. Test arrows, `Tab`, `Enter`, `Esc`, `Ctrl+Tab`, and `Ctrl+Shift+Tab`.
4. Click an emoji; confirm it inserts without touching the clipboard.
5. Click a category, especially the last ones; check selection and rail consistency.
6. Click outside; confirm the app exits.
7. Launch a second instance; confirm it does not open another window.
