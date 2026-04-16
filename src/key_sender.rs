#[cfg(target_os = "macos")]
use std::{ffi::c_void, process::Command, sync::Once};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendMode {
    PostMessage,
    SendMessage,
}

impl SendMode {
    pub fn label(&self) -> &'static str {
        match self {
            Self::PostMessage => "PostMessage",
            Self::SendMessage => "SendMessage",
        }
    }

    pub fn all() -> &'static [SendMode] {
        &[Self::PostMessage, Self::SendMessage]
    }
}

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{
        Input::KeyboardAndMouse::{MapVirtualKeyW, MAP_VIRTUAL_KEY_TYPE},
        WindowsAndMessaging::{PostMessageW, SendMessageW, WM_CHAR, WM_KEYDOWN, WM_KEYUP},
    },
};

#[cfg(target_os = "macos")]
type CGEventRef = *const c_void;

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> CGEventRef;
    fn CGEventPostToPid(pid: i32, event: CGEventRef);
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VirtualKey(pub u16);

impl VirtualKey {
    pub fn name(&self) -> &'static str {
        KEY_NAMES
            .iter()
            .find(|(_, vk)| vk.0 == self.0)
            .map(|(name, _)| *name)
            .unwrap_or("?")
    }
}

#[cfg(target_os = "windows")]
fn make_lparam_down(scan_code: u32) -> LPARAM {
    LPARAM((1 | ((scan_code & 0xFF) << 16)) as isize)
}

#[cfg(target_os = "windows")]
fn make_lparam_up(scan_code: u32) -> LPARAM {
    LPARAM((1 | ((scan_code & 0xFF) << 16) | (1 << 30) | (1 << 31)) as isize)
}

#[cfg(target_os = "windows")]
fn get_scan_code(vk: u16) -> u32 {
    unsafe { MapVirtualKeyW(vk as u32, MAP_VIRTUAL_KEY_TYPE(0)) }
}

#[cfg(target_os = "windows")]
pub fn send_key(hwnd: isize, vk: VirtualKey, mode: SendMode) {
    match mode {
        SendMode::PostMessage => send_key_post(hwnd, vk),
        SendMode::SendMessage => send_key_sendmsg(hwnd, vk),
    }
}

#[cfg(target_os = "windows")]
fn send_key_post(hwnd: isize, vk: VirtualKey) {
    unsafe {
        let h = HWND(hwnd as *mut _);
        let scan = get_scan_code(vk.0);
        let _ = PostMessageW(h, WM_KEYDOWN, WPARAM(vk.0 as usize), make_lparam_down(scan));
        if let Some(ch) = vk_to_char(vk.0) {
            let _ = PostMessageW(h, WM_CHAR, WPARAM(ch as usize), make_lparam_down(scan));
        }
        let _ = PostMessageW(h, WM_KEYUP, WPARAM(vk.0 as usize), make_lparam_up(scan));
    }
}

#[cfg(target_os = "windows")]
fn send_key_sendmsg(hwnd: isize, vk: VirtualKey) {
    unsafe {
        let h = HWND(hwnd as *mut _);
        let scan = get_scan_code(vk.0);
        SendMessageW(h, WM_KEYDOWN, WPARAM(vk.0 as usize), make_lparam_down(scan));
        if let Some(ch) = vk_to_char(vk.0) {
            SendMessageW(h, WM_CHAR, WPARAM(ch as usize), make_lparam_down(scan));
        }
        SendMessageW(h, WM_KEYUP, WPARAM(vk.0 as usize), make_lparam_up(scan));
    }
}

#[cfg(target_os = "windows")]
fn vk_to_char(vk: u16) -> Option<u32> {
    match vk {
        0x20 => Some(' ' as u32),
        0x0D => Some('\r' as u32),
        0x09 => Some('\t' as u32),
        v if (0x30..=0x39).contains(&v) => Some(v as u32),
        v if (0x41..=0x5A).contains(&v) => Some((v - 0x41 + 'a' as u16) as u32),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
static ACCESSIBILITY_WARNING: Once = Once::new();

#[cfg(target_os = "macos")]
const ACCESSIBILITY_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";

#[cfg(target_os = "macos")]
pub fn accessibility_trusted() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

#[cfg(not(target_os = "macos"))]
pub fn accessibility_trusted() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub fn open_accessibility_settings() -> bool {
    Command::new("open")
        .arg(ACCESSIBILITY_SETTINGS_URL)
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "macos"))]
pub fn open_accessibility_settings() -> bool {
    false
}

#[cfg(target_os = "macos")]
pub fn send_key(target_pid: isize, vk: VirtualKey, _mode: SendMode) {
    if target_pid <= 0 {
        return;
    }

    if !accessibility_trusted() {
        ACCESSIBILITY_WARNING.call_once(|| {
            tracing::warn!(
                "macOS 按键发送需要在“系统设置 -> 隐私与安全性 -> 辅助功能”里允许当前应用"
            );
        });
    }

    let Some(key_code) = macos_key_code(vk) else {
        return;
    };

    unsafe {
        macos_post_key_event(target_pid as i32, key_code, true);
        macos_post_key_event(target_pid as i32, key_code, false);
    }
}

#[cfg(target_os = "macos")]
unsafe fn macos_post_key_event(target_pid: i32, key_code: u16, key_down: bool) {
    let event = unsafe { CGEventCreateKeyboardEvent(std::ptr::null(), key_code, key_down) };
    if event.is_null() {
        return;
    }

    unsafe { CGEventPostToPid(target_pid, event) };
    unsafe { CFRelease(event) };
}

#[cfg(target_os = "macos")]
fn macos_key_code(vk: VirtualKey) -> Option<u16> {
    match vk.0 {
        0x41 => Some(0x00), // A
        0x42 => Some(0x0B), // B
        0x43 => Some(0x08), // C
        0x44 => Some(0x02), // D
        0x45 => Some(0x0E), // E
        0x46 => Some(0x03), // F
        0x47 => Some(0x05), // G
        0x48 => Some(0x04), // H
        0x49 => Some(0x22), // I
        0x4A => Some(0x26), // J
        0x4B => Some(0x28), // K
        0x4C => Some(0x25), // L
        0x4D => Some(0x2E), // M
        0x4E => Some(0x2D), // N
        0x4F => Some(0x1F), // O
        0x50 => Some(0x23), // P
        0x51 => Some(0x0C), // Q
        0x52 => Some(0x0F), // R
        0x53 => Some(0x01), // S
        0x54 => Some(0x11), // T
        0x55 => Some(0x20), // U
        0x56 => Some(0x09), // V
        0x57 => Some(0x0D), // W
        0x58 => Some(0x07), // X
        0x59 => Some(0x10), // Y
        0x5A => Some(0x06), // Z
        0x30 => Some(0x1D), // 0
        0x31 => Some(0x12), // 1
        0x32 => Some(0x13), // 2
        0x33 => Some(0x14), // 3
        0x34 => Some(0x15), // 4
        0x35 => Some(0x17), // 5
        0x36 => Some(0x16), // 6
        0x37 => Some(0x1A), // 7
        0x38 => Some(0x1C), // 8
        0x39 => Some(0x19), // 9
        0x70 => Some(0x7A), // F1
        0x71 => Some(0x78), // F2
        0x72 => Some(0x63), // F3
        0x73 => Some(0x76), // F4
        0x74 => Some(0x60), // F5
        0x75 => Some(0x61), // F6
        0x76 => Some(0x62), // F7
        0x77 => Some(0x64), // F8
        0x78 => Some(0x65), // F9
        0x79 => Some(0x6D), // F10
        0x7A => Some(0x67), // F11
        0x7B => Some(0x6F), // F12
        0x20 => Some(0x31), // Space
        0x0D => Some(0x24), // Enter
        0x09 => Some(0x30), // Tab
        0x1B => Some(0x35), // Esc
        0x08 => Some(0x33), // Backspace
        0x2E => Some(0x75), // Delete
        0x2D => Some(0x72), // Insert / Help
        0x24 => Some(0x73), // Home
        0x23 => Some(0x77), // End
        0x21 => Some(0x74), // PageUp
        0x22 => Some(0x79), // PageDown
        0x26 => Some(0x7E), // Up
        0x28 => Some(0x7D), // Down
        0x25 => Some(0x7B), // Left
        0x27 => Some(0x7C), // Right
        _ => None,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn send_key(_hwnd: isize, _vk: VirtualKey, _mode: SendMode) {}

pub static KEY_NAMES: &[(&str, VirtualKey)] = &[
    ("A", VirtualKey(0x41)),
    ("B", VirtualKey(0x42)),
    ("C", VirtualKey(0x43)),
    ("D", VirtualKey(0x44)),
    ("E", VirtualKey(0x45)),
    ("F", VirtualKey(0x46)),
    ("G", VirtualKey(0x47)),
    ("H", VirtualKey(0x48)),
    ("I", VirtualKey(0x49)),
    ("J", VirtualKey(0x4A)),
    ("K", VirtualKey(0x4B)),
    ("L", VirtualKey(0x4C)),
    ("M", VirtualKey(0x4D)),
    ("N", VirtualKey(0x4E)),
    ("O", VirtualKey(0x4F)),
    ("P", VirtualKey(0x50)),
    ("Q", VirtualKey(0x51)),
    ("R", VirtualKey(0x52)),
    ("S", VirtualKey(0x53)),
    ("T", VirtualKey(0x54)),
    ("U", VirtualKey(0x55)),
    ("V", VirtualKey(0x56)),
    ("W", VirtualKey(0x57)),
    ("X", VirtualKey(0x58)),
    ("Y", VirtualKey(0x59)),
    ("Z", VirtualKey(0x5A)),
    ("0", VirtualKey(0x30)),
    ("1", VirtualKey(0x31)),
    ("2", VirtualKey(0x32)),
    ("3", VirtualKey(0x33)),
    ("4", VirtualKey(0x34)),
    ("5", VirtualKey(0x35)),
    ("6", VirtualKey(0x36)),
    ("7", VirtualKey(0x37)),
    ("8", VirtualKey(0x38)),
    ("9", VirtualKey(0x39)),
    ("F1", VirtualKey(0x70)),
    ("F2", VirtualKey(0x71)),
    ("F3", VirtualKey(0x72)),
    ("F4", VirtualKey(0x73)),
    ("F5", VirtualKey(0x74)),
    ("F6", VirtualKey(0x75)),
    ("F7", VirtualKey(0x76)),
    ("F8", VirtualKey(0x77)),
    ("F9", VirtualKey(0x78)),
    ("F10", VirtualKey(0x79)),
    ("F11", VirtualKey(0x7A)),
    ("F12", VirtualKey(0x7B)),
    ("SPACE", VirtualKey(0x20)),
    ("ENTER", VirtualKey(0x0D)),
    ("TAB", VirtualKey(0x09)),
    ("ESC", VirtualKey(0x1B)),
    ("BACKSPACE", VirtualKey(0x08)),
    ("DELETE", VirtualKey(0x2E)),
    ("INSERT", VirtualKey(0x2D)),
    ("HOME", VirtualKey(0x24)),
    ("END", VirtualKey(0x23)),
    ("PAGEUP", VirtualKey(0x21)),
    ("PAGEDOWN", VirtualKey(0x22)),
    ("UP", VirtualKey(0x26)),
    ("DOWN", VirtualKey(0x28)),
    ("LEFT", VirtualKey(0x25)),
    ("RIGHT", VirtualKey(0x27)),
];

#[cfg(test)]
mod tests {
    use super::{VirtualKey, KEY_NAMES};

    #[cfg(target_os = "macos")]
    use super::{macos_key_code, ACCESSIBILITY_SETTINGS_URL};

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_supports_all_configured_keys() {
        let unsupported: Vec<&str> = KEY_NAMES
            .iter()
            .filter_map(|(name, vk)| macos_key_code(*vk).is_none().then_some(*name))
            .collect();

        assert!(
            unsupported.is_empty(),
            "macOS 仍有未映射按键: {unsupported:?}"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_key_code_matches_known_values() {
        assert_eq!(macos_key_code(VirtualKey(0x41)), Some(0x00));
        assert_eq!(macos_key_code(VirtualKey(0x0D)), Some(0x24));
        assert_eq!(macos_key_code(VirtualKey(0x2E)), Some(0x75));
        assert_eq!(macos_key_code(VirtualKey(0x78)), Some(0x65));
        assert_eq!(macos_key_code(VirtualKey(0x25)), Some(0x7B));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_accessibility_settings_url_is_expected() {
        assert_eq!(
            ACCESSIBILITY_SETTINGS_URL,
            "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
        );
    }
}
