use std::fmt;

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, POINT},
    UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetCursorPos, GetWindowTextLengthW, GetWindowTextW,
        IsWindowVisible, RealGetWindowClassW, WindowFromPoint, GA_ROOT,
    },
};

#[derive(Clone, Debug)]
pub struct WindowInfo {
    #[cfg(target_os = "windows")]
    pub hwnd: isize,
    pub title: String,
    pub class_name: String,
}

impl fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[cfg(target_os = "windows")]
pub fn get_window_under_cursor() -> Option<WindowInfo> {
    unsafe {
        let mut pt = POINT::default();
        GetCursorPos(&mut pt).ok()?;
        let child_hwnd = WindowFromPoint(pt);
        if child_hwnd.0.is_null() {
            return None;
        }
        let hwnd = GetAncestor(child_hwnd, GA_ROOT);
        if hwnd.0.is_null() {
            return None;
        }
        read_window_info(hwnd)
    }
}

#[cfg(target_os = "windows")]
unsafe fn read_window_info(hwnd: HWND) -> Option<WindowInfo> {
    let title_len = GetWindowTextLengthW(hwnd);
    if title_len == 0 {
        return None;
    }
    let mut title_buf = vec![0u16; (title_len + 1) as usize];
    let copied = GetWindowTextW(hwnd, &mut title_buf);
    if copied == 0 {
        return None;
    }
    let title = String::from_utf16_lossy(&title_buf[..copied as usize]);

    let mut cls_buf = [0u16; 256];
    let cls_len = RealGetWindowClassW(hwnd, &mut cls_buf);
    let class_name = String::from_utf16_lossy(&cls_buf[..cls_len as usize]);

    Some(WindowInfo {
        hwnd: hwnd.0 as isize,
        title,
        class_name,
    })
}

#[cfg(target_os = "windows")]
pub fn list_visible_windows() -> Vec<WindowInfo> {
    let mut results: Vec<WindowInfo> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(enum_window_proc),
            LPARAM(&mut results as *mut Vec<WindowInfo> as isize),
        );
    }
    results
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_window_proc(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if !IsWindowVisible(hwnd).as_bool() {
        return BOOL(1);
    }
    let results = &mut *(lparam.0 as *mut Vec<WindowInfo>);
    if let Some(info) = read_window_info(hwnd) {
        if !info.title.is_empty() {
            results.push(info);
        }
    }
    BOOL(1)
}

#[cfg(not(target_os = "windows"))]
pub fn get_window_under_cursor() -> Option<WindowInfo> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn list_visible_windows() -> Vec<WindowInfo> {
    Vec::new()
}
