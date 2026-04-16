use std::fmt;

#[cfg(target_os = "macos")]
use std::{
    ffi::{c_char, c_void},
    ptr,
};

#[cfg(target_os = "windows")]
use windows::Win32::{
    Foundation::{BOOL, HWND, LPARAM, POINT},
    UI::WindowsAndMessaging::{
        EnumWindows, GetAncestor, GetCursorPos, GetWindowTextLengthW, GetWindowTextW,
        IsWindowVisible, RealGetWindowClassW, WindowFromPoint, GA_ROOT,
    },
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_LBUTTON, VK_MBUTTON, VK_RBUTTON,
};

#[cfg(target_os = "macos")]
type CFArrayRef = *const c_void;
#[cfg(target_os = "macos")]
type CFDictionaryRef = *const c_void;
#[cfg(target_os = "macos")]
type CFStringRef = *const c_void;
#[cfg(target_os = "macos")]
type CGEventType = u32;
#[cfg(target_os = "macos")]
type CGEventSourceStateID = i32;
#[cfg(target_os = "macos")]
type CGMouseButton = u32;

#[cfg(target_os = "macos")]
const K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY: u32 = 1;
#[cfg(target_os = "macos")]
const K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS: u32 = 1 << 4;
#[cfg(target_os = "macos")]
const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
#[cfg(target_os = "macos")]
const K_CF_NUMBER_SINT64_TYPE: i32 = 4;
#[cfg(target_os = "macos")]
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE: CGEventSourceStateID = 0;
#[cfg(target_os = "macos")]
const K_CG_EVENT_LEFT_MOUSE_DOWN: CGEventType = 1;
#[cfg(target_os = "macos")]
const K_CG_EVENT_RIGHT_MOUSE_DOWN: CGEventType = 3;
#[cfg(target_os = "macos")]
const K_CG_EVENT_OTHER_MOUSE_DOWN: CGEventType = 25;
#[cfg(target_os = "macos")]
const K_CG_MOUSE_BUTTON_LEFT: CGMouseButton = 0;
#[cfg(target_os = "macos")]
const K_CG_MOUSE_BUTTON_RIGHT: CGMouseButton = 1;
#[cfg(target_os = "macos")]
const K_CG_MOUSE_BUTTON_CENTER: CGMouseButton = 2;

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct MacPoint {
    x: f64,
    y: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MacSize {
    width: f64,
    height: f64,
}

#[cfg(target_os = "macos")]
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct MacRect {
    origin: MacPoint,
    size: MacSize,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MacWindowCandidate {
    window_id: u32,
    owner_pid: i32,
    bounds_origin: MacPoint,
    bounds_size: MacSize,
    owner_name: String,
    window_name: String,
    layer: i64,
}

#[cfg(target_os = "macos")]
#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: *const c_void) -> *const c_void;
    fn CGEventGetLocation(event: *const c_void) -> MacPoint;
    fn CGEventSourceButtonState(state_id: CGEventSourceStateID, button: CGMouseButton) -> bool;
    fn CGEventSourceCounterForEventType(
        state_id: CGEventSourceStateID,
        event_type: CGEventType,
    ) -> u32;
    fn CGWindowListCopyWindowInfo(option: u32, relative_to_window: u32) -> CFArrayRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut MacRect) -> u8;

    static kCGWindowBounds: CFStringRef;
    static kCGWindowLayer: CFStringRef;
    static kCGWindowName: CFStringRef;
    static kCGWindowNumber: CFStringRef;
    static kCGWindowOwnerName: CFStringRef;
    static kCGWindowOwnerPID: CFStringRef;
}

#[cfg(target_os = "macos")]
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: CFArrayRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFArrayRef, index: isize) -> *const c_void;
    fn CFDictionaryGetTypeID() -> usize;
    fn CFDictionaryGetValueIfPresent(
        dict: CFDictionaryRef,
        key: *const c_void,
        value: *mut *const c_void,
    ) -> u8;
    fn CFGetTypeID(value: *const c_void) -> usize;
    fn CFNumberGetTypeID() -> usize;
    fn CFNumberGetValue(number: *const c_void, number_type: i32, value: *mut c_void) -> u8;
    fn CFStringGetCString(
        string: CFStringRef,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFStringGetLength(string: CFStringRef) -> isize;
    fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
    fn CFStringGetTypeID() -> usize;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WindowInfo {
    #[cfg(target_os = "windows")]
    pub hwnd: isize,
    #[cfg(target_os = "macos")]
    pub window_id: u32,
    #[cfg(target_os = "macos")]
    pub owner_pid: i32,
    pub title: String,
    pub class_name: String,
}

impl WindowInfo {
    #[cfg(target_os = "windows")]
    pub fn target_hwnd(&self) -> Option<isize> {
        Some(self.hwnd)
    }

    #[cfg(target_os = "macos")]
    pub fn target_hwnd(&self) -> Option<isize> {
        Some(self.owner_pid as isize)
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn target_hwnd(&self) -> Option<isize> {
        None
    }

    #[cfg(target_os = "windows")]
    pub fn matches_target(&self, other: &Self) -> bool {
        self.hwnd == other.hwnd
    }

    #[cfg(target_os = "macos")]
    pub fn matches_target(&self, other: &Self) -> bool {
        self.window_id == other.window_id && self.owner_pid == other.owner_pid
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    pub fn matches_target(&self, other: &Self) -> bool {
        self.title == other.title && self.class_name == other.class_name
    }
}

impl fmt::Display for WindowInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MousePressSnapshot {
    pub buttons_down: u8,
    #[cfg(target_os = "macos")]
    left_press_count: u32,
    #[cfg(target_os = "macos")]
    right_press_count: u32,
    #[cfg(target_os = "macos")]
    other_press_count: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MousePressPoll {
    pub snapshot: MousePressSnapshot,
    pub is_pressed: bool,
    pub saw_new_press: bool,
}

#[cfg(target_os = "windows")]
fn windows_mouse_button_state() -> (u8, u8) {
    let mut buttons_down = 0u8;
    let mut pressed_since_last_poll = 0u8;

    for (bit, vk) in [(1u8, VK_LBUTTON), (2u8, VK_RBUTTON), (4u8, VK_MBUTTON)] {
        let state = unsafe { GetAsyncKeyState(vk as i32) } as u16;
        if (state & 0x8000) != 0 {
            buttons_down |= bit;
        }
        if (state & 0x0001) != 0 {
            pressed_since_last_poll |= bit;
        }
    }

    (buttons_down, pressed_since_last_poll)
}

#[cfg(target_os = "windows")]
pub fn capture_mouse_press_snapshot() -> MousePressSnapshot {
    let (buttons_down, _) = windows_mouse_button_state();
    MousePressSnapshot { buttons_down }
}

#[cfg(target_os = "windows")]
pub fn poll_mouse_press(previous: MousePressSnapshot) -> MousePressPoll {
    let (buttons_down, pressed_since_last_poll) = windows_mouse_button_state();
    let snapshot = MousePressSnapshot { buttons_down };
    let saw_new_press =
        pressed_since_last_poll != 0 || (buttons_down & !previous.buttons_down) != 0;

    MousePressPoll {
        snapshot,
        is_pressed: buttons_down != 0,
        saw_new_press,
    }
}

#[cfg(target_os = "macos")]
fn macos_mouse_buttons_down() -> u8 {
    let mut buttons_down = 0u8;

    unsafe {
        if CGEventSourceButtonState(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
            K_CG_MOUSE_BUTTON_LEFT,
        ) {
            buttons_down |= 1;
        }
        if CGEventSourceButtonState(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
            K_CG_MOUSE_BUTTON_RIGHT,
        ) {
            buttons_down |= 2;
        }
        if CGEventSourceButtonState(
            K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
            K_CG_MOUSE_BUTTON_CENTER,
        ) {
            buttons_down |= 4;
        }
    }

    buttons_down
}

#[cfg(target_os = "macos")]
pub fn capture_mouse_press_snapshot() -> MousePressSnapshot {
    unsafe {
        MousePressSnapshot {
            buttons_down: macos_mouse_buttons_down(),
            left_press_count: CGEventSourceCounterForEventType(
                K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                K_CG_EVENT_LEFT_MOUSE_DOWN,
            ),
            right_press_count: CGEventSourceCounterForEventType(
                K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                K_CG_EVENT_RIGHT_MOUSE_DOWN,
            ),
            other_press_count: CGEventSourceCounterForEventType(
                K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION_STATE,
                K_CG_EVENT_OTHER_MOUSE_DOWN,
            ),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn poll_mouse_press(previous: MousePressSnapshot) -> MousePressPoll {
    let snapshot = capture_mouse_press_snapshot();
    let saw_new_press = snapshot.left_press_count > previous.left_press_count
        || snapshot.right_press_count > previous.right_press_count
        || snapshot.other_press_count > previous.other_press_count
        || (snapshot.buttons_down & !previous.buttons_down) != 0;

    MousePressPoll {
        snapshot,
        is_pressed: snapshot.buttons_down != 0,
        saw_new_press,
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn capture_mouse_press_snapshot() -> MousePressSnapshot {
    MousePressSnapshot::default()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn poll_mouse_press(_previous: MousePressSnapshot) -> MousePressPoll {
    MousePressPoll::default()
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

#[cfg(target_os = "macos")]
pub fn get_window_under_cursor() -> Option<WindowInfo> {
    let cursor = unsafe { current_mouse_location()? };
    let windows = unsafe { copy_macos_window_candidates()? };
    pick_macos_window_at_point(&windows, cursor)
}

#[cfg(target_os = "macos")]
pub fn list_visible_windows() -> Vec<WindowInfo> {
    let windows = unsafe { copy_macos_window_candidates() }.unwrap_or_default();
    windows
        .iter()
        .filter_map(macos_candidate_to_window_info)
        .collect()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn get_window_under_cursor() -> Option<WindowInfo> {
    None
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn list_visible_windows() -> Vec<WindowInfo> {
    Vec::new()
}

#[cfg(target_os = "macos")]
fn pick_macos_window_at_point(
    windows: &[MacWindowCandidate],
    cursor: MacPoint,
) -> Option<WindowInfo> {
    windows
        .iter()
        .filter(|candidate| candidate.layer == 0)
        .find(|candidate| macos_window_contains_point(candidate, cursor))
        .and_then(macos_candidate_to_window_info)
}

#[cfg(target_os = "macos")]
fn macos_candidate_to_window_info(candidate: &MacWindowCandidate) -> Option<WindowInfo> {
    let owner_name = candidate.owner_name.trim();
    let window_name = candidate.window_name.trim();
    let title = if !window_name.is_empty() {
        window_name.to_string()
    } else if !owner_name.is_empty() {
        owner_name.to_string()
    } else {
        return None;
    };

    Some(WindowInfo {
        window_id: candidate.window_id,
        owner_pid: candidate.owner_pid,
        title,
        class_name: owner_name.to_string(),
    })
}

#[cfg(target_os = "macos")]
fn macos_window_contains_point(candidate: &MacWindowCandidate, point: MacPoint) -> bool {
    let left = candidate.bounds_origin.x;
    let top = candidate.bounds_origin.y;
    let right = left + candidate.bounds_size.width;
    let bottom = top + candidate.bounds_size.height;

    point.x >= left && point.x <= right && point.y >= top && point.y <= bottom
}

#[cfg(target_os = "macos")]
unsafe fn current_mouse_location() -> Option<MacPoint> {
    let event = unsafe { CGEventCreate(ptr::null()) };
    if event.is_null() {
        return None;
    }

    let location = unsafe { CGEventGetLocation(event) };
    unsafe { CFRelease(event) };
    Some(location)
}

#[cfg(target_os = "macos")]
unsafe fn copy_macos_window_candidates() -> Option<Vec<MacWindowCandidate>> {
    let window_list = unsafe {
        CGWindowListCopyWindowInfo(
            K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY | K_CG_WINDOW_LIST_EXCLUDE_DESKTOP_ELEMENTS,
            0,
        )
    };
    if window_list.is_null() {
        return None;
    }

    let count = unsafe { CFArrayGetCount(window_list) };
    let mut results = Vec::new();
    for index in 0..count {
        let raw_value = unsafe { CFArrayGetValueAtIndex(window_list, index) };
        if raw_value.is_null() {
            continue;
        }
        let dictionary = raw_value as CFDictionaryRef;
        if unsafe { CFGetTypeID(dictionary) } != unsafe { CFDictionaryGetTypeID() } {
            continue;
        }
        if let Some(candidate) = unsafe { macos_window_candidate_from_dict(dictionary) } {
            results.push(candidate);
        }
    }

    unsafe { CFRelease(window_list) };
    Some(results)
}

#[cfg(target_os = "macos")]
unsafe fn macos_window_candidate_from_dict(
    dictionary: CFDictionaryRef,
) -> Option<MacWindowCandidate> {
    let bounds = unsafe { cf_dictionary_rect_value(dictionary, kCGWindowBounds as *const c_void) }?;
    let window_id =
        unsafe { cf_dictionary_number_value(dictionary, kCGWindowNumber as *const c_void) }? as u32;
    let owner_pid =
        unsafe { cf_dictionary_number_value(dictionary, kCGWindowOwnerPID as *const c_void) }?
            as i32;
    let layer = unsafe { cf_dictionary_number_value(dictionary, kCGWindowLayer as *const c_void) }
        .unwrap_or(0);
    let owner_name =
        unsafe { cf_dictionary_string_value(dictionary, kCGWindowOwnerName as *const c_void) }
            .unwrap_or_default();
    let window_name =
        unsafe { cf_dictionary_string_value(dictionary, kCGWindowName as *const c_void) }
            .unwrap_or_default();

    Some(MacWindowCandidate {
        window_id,
        owner_pid,
        bounds_origin: bounds.origin,
        bounds_size: bounds.size,
        owner_name,
        window_name,
        layer,
    })
}

#[cfg(target_os = "macos")]
unsafe fn cf_dictionary_string_value(
    dictionary: CFDictionaryRef,
    key: *const c_void,
) -> Option<String> {
    let value = unsafe { cf_dictionary_value(dictionary, key) }?;
    unsafe { cf_string_to_string(value as CFStringRef) }
}

#[cfg(target_os = "macos")]
unsafe fn cf_dictionary_number_value(
    dictionary: CFDictionaryRef,
    key: *const c_void,
) -> Option<i64> {
    let value = unsafe { cf_dictionary_value(dictionary, key) }?;
    if unsafe { CFGetTypeID(value) } != unsafe { CFNumberGetTypeID() } {
        return None;
    }

    let mut number = 0_i64;
    let ok = unsafe {
        CFNumberGetValue(
            value,
            K_CF_NUMBER_SINT64_TYPE,
            &mut number as *mut _ as *mut c_void,
        )
    };
    if ok == 0 {
        None
    } else {
        Some(number)
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_dictionary_rect_value(
    dictionary: CFDictionaryRef,
    key: *const c_void,
) -> Option<MacRect> {
    let value = unsafe { cf_dictionary_value(dictionary, key) }?;
    if unsafe { CFGetTypeID(value) } != unsafe { CFDictionaryGetTypeID() } {
        return None;
    }

    let mut rect = MacRect::default();
    let ok = unsafe { CGRectMakeWithDictionaryRepresentation(value as CFDictionaryRef, &mut rect) };
    if ok == 0 {
        None
    } else {
        Some(rect)
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_dictionary_value(
    dictionary: CFDictionaryRef,
    key: *const c_void,
) -> Option<*const c_void> {
    let mut value = ptr::null();
    let ok = unsafe { CFDictionaryGetValueIfPresent(dictionary, key, &mut value) };
    if ok == 0 || value.is_null() {
        None
    } else {
        Some(value)
    }
}

#[cfg(target_os = "macos")]
unsafe fn cf_string_to_string(string: CFStringRef) -> Option<String> {
    if string.is_null() || unsafe { CFGetTypeID(string) } != unsafe { CFStringGetTypeID() } {
        return None;
    }

    let length = unsafe { CFStringGetLength(string) };
    let buffer_len =
        unsafe { CFStringGetMaximumSizeForEncoding(length, K_CF_STRING_ENCODING_UTF8) } + 1;
    let mut buffer = vec![0_u8; buffer_len as usize];
    let ok = unsafe {
        CFStringGetCString(
            string,
            buffer.as_mut_ptr() as *mut c_char,
            buffer_len,
            K_CF_STRING_ENCODING_UTF8,
        )
    };
    if ok == 0 {
        return None;
    }

    let string_len = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf8_lossy(&buffer[..string_len]).into_owned())
}

#[cfg(test)]
mod tests {
    use super::WindowInfo;
    #[cfg(target_os = "macos")]
    use super::{pick_macos_window_at_point, MacPoint, MacWindowCandidate};

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn non_windows_window_info_has_no_target_handle() {
        let info = WindowInfo {
            title: "Example".to_string(),
            class_name: "Demo".to_string(),
        };

        assert_eq!(info.target_hwnd(), None);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    #[test]
    fn non_windows_window_match_uses_title_and_class() {
        let first = WindowInfo {
            title: "Example".to_string(),
            class_name: "Demo".to_string(),
        };
        let second = WindowInfo {
            title: "Example".to_string(),
            class_name: "Demo".to_string(),
        };
        let third = WindowInfo {
            title: "Other".to_string(),
            class_name: "Demo".to_string(),
        };

        assert!(first.matches_target(&second));
        assert!(!first.matches_target(&third));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_pick_returns_frontmost_window_containing_cursor() {
        let windows = vec![
            MacWindowCandidate {
                window_id: 2,
                owner_pid: 222,
                bounds_origin: MacPoint { x: 100.0, y: 100.0 },
                bounds_size: super::MacSize {
                    width: 200.0,
                    height: 200.0,
                },
                owner_name: "Front".to_string(),
                window_name: "Front Window".to_string(),
                layer: 0,
            },
            MacWindowCandidate {
                window_id: 1,
                owner_pid: 111,
                bounds_origin: MacPoint { x: 0.0, y: 0.0 },
                bounds_size: super::MacSize {
                    width: 500.0,
                    height: 500.0,
                },
                owner_name: "Background".to_string(),
                window_name: "Background Window".to_string(),
                layer: 0,
            },
        ];

        let info = pick_macos_window_at_point(&windows, MacPoint { x: 150.0, y: 150.0 })
            .expect("expected to pick frontmost window");

        assert_eq!(info.title, "Front Window");
        assert_eq!(info.class_name, "Front");
        assert_eq!(info.target_hwnd(), Some(222));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_pick_ignores_non_normal_layers() {
        let windows = vec![
            MacWindowCandidate {
                window_id: 10,
                owner_pid: 310,
                bounds_origin: MacPoint { x: 0.0, y: 0.0 },
                bounds_size: super::MacSize {
                    width: 200.0,
                    height: 200.0,
                },
                owner_name: "Overlay".to_string(),
                window_name: "Tooltip".to_string(),
                layer: 25,
            },
            MacWindowCandidate {
                window_id: 11,
                owner_pid: 311,
                bounds_origin: MacPoint { x: 0.0, y: 0.0 },
                bounds_size: super::MacSize {
                    width: 200.0,
                    height: 200.0,
                },
                owner_name: "Editor".to_string(),
                window_name: "Document".to_string(),
                layer: 0,
            },
        ];

        let info = pick_macos_window_at_point(&windows, MacPoint { x: 50.0, y: 50.0 })
            .expect("expected to ignore overlay and pick normal window");

        assert_eq!(info.title, "Document");
        assert_eq!(info.class_name, "Editor");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_window_match_uses_window_id_and_pid() {
        let first = WindowInfo {
            window_id: 7,
            owner_pid: 700,
            title: "Editor".to_string(),
            class_name: "Code".to_string(),
        };
        let same = WindowInfo {
            window_id: 7,
            owner_pid: 700,
            title: "Editor".to_string(),
            class_name: "Code".to_string(),
        };
        let other_window = WindowInfo {
            window_id: 8,
            owner_pid: 700,
            title: "Editor".to_string(),
            class_name: "Code".to_string(),
        };

        assert!(first.matches_target(&same));
        assert!(!first.matches_target(&other_window));
    }
}
