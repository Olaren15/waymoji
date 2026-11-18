use std::collections::HashSet;
use std::os::fd::AsFd;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow, bail};
use async_channel::Sender;
use wayland_client::protocol::{wl_keyboard, wl_seat};
use wayland_client::{Connection, EventQueue};
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_keyboard_grab_v2::ZwpInputMethodKeyboardGrabV2;
use wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_v2::ZwpInputMethodV2;
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1;
use xkbcommon::xkb;

use super::debug_log;
use super::keymap::{EVDEV_KEYCODES, build_keymap, create_keymap_fd};
use crate::messages::{KeyAction, UiEvent};

pub(super) struct ImeState {
    pub(super) context: xkb::Context,
    pub(super) xkb_state: Option<xkb::State>,
    pub(super) serial: u32,
    pub(super) active: bool,
    pub(super) unavailable: bool,
    pub(super) ime: Option<ZwpInputMethodV2>,
    pub(super) grab: Option<ZwpInputMethodKeyboardGrabV2>,
    seat: wl_seat::WlSeat,
    vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    pub(super) tx: Sender<UiEvent>,
    pub(super) ignore_until: Instant,
    block_enter: bool,
    pub(super) pressed_keys: HashSet<u32>,
}

impl ImeState {
    pub(super) fn new(
        tx: Sender<UiEvent>,
        ime: ZwpInputMethodV2,
        seat: wl_seat::WlSeat,
        vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    ) -> Self {
        Self {
            context: xkb::Context::new(xkb::CONTEXT_NO_FLAGS),
            xkb_state: None,
            serial: 0,
            active: false,
            unavailable: false,
            ime: Some(ime),
            grab: None,
            seat,
            vk_manager,
            tx,
            ignore_until: Instant::now(),
            block_enter: false,
            pressed_keys: HashSet::new(),
        }
    }

    fn send_key(&self, action: KeyAction) {
        let _ = self.tx.try_send(UiEvent::Key(action));
    }

    pub(super) fn handle_key(&mut self, key: u32, pressed: bool) {
        let Some(state) = self.xkb_state.as_mut() else {
            return;
        };

        let keycode = xkb::Keycode::new(key + 8);

        if !pressed {
            self.pressed_keys.remove(&key);
            let sym = state.key_get_one_sym(keycode);
            if matches!(
                sym.raw(),
                xkb::keysyms::KEY_Return | xkb::keysyms::KEY_KP_Enter
            ) {
                self.block_enter = false;
            }
            state.update_key(keycode, xkb::KeyDirection::Up);
            return;
        }

        if self.pressed_keys.insert(key) {
            state.update_key(keycode, xkb::KeyDirection::Down);
        }

        let sym = state.key_get_one_sym(keycode);
        let text = state.key_get_utf8(keycode);
        let is_enter = matches!(
            sym.raw(),
            xkb::keysyms::KEY_Return | xkb::keysyms::KEY_KP_Enter
        );

        if Instant::now() < self.ignore_until {
            if is_enter {
                self.block_enter = true;
            }
            return;
        }
        if is_enter && self.block_enter {
            return;
        }

        let shift = state.mod_name_is_active("Shift", xkb::STATE_MODS_EFFECTIVE);
        let ctrl = state.mod_name_is_active("Control", xkb::STATE_MODS_EFFECTIVE);
        let alt = state.mod_name_is_active("Alt", xkb::STATE_MODS_EFFECTIVE);
        let logo = state.mod_name_is_active("Super", xkb::STATE_MODS_EFFECTIVE)
            || state.mod_name_is_active("Logo", xkb::STATE_MODS_EFFECTIVE);
        if logo {
            return;
        }

        let key = sym.raw();
        let action = if ctrl {
            match key {
                xkb::keysyms::KEY_a => Some(KeyAction::SelectAll),
                xkb::keysyms::KEY_c => Some(KeyAction::Copy),
                xkb::keysyms::KEY_x => Some(KeyAction::Cut),
                xkb::keysyms::KEY_v => Some(KeyAction::Paste),
                xkb::keysyms::KEY_z if shift => Some(KeyAction::Redo),
                xkb::keysyms::KEY_z => Some(KeyAction::Undo),
                xkb::keysyms::KEY_y => Some(KeyAction::Redo),
                xkb::keysyms::KEY_u => Some(KeyAction::DeleteToStart),
                xkb::keysyms::KEY_k => Some(KeyAction::DeleteToEnd),
                xkb::keysyms::KEY_w => Some(KeyAction::DeleteWordBackward),
                xkb::keysyms::KEY_h => Some(KeyAction::DeleteBackward),
                xkb::keysyms::KEY_d => Some(KeyAction::DeleteForward),
                xkb::keysyms::KEY_b => Some(KeyAction::MoveLeft { extend: shift }),
                xkb::keysyms::KEY_f => Some(KeyAction::MoveRight { extend: shift }),
                xkb::keysyms::KEY_e => Some(KeyAction::MoveEnd { extend: shift }),
                xkb::keysyms::KEY_BackSpace => Some(KeyAction::DeleteWordBackward),
                xkb::keysyms::KEY_Delete => Some(KeyAction::DeleteWordForward),
                xkb::keysyms::KEY_Left => Some(KeyAction::MoveWordLeft { extend: shift }),
                xkb::keysyms::KEY_Right => Some(KeyAction::MoveWordRight { extend: shift }),
                xkb::keysyms::KEY_Home => Some(KeyAction::MoveHome { extend: shift }),
                xkb::keysyms::KEY_End => Some(KeyAction::MoveEnd { extend: shift }),
                xkb::keysyms::KEY_Tab => Some(KeyAction::NextCategory),
                xkb::keysyms::KEY_ISO_Left_Tab => Some(KeyAction::PreviousCategory),
                _ => None,
            }
        } else if alt {
            match key {
                xkb::keysyms::KEY_Left => Some(KeyAction::MoveWordLeft { extend: shift }),
                xkb::keysyms::KEY_Right => Some(KeyAction::MoveWordRight { extend: shift }),
                xkb::keysyms::KEY_BackSpace => Some(KeyAction::DeleteWordBackward),
                xkb::keysyms::KEY_Delete => Some(KeyAction::DeleteWordForward),
                _ if !text.is_empty() && !text.chars().any(char::is_control) => {
                    Some(KeyAction::Char(text))
                }
                _ => None,
            }
        } else {
            match key {
                xkb::keysyms::KEY_Escape => Some(KeyAction::Escape),
                xkb::keysyms::KEY_Return | xkb::keysyms::KEY_KP_Enter => Some(KeyAction::Enter),
                xkb::keysyms::KEY_BackSpace => Some(KeyAction::DeleteBackward),
                xkb::keysyms::KEY_Delete => Some(KeyAction::DeleteForward),
                xkb::keysyms::KEY_Left => Some(KeyAction::MoveLeft { extend: shift }),
                xkb::keysyms::KEY_Right => Some(KeyAction::MoveRight { extend: shift }),
                xkb::keysyms::KEY_Home => Some(KeyAction::MoveHome { extend: shift }),
                xkb::keysyms::KEY_End => Some(KeyAction::MoveEnd { extend: shift }),
                xkb::keysyms::KEY_Up => Some(KeyAction::Up),
                xkb::keysyms::KEY_Down => Some(KeyAction::Down),
                xkb::keysyms::KEY_Tab | xkb::keysyms::KEY_ISO_Left_Tab => {
                    Some(KeyAction::ToggleList)
                }
                _ if !text.is_empty() && !text.chars().any(char::is_control) => {
                    Some(KeyAction::Char(text))
                }
                _ => None,
            }
        };

        if let Some(action) = action {
            debug_log(format_args!("key: {action:?}"));
            self.send_key(action);
        }
    }

    pub(super) fn release_grab(&mut self) {
        if let Some(grab) = self.grab.take() {
            grab.release();
        }
    }

    pub(super) fn insert(
        &mut self,
        text: String,
        conn: &Connection,
        queue: &mut EventQueue<Self>,
    ) -> Result<()> {
        debug_log(format_args!(
            "insert: active={} bytes={}",
            self.active,
            text.len()
        ));
        if self.active {
            let ime = self
                .ime
                .clone()
                .ok_or_else(|| anyhow!("input method is unavailable"))?;
            ime.commit_string(text);
            ime.commit(self.serial);
            conn.flush()?;
            queue.roundtrip(self)?;
        } else {
            self.send_virtual_text(&text, conn, queue)?;
        }

        conn.flush()?;
        Ok(())
    }

    fn send_virtual_text(
        &mut self,
        text: &str,
        conn: &Connection,
        queue: &mut EventQueue<Self>,
    ) -> Result<()> {
        let chars: Vec<char> = text.chars().collect();
        let codepoints: Vec<String> = chars
            .iter()
            .map(|ch| format!("{:04X}", *ch as u32))
            .collect();
        debug_log(format_args!(
            "virtual keyboard: chars={} codepoints={}",
            chars.len(),
            codepoints.join(",")
        ));
        if chars.is_empty() {
            return Ok(());
        }
        if chars.len() > EVDEV_KEYCODES.len() {
            bail!("emoji sequence is too long");
        }

        let manager = self
            .vk_manager
            .clone()
            .ok_or_else(|| anyhow!("compositor does not support the virtual keyboard protocol"))?;
        let qh = queue.handle();
        let vk = manager.create_virtual_keyboard(&self.seat, &qh, ());

        let result = (|| -> Result<()> {
            let keymap = build_keymap(&chars);
            let (fd, size) = create_keymap_fd(keymap.as_bytes())?;

            vk.keymap(wl_keyboard::KeymapFormat::XkbV1.into(), fd.as_fd(), size);
            conn.flush()?;
            queue.roundtrip(self)?;

            for (i, _) in chars.iter().enumerate() {
                let key = EVDEV_KEYCODES[i];
                vk.key(0, key, wl_keyboard::KeyState::Pressed.into());
                vk.key(0, key, wl_keyboard::KeyState::Released.into());
                thread::sleep(Duration::from_millis(2));
            }

            conn.flush()?;
            queue.roundtrip(self)?;
            Ok(())
        })();

        vk.destroy();
        conn.flush()?;
        queue.roundtrip(self)?;
        result
    }

    pub(super) fn cancel(&mut self, conn: &Connection) {
        self.release_grab();
        let _ = conn.flush();
    }
}
