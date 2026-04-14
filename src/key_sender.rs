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

#[cfg(not(target_os = "windows"))]
pub fn send_key(_hwnd: isize, _vk: VirtualKey, _mode: SendMode) {}

pub static KEY_NAMES: &[(&str, VirtualKey)] = &[
    ("A", VirtualKey(0x41)), ("B", VirtualKey(0x42)), ("C", VirtualKey(0x43)),
    ("D", VirtualKey(0x44)), ("E", VirtualKey(0x45)), ("F", VirtualKey(0x46)),
    ("G", VirtualKey(0x47)), ("H", VirtualKey(0x48)), ("I", VirtualKey(0x49)),
    ("J", VirtualKey(0x4A)), ("K", VirtualKey(0x4B)), ("L", VirtualKey(0x4C)),
    ("M", VirtualKey(0x4D)), ("N", VirtualKey(0x4E)), ("O", VirtualKey(0x4F)),
    ("P", VirtualKey(0x50)), ("Q", VirtualKey(0x51)), ("R", VirtualKey(0x52)),
    ("S", VirtualKey(0x53)), ("T", VirtualKey(0x54)), ("U", VirtualKey(0x55)),
    ("V", VirtualKey(0x56)), ("W", VirtualKey(0x57)), ("X", VirtualKey(0x58)),
    ("Y", VirtualKey(0x59)), ("Z", VirtualKey(0x5A)),
    ("0", VirtualKey(0x30)), ("1", VirtualKey(0x31)), ("2", VirtualKey(0x32)),
    ("3", VirtualKey(0x33)), ("4", VirtualKey(0x34)), ("5", VirtualKey(0x35)),
    ("6", VirtualKey(0x36)), ("7", VirtualKey(0x37)), ("8", VirtualKey(0x38)),
    ("9", VirtualKey(0x39)),
    ("F1", VirtualKey(0x70)), ("F2", VirtualKey(0x71)), ("F3", VirtualKey(0x72)),
    ("F4", VirtualKey(0x73)), ("F5", VirtualKey(0x74)), ("F6", VirtualKey(0x75)),
    ("F7", VirtualKey(0x76)), ("F8", VirtualKey(0x77)), ("F9", VirtualKey(0x78)),
    ("F10", VirtualKey(0x79)), ("F11", VirtualKey(0x7A)), ("F12", VirtualKey(0x7B)),
    ("SPACE", VirtualKey(0x20)), ("ENTER", VirtualKey(0x0D)),
    ("TAB", VirtualKey(0x09)), ("ESC", VirtualKey(0x1B)),
    ("BACKSPACE", VirtualKey(0x08)), ("DELETE", VirtualKey(0x2E)),
    ("INSERT", VirtualKey(0x2D)), ("HOME", VirtualKey(0x24)),
    ("END", VirtualKey(0x23)), ("PAGEUP", VirtualKey(0x21)),
    ("PAGEDOWN", VirtualKey(0x22)), ("UP", VirtualKey(0x26)),
    ("DOWN", VirtualKey(0x28)), ("LEFT", VirtualKey(0x25)),
    ("RIGHT", VirtualKey(0x27)),
];
