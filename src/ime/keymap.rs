use std::fs::File;
use std::io::Write;
use std::os::fd::OwnedFd;

use anyhow::{Context, Result};
use rustix::fs::{MemfdFlags, memfd_create};
use xkbcommon::xkb;

pub(super) const EVDEV_KEYCODES: [u32; 48] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 30, 31,
    32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 57,
];

pub(super) fn build_keymap(chars: &[char]) -> String {
    let max_keycode = EVDEV_KEYCODES[chars.len() - 1] + 8;
    let mut keymap = String::new();
    keymap.push_str("xkb_keymap {\n");
    keymap.push_str("xkb_keycodes \"(unnamed)\" {\n");
    keymap.push_str("minimum = 8;\n");
    keymap.push_str(&format!("maximum = {max_keycode};\n"));
    for (i, code) in EVDEV_KEYCODES.iter().take(chars.len()).enumerate() {
        keymap.push_str(&format!("<K{}> = {};\n", i + 1, code + 8));
    }
    keymap.push_str("};\n");
    keymap.push_str("xkb_types \"(unnamed)\" { include \"complete\" };\n");
    keymap.push_str("xkb_compatibility \"(unnamed)\" { include \"complete\" };\n");
    keymap.push_str("xkb_symbols \"(unnamed)\" {\n");
    for (i, ch) in chars.iter().enumerate() {
        let sym = xkb::utf32_to_keysym(*ch as u32);
        let mut name = xkb::keysym_get_name(sym);
        if name.is_empty() || name == "NoSymbol" {
            name = format!("U{:04X}", *ch as u32);
        }
        keymap.push_str(&format!("key <K{}> {{[{}]}};\n", i + 1, name));
    }
    keymap.push_str("};\n");
    keymap.push_str("};\n");
    keymap
}

pub(super) fn create_keymap_fd(bytes: &[u8]) -> Result<(OwnedFd, u32)> {
    let fd = memfd_create("waymoji-keymap", MemfdFlags::CLOEXEC).context("memfd_create failed")?;
    let mut file = File::from(fd.try_clone()?);
    file.write_all(bytes)?;
    file.write_all(b"\0")?;
    file.flush()?;

    Ok((fd, (bytes.len() + 1) as u32))
}
