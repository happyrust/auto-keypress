use std::collections::HashMap;
use std::time::Duration;

use global_hotkey::{hotkey::HotKey, GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::{
    prelude::{
        FluentBuilder, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
        Styled,
    },
    rgb, AppContext, Context, Entity, FocusHandle, KeyDownEvent, MouseButton, Render, WeakEntity,
    Window,
};
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex, Sizable};

use crate::i18n::{Language, UiText};
use crate::key_sender::{SendMode, VirtualKey};
use crate::scheduler::{KeyTask, Scheduler, SendStats};
use crate::window_picker::{MousePressSnapshot, WindowInfo};

#[cfg(target_os = "windows")]
fn start_window_drag(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{PostMessageA, HTCAPTION, WM_NCLBUTTONDOWN},
    };
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            let hwnd = h.hwnd.get() as *mut std::ffi::c_void;
            unsafe {
                ReleaseCapture();
                PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn start_window_drag(_window: &Window) {}

#[cfg(target_os = "windows")]
fn set_always_on_top(window: &Window, on_top: bool) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SetWindowPos, HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
    };
    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            let hwnd = h.hwnd.get() as *mut std::ffi::c_void;
            let insert_after = if on_top { HWND_TOPMOST } else { HWND_NOTOPMOST };
            unsafe {
                SetWindowPos(hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
        }
    }
}

#[cfg(target_os = "macos")]
const NS_NORMAL_WINDOW_LEVEL: isize = 0;
#[cfg(target_os = "macos")]
const NS_POP_UP_WINDOW_LEVEL: isize = 101;

#[cfg(target_os = "macos")]
fn mac_window_level(on_top: bool) -> isize {
    if on_top {
        NS_POP_UP_WINDOW_LEVEL
    } else {
        NS_NORMAL_WINDOW_LEVEL
    }
}

#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)]
fn set_always_on_top(window: &Window, on_top: bool) {
    use objc::{msg_send, runtime::Object, sel, sel_impl};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if let Ok(handle) = HasWindowHandle::window_handle(window) {
        if let RawWindowHandle::AppKit(h) = handle.as_raw() {
            let ns_view = h.ns_view.as_ptr() as *mut Object;
            unsafe {
                let ns_window: *mut Object = msg_send![ns_view, window];
                if !ns_window.is_null() {
                    let _: () = msg_send![ns_window, setLevel: mac_window_level(on_top)];
                }
            }
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn set_always_on_top(_window: &Window, _on_top: bool) {}

const BG: u32 = 0x2D2D2D;
const SECONDARY: u32 = 0x3A3A3A;
const BORDER: u32 = 0x4D4D4D;
const PRIMARY: u32 = 0x6B8CFF;
const FG: u32 = 0xFFFFFF;
const MUTED: u32 = 0x999999;
const DANGER: u32 = 0xFF5C5C;
const SUCCESS: u32 = 0x4CAF50;
const INPUT_BG: u32 = 0x353535;
const WARN: u32 = 0xFFA500;

static NEXT_TASK_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

fn next_id() -> u32 {
    NEXT_TASK_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn can_start(is_running: bool, has_target: bool, accessibility_ready: bool) -> bool {
    !is_running && has_target && accessibility_ready
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartBlockReason {
    MissingPermission,
    MissingTarget,
}

fn start_block_reason(
    is_running: bool,
    has_target: bool,
    accessibility_ready: bool,
) -> Option<StartBlockReason> {
    if is_running {
        None
    } else if !accessibility_ready {
        Some(StartBlockReason::MissingPermission)
    } else if !has_target {
        Some(StartBlockReason::MissingTarget)
    } else {
        None
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PickFrame<T> {
    target: Option<T>,
    ready_to_confirm: bool,
    stop_picking: bool,
    changed: bool,
}

fn advance_pick_frame<T: Clone>(
    current_target: Option<&T>,
    hovered_target: Option<T>,
    ready_to_confirm: bool,
    mouse_pressed: bool,
    mouse_clicked: bool,
    same_target: impl Fn(&T, &T) -> bool,
) -> PickFrame<T> {
    let target = hovered_target.or_else(|| current_target.cloned());
    let changed = match (current_target, target.as_ref()) {
        (Some(current), Some(next)) => !same_target(current, next),
        (None, None) => false,
        _ => true,
    };
    let ready_to_confirm = ready_to_confirm || !mouse_pressed;
    let stop_picking = ready_to_confirm && (mouse_pressed || mouse_clicked);

    PickFrame {
        target,
        ready_to_confirm,
        stop_picking,
        changed,
    }
}

fn keystroke_to_vk(key: &str) -> Option<VirtualKey> {
    let upper = key.to_uppercase();
    match upper.as_str() {
        "SPACE" | " " => Some(VirtualKey(0x20)),
        "ENTER" | "RETURN" => Some(VirtualKey(0x0D)),
        "TAB" => Some(VirtualKey(0x09)),
        "ESCAPE" => Some(VirtualKey(0x1B)),
        "BACKSPACE" => Some(VirtualKey(0x08)),
        "DELETE" => Some(VirtualKey(0x2E)),
        "UP" => Some(VirtualKey(0x26)),
        "DOWN" => Some(VirtualKey(0x28)),
        "LEFT" => Some(VirtualKey(0x25)),
        "RIGHT" => Some(VirtualKey(0x27)),
        "HOME" => Some(VirtualKey(0x24)),
        "END" => Some(VirtualKey(0x23)),
        "PAGEUP" => Some(VirtualKey(0x21)),
        "PAGEDOWN" => Some(VirtualKey(0x22)),
        "INSERT" => Some(VirtualKey(0x2D)),
        s if s.len() == 1 => {
            let ch = s.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                Some(VirtualKey(ch as u16))
            } else if ch.is_ascii_digit() {
                Some(VirtualKey(ch as u16))
            } else {
                None
            }
        }
        s if s.starts_with('F') && s.len() <= 3 => {
            if let Ok(n) = s[1..].parse::<u16>() {
                if (1..=12).contains(&n) {
                    Some(VirtualKey(0x6F + n))
                } else {
                    None
                }
            } else {
                None
            }
        }
        _ => None,
    }
}

pub struct AppState {
    pub focus_handle: FocusHandle,
    pub target_window: Option<WindowInfo>,
    pub key_tasks: Vec<KeyTask>,
    pub interval_inputs: HashMap<u32, Entity<InputState>>,
    pub recording_task_id: Option<u32>,
    pub send_mode: SendMode,
    pub scheduler: Option<Scheduler>,
    pub is_running: bool,
    pub is_picking: bool,
    pub pick_ready_to_confirm: bool,
    pub pick_mouse_snapshot: MousePressSnapshot,
    pub stats: SendStats,
    pub language: Language,
    pub always_on_top: bool,
    #[allow(dead_code)]
    hotkey_manager: Option<GlobalHotKeyManager>,
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let cfg = crate::config::load_config();

        let mut key_tasks = Vec::new();
        let mut interval_inputs = HashMap::new();

        for tc in &cfg.tasks {
            let tid = next_id();
            key_tasks.push(KeyTask {
                id: tid,
                vk: VirtualKey(tc.vk),
                interval_ms: tc.interval_ms,
            });
            let input =
                cx.new(|cx| InputState::new(window, cx).placeholder(&tc.interval_ms.to_string()));
            interval_inputs.insert(tid, input);
        }

        if key_tasks.is_empty() {
            let tid = next_id();
            key_tasks.push(KeyTask {
                id: tid,
                vk: VirtualKey(0x0D),
                interval_ms: 200,
            });
            let input = cx.new(|cx| InputState::new(window, cx).placeholder("200"));
            interval_inputs.insert(tid, input);
        }

        let hotkey_manager = Self::register_global_hotkey();
        Self::start_hotkey_listener(cx);
        Self::start_stats_timer(cx);

        if cfg.always_on_top {
            set_always_on_top(window, true);
        }

        Self {
            focus_handle,
            target_window: None,
            key_tasks,
            interval_inputs,
            recording_task_id: None,
            send_mode: cfg.send_mode_enum(),
            scheduler: None,
            is_running: false,
            is_picking: false,
            pick_ready_to_confirm: false,
            pick_mouse_snapshot: MousePressSnapshot::default(),
            stats: SendStats::default(),
            language: cfg.language_enum(),
            always_on_top: cfg.always_on_top,
            hotkey_manager,
        }
    }

    pub fn save_config(&self, cx: &Context<Self>) {
        self.sync_intervals_snapshot(cx);
        let tasks: Vec<(VirtualKey, u64)> = self
            .key_tasks
            .iter()
            .map(|t| (t.vk, t.interval_ms))
            .collect();
        let cfg = crate::config::config_from_state(
            self.send_mode,
            self.language,
            self.always_on_top,
            &tasks,
        );
        crate::config::save_config(&cfg);
    }

    fn sync_intervals_snapshot(&self, cx: &Context<Self>) {
        for task in &self.key_tasks {
            if let Some(input_entity) = self.interval_inputs.get(&task.id) {
                let text = input_entity.read(cx).text().to_string();
                if let Ok(ms) = text.trim().parse::<u64>() {
                    if ms > 0 {
                        // interval_ms is on the task which we iterate immutably here
                        // actual sync happens in sync_intervals_from_inputs
                    }
                    let _ = ms;
                }
            }
        }
    }

    fn register_global_hotkey() -> Option<GlobalHotKeyManager> {
        let mgr = GlobalHotKeyManager::new().ok()?;
        match "F9".parse::<HotKey>() {
            Ok(hk) => {
                if let Err(e) = mgr.register(hk) {
                    tracing::warn!("Failed to register F9 hotkey: {e}");
                    return None;
                }
                tracing::info!("Global hotkey F9 registered (toggle start/stop)");
                Some(mgr)
            }
            Err(e) => {
                tracing::warn!("Failed to parse hotkey: {e}");
                None
            }
        }
    }

    fn start_hotkey_listener(cx: &mut Context<Self>) {
        let hotkey_rx = GlobalHotKeyEvent::receiver().clone();
        cx.spawn(async move |entity: WeakEntity<Self>, async_app| loop {
            let rx = hotkey_rx.clone();
            let event = async_app
                .background_executor()
                .spawn(async move { rx.recv().ok() })
                .await;
            match event {
                Some(ev) if ev.state() == HotKeyState::Pressed => {
                    let ok = entity
                        .update(async_app, |state, cx| {
                            state.toggle_running(cx);
                            cx.notify();
                        })
                        .is_ok();
                    if !ok {
                        break;
                    }
                }
                None => break,
                _ => {}
            }
        })
        .detach();
    }

    fn start_stats_timer(cx: &mut Context<Self>) {
        cx.spawn(async move |entity: WeakEntity<Self>, async_app| loop {
            async_app
                .background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let ok = entity
                .update(async_app, |state, cx| {
                    if state.is_running {
                        state.refresh_stats();
                        cx.notify();
                    }
                })
                .is_ok();
            if !ok {
                break;
            }
        })
        .detach();
    }

    pub fn toggle_running(&mut self, cx: &Context<Self>) {
        if self.is_running {
            self.stop();
            self.save_current_config(cx);
        } else {
            self.start(cx);
        }
    }

    fn save_current_config(&mut self, cx: &Context<Self>) {
        self.sync_intervals_from_inputs(cx);
        let tasks: Vec<(VirtualKey, u64)> = self
            .key_tasks
            .iter()
            .map(|t| (t.vk, t.interval_ms))
            .collect();
        let cfg = crate::config::config_from_state(
            self.send_mode,
            self.language,
            self.always_on_top,
            &tasks,
        );
        crate::config::save_config(&cfg);
    }

    pub fn add_key_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let tid = next_id();
        self.key_tasks.push(KeyTask {
            id: tid,
            vk: VirtualKey(0x0D),
            interval_ms: 200,
        });
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("200"));
        self.interval_inputs.insert(tid, input);
    }

    pub fn remove_key_task(&mut self, task_id: u32) {
        self.key_tasks.retain(|t| t.id != task_id);
        self.interval_inputs.remove(&task_id);
    }

    fn sync_intervals_from_inputs(&mut self, cx: &Context<Self>) {
        for task in &mut self.key_tasks {
            if let Some(input_entity) = self.interval_inputs.get(&task.id) {
                let text = input_entity.read(cx).text().to_string();
                if let Ok(ms) = text.trim().parse::<u64>() {
                    if ms > 0 {
                        task.interval_ms = ms;
                    }
                }
            }
        }
    }

    pub fn start(&mut self, cx: &Context<Self>) {
        if !crate::key_sender::accessibility_trusted() {
            return;
        }

        let hwnd = match &self.target_window {
            Some(w) => match w.target_hwnd() {
                Some(hwnd) => hwnd,
                None => return,
            },
            None => return,
        };
        if self.key_tasks.is_empty() {
            return;
        }

        self.sync_intervals_from_inputs(cx);

        let mut scheduler = Scheduler::new(hwnd, self.key_tasks.clone(), self.send_mode);
        scheduler.start();
        self.scheduler = Some(scheduler);
        self.is_running = true;
    }

    pub fn stop(&mut self) {
        if let Some(mut scheduler) = self.scheduler.take() {
            self.stats = scheduler.stats();
            scheduler.stop();
        }
        self.is_running = false;
    }

    pub fn toggle_pick(&mut self, cx: &mut Context<Self>) {
        self.is_picking = !self.is_picking;
        self.pick_ready_to_confirm = false;
        if self.is_picking {
            self.pick_mouse_snapshot = crate::window_picker::capture_mouse_press_snapshot();
            self.start_picking(cx);
        } else {
            self.pick_mouse_snapshot = MousePressSnapshot::default();
        }
    }

    pub fn start_recording(&mut self, task_id: u32) {
        self.recording_task_id = Some(task_id);
    }

    pub fn handle_key_record(&mut self, event: &KeyDownEvent) {
        let task_id = match self.recording_task_id {
            Some(id) => id,
            None => return,
        };

        let key_str = event.keystroke.key.as_str();
        if let Some(vk) = keystroke_to_vk(key_str) {
            if let Some(task) = self.key_tasks.iter_mut().find(|t| t.id == task_id) {
                task.vk = vk;
            }
        }
        self.recording_task_id = None;
    }

    fn start_picking(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |entity: WeakEntity<Self>, async_app| loop {
            async_app
                .background_executor()
                .timer(Duration::from_millis(100))
                .await;

            let should_continue = entity
                .update(async_app, |state, cx| {
                    if !state.is_picking {
                        return false;
                    }
                    let mouse_poll =
                        crate::window_picker::poll_mouse_press(state.pick_mouse_snapshot);
                    let frame = advance_pick_frame(
                        state.target_window.as_ref(),
                        crate::window_picker::get_window_under_cursor(),
                        state.pick_ready_to_confirm,
                        mouse_poll.is_pressed,
                        mouse_poll.saw_new_press,
                        |current, next| current.matches_target(next),
                    );
                    state.pick_mouse_snapshot = mouse_poll.snapshot;

                    if frame.changed {
                        state.target_window = frame.target;
                    }

                    if frame.stop_picking {
                        state.is_picking = false;
                        state.pick_ready_to_confirm = false;
                        state.pick_mouse_snapshot = MousePressSnapshot::default();
                    } else {
                        state.pick_ready_to_confirm = frame.ready_to_confirm;
                    }

                    if frame.changed || frame.stop_picking {
                        cx.notify();
                    }

                    !frame.stop_picking
                })
                .unwrap_or(false);

            if !should_continue {
                break;
            }
        })
        .detach();
    }

    fn refresh_stats(&mut self) {
        if let Some(scheduler) = &self.scheduler {
            self.stats = scheduler.stats();
        }
    }
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.is_running {
            self.refresh_stats();
        }

        let text = self.language.labels();
        let target_text = self
            .target_window
            .as_ref()
            .map_or(text.no_window_selected.to_string(), |w| w.title.clone());

        let pick_label = if self.is_picking {
            text.stop
        } else {
            text.pick
        };
        let is_recording = self.recording_task_id.is_some();

        v_flex()
            .id("auto-keypress")
            .track_focus(&self.focus_handle)
            .size_full()
            .bg(rgb(BG))
            .when(is_recording, |el| {
                el.on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                    this.handle_key_record(event);
                    cx.notify();
                }))
            })
            .child(self.render_header(text, cx))
            .child(self.render_body(text, &target_text, pick_label, cx))
            .child(self.render_status_bar(text))
    }
}

impl AppState {
    fn render_header(&self, text: &'static UiText, cx: &Context<Self>) -> impl IntoElement {
        h_flex()
            .id("header-drag")
            .w_full()
            .h(gpui::px(48.))
            .px(gpui::px(16.))
            .items_center()
            .justify_between()
            .border_b_1()
            .border_color(rgb(BORDER))
            .on_mouse_down(MouseButton::Left, |_, window, _cx| {
                start_window_drag(window);
            })
            .child(
                h_flex().gap(gpui::px(8.)).items_center().child(
                    gpui::div()
                        .text_size(gpui::px(16.))
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(FG))
                        .child(text.app_title),
                ),
            )
            .child(
                h_flex()
                    .gap(gpui::px(4.))
                    .items_center()
                    .child(self.render_language_switch(cx))
                    .child(
                        gpui::div()
                            .id("pin-btn")
                            .cursor_pointer()
                            .w(gpui::px(28.))
                            .h(gpui::px(28.))
                            .rounded(gpui::px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(self.always_on_top, |s| s.bg(rgb(PRIMARY)))
                            .when(!self.always_on_top, |s| s.hover(|s| s.bg(rgb(SECONDARY))))
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(12.))
                                    .text_color(rgb(if self.always_on_top { FG } else { MUTED }))
                                    .child("📌"),
                            )
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.always_on_top = !this.always_on_top;
                                set_always_on_top(window, this.always_on_top);
                                this.save_current_config(cx);
                                cx.notify();
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        gpui::div()
                            .id("min-btn")
                            .cursor_pointer()
                            .w(gpui::px(28.))
                            .h(gpui::px(28.))
                            .rounded(gpui::px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(rgb(SECONDARY)))
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(14.))
                                    .text_color(rgb(MUTED))
                                    .child("—"),
                            )
                            .on_click(|_, window, _cx| {
                                window.minimize_window();
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        gpui::div()
                            .id("close-btn")
                            .cursor_pointer()
                            .w(gpui::px(28.))
                            .h(gpui::px(28.))
                            .rounded(gpui::px(6.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .hover(|s| s.bg(rgb(DANGER)))
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(14.))
                                    .text_color(rgb(MUTED))
                                    .child("×"),
                            )
                            .on_click(cx.listener(|_this, _, _window, cx| {
                                cx.quit();
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                    ),
            )
    }

    fn render_body(
        &self,
        text: &'static UiText,
        target_text: &str,
        pick_label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_picking = self.is_picking;
        let is_running = self.is_running;
        let has_target = self.target_window.is_some();
        let accessibility_ready = crate::key_sender::accessibility_trusted();
        let tasks = self.key_tasks.clone();

        v_flex()
            .w_full()
            .flex_grow()
            .p(gpui::px(16.))
            .gap(gpui::px(16.))
            .child(self.render_target_section(text, target_text, pick_label, is_picking, cx))
            .when(!accessibility_ready, |el| {
                el.child(self.render_accessibility_notice(text, cx))
            })
            .child(self.render_keys_section(text, &tasks, cx))
            .child(self.render_mode_selector(text, cx))
            .child(gpui::div().w_full().h(gpui::px(1.)).bg(rgb(BORDER)))
            .child(self.render_controls(text, is_running, has_target, cx))
    }

    fn render_target_section(
        &self,
        text: &'static UiText,
        target_text: &str,
        pick_label: &str,
        is_picking: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let target_owned = target_text.to_string();
        v_flex()
            .w_full()
            .gap(gpui::px(8.))
            .child(
                gpui::div()
                    .text_size(gpui::px(12.))
                    .text_color(rgb(MUTED))
                    .child(text.target_window),
            )
            .child(
                h_flex()
                    .w_full()
                    .h(gpui::px(40.))
                    .gap(gpui::px(8.))
                    .items_center()
                    .child(
                        h_flex()
                            .flex_grow()
                            .h_full()
                            .bg(rgb(INPUT_BG))
                            .rounded(gpui::px(8.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .px(gpui::px(12.))
                            .items_center()
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(13.))
                                    .text_color(rgb(if self.target_window.is_some() {
                                        FG
                                    } else {
                                        MUTED
                                    }))
                                    .overflow_x_hidden()
                                    .child(target_owned),
                            ),
                    )
                    .child(
                        gpui::div()
                            .id("pick-btn")
                            .cursor_pointer()
                            .h_full()
                            .px(gpui::px(14.))
                            .bg(rgb(if is_picking { DANGER } else { PRIMARY }))
                            .rounded(gpui::px(8.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(13.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(FG))
                                    .child(pick_label.to_string()),
                            )
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_pick(cx);
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_accessibility_notice(
        &self,
        text: &'static UiText,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .w_full()
            .gap(gpui::px(8.))
            .p(gpui::px(12.))
            .bg(rgb(0x4A3320))
            .rounded(gpui::px(8.))
            .border_1()
            .border_color(rgb(WARN))
            .child(
                gpui::div()
                    .text_size(gpui::px(12.))
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(rgb(0xFFD08A))
                    .child(text.accessibility_required),
            )
            .child(
                gpui::div()
                    .text_size(gpui::px(11.))
                    .line_height(gpui::px(16.))
                    .text_color(rgb(0xFFE1B8))
                    .child(text.accessibility_hint),
            )
            .child(
                gpui::div()
                    .id("open-accessibility-settings-btn")
                    .cursor_pointer()
                    .h(gpui::px(30.))
                    .rounded(gpui::px(6.))
                    .px(gpui::px(10.))
                    .bg(rgb(WARN))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        gpui::div()
                            .text_size(gpui::px(11.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(BG))
                            .child(text.open_settings),
                    )
                    .on_click(cx.listener(|_this, _, _window, _cx| {
                        let _ = crate::key_sender::open_accessibility_settings();
                    })),
            )
    }

    fn render_keys_section(
        &self,
        text: &'static UiText,
        tasks: &[KeyTask],
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let mut section = v_flex().w_full().gap(gpui::px(8.));

        section = section.child(
            h_flex()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    gpui::div()
                        .text_size(gpui::px(12.))
                        .text_color(rgb(MUTED))
                        .child(text.key_tasks),
                )
                .child(
                    gpui::div()
                        .id("add-btn")
                        .cursor_pointer()
                        .h(gpui::px(28.))
                        .px(gpui::px(10.))
                        .bg(rgb(SECONDARY))
                        .rounded(gpui::px(6.))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .flex()
                        .items_center()
                        .gap(gpui::px(4.))
                        .child(
                            gpui::div()
                                .text_size(gpui::px(12.))
                                .text_color(rgb(FG))
                                .child(text.add_key),
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_key_task(window, cx);
                            cx.notify();
                        })),
                ),
        );

        let mut list = v_flex().w_full().gap(gpui::px(6.));
        for task in tasks {
            list = list.child(self.render_key_row(text, task, cx));
        }
        section = section.child(list);

        section
    }

    fn render_key_row(
        &self,
        text: &'static UiText,
        task: &KeyTask,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let task_id = task.id;
        let key_name = task.vk.name().to_string();
        let is_recording = self.recording_task_id == Some(task_id);

        let badge_bg = if is_recording { WARN } else { SECONDARY };
        let badge_text = if is_recording {
            text.press_key
        } else {
            &key_name
        };

        let mut row = h_flex()
            .id(("row", task_id))
            .w_full()
            .h(gpui::px(44.))
            .bg(rgb(INPUT_BG))
            .rounded(gpui::px(8.))
            .border_1()
            .border_color(rgb(BORDER))
            .px(gpui::px(12.))
            .gap(gpui::px(10.))
            .items_center();

        row = row.child(
            gpui::div()
                .id(("key-badge", task_id))
                .cursor_pointer()
                .min_w(gpui::px(if is_recording { 64. } else { 40. }))
                .h(gpui::px(28.))
                .bg(rgb(badge_bg))
                .rounded(gpui::px(6.))
                .border_1()
                .border_color(rgb(BORDER))
                .flex()
                .items_center()
                .justify_center()
                .px(gpui::px(6.))
                .child(
                    gpui::div()
                        .text_size(gpui::px(if is_recording { 11. } else { 13. }))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(if is_recording { BG } else { FG }))
                        .child(badge_text.to_string()),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    if this.recording_task_id == Some(task_id) {
                        this.recording_task_id = None;
                    } else {
                        this.recording_task_id = Some(task_id);
                    }
                    cx.notify();
                })),
        );

        row = row
            .child(gpui::div().w(gpui::px(1.)).h(gpui::px(20.)).bg(rgb(BORDER)))
            .child(
                gpui::div()
                    .text_size(gpui::px(11.))
                    .text_color(rgb(MUTED))
                    .child(text.interval),
            );

        if let Some(input_entity) = self.interval_inputs.get(&task_id) {
            row = row.child(
                gpui::div()
                    .w(gpui::px(80.))
                    .child(Input::new(input_entity).appearance(false).small()),
            );
        }

        row = row.child(
            gpui::div()
                .text_size(gpui::px(11.))
                .text_color(rgb(MUTED))
                .child(text.milliseconds),
        );

        row = row.child(gpui::div().flex_grow());

        row = row.child(
            gpui::div()
                .id(("del", task_id))
                .cursor_pointer()
                .w(gpui::px(28.))
                .h(gpui::px(28.))
                .rounded(gpui::px(6.))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    gpui::div()
                        .text_size(gpui::px(14.))
                        .text_color(rgb(DANGER))
                        .child("×"),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.remove_key_task(task_id);
                    cx.notify();
                })),
        );

        row
    }

    fn render_mode_selector(&self, text: &'static UiText, cx: &Context<Self>) -> impl IntoElement {
        let current = self.send_mode;
        let modes = SendMode::all();

        let mut tabs = h_flex()
            .h(gpui::px(28.))
            .bg(rgb(SECONDARY))
            .rounded(gpui::px(6.))
            .border_1()
            .border_color(rgb(BORDER));

        for (i, mode) in modes.iter().enumerate() {
            let m = *mode;
            let active = current == m;
            let is_first = i == 0;
            let is_last = i == modes.len() - 1;

            let mut tab = gpui::div()
                .id(("mode", i))
                .cursor_pointer()
                .h_full()
                .px(gpui::px(8.))
                .flex()
                .items_center()
                .bg(rgb(if active { PRIMARY } else { SECONDARY }));

            if is_first {
                tab = tab.rounded_l(gpui::px(5.));
            }
            if is_last {
                tab = tab.rounded_r(gpui::px(5.));
            }

            tab = tab.child(
                gpui::div()
                    .text_size(gpui::px(10.))
                    .text_color(rgb(if active { FG } else { MUTED }))
                    .child(m.label().to_string()),
            );

            tab = tab.on_click(cx.listener(move |this, _, _window, cx| {
                this.send_mode = m;
                this.save_current_config(cx);
                cx.notify();
            }));

            tabs = tabs.child(tab);
        }

        h_flex()
            .w_full()
            .gap(gpui::px(8.))
            .items_center()
            .child(
                gpui::div()
                    .text_size(gpui::px(12.))
                    .text_color(rgb(MUTED))
                    .child(text.send_mode),
            )
            .child(gpui::div().flex_grow())
            .child(tabs)
    }

    fn render_controls(
        &self,
        text: &'static UiText,
        is_running: bool,
        has_target: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let accessibility_ready = crate::key_sender::accessibility_trusted();
        let start_enabled = can_start(is_running, has_target, accessibility_ready);
        let block_reason = start_block_reason(is_running, has_target, accessibility_ready);
        let block_message = match block_reason {
            Some(StartBlockReason::MissingPermission) => Some(text.start_needs_permission),
            Some(StartBlockReason::MissingTarget) => Some(text.start_needs_window),
            None => None,
        };

        v_flex()
            .w_full()
            .gap(gpui::px(6.))
            .child(
                h_flex()
                    .w_full()
                    .gap(gpui::px(10.))
                    .items_center()
                    .child(
                        gpui::div()
                            .id("start-btn")
                            .cursor_pointer()
                            .flex_grow()
                            .h(gpui::px(40.))
                            .bg(rgb(if is_running {
                                SECONDARY
                            } else if start_enabled {
                                PRIMARY
                            } else {
                                INPUT_BG
                            }))
                            .rounded(gpui::px(8.))
                            .border_1()
                            .border_color(rgb(if start_enabled { PRIMARY } else { BORDER }))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(gpui::px(8.))
                            .when(!is_running, |el| {
                                el.child(
                                    gpui::div()
                                        .text_size(gpui::px(14.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(rgb(if start_enabled { FG } else { MUTED }))
                                        .child(text.start),
                                )
                            })
                            .when(is_running, |el| {
                                el.child(
                                    gpui::div()
                                        .text_size(gpui::px(14.))
                                        .font_weight(gpui::FontWeight::MEDIUM)
                                        .text_color(rgb(MUTED))
                                        .child(text.running),
                                )
                            })
                            .on_click(cx.listener(|this, _, _window, cx| {
                                if can_start(
                                    this.is_running,
                                    this.target_window.is_some(),
                                    crate::key_sender::accessibility_trusted(),
                                ) {
                                    this.start(cx);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        gpui::div()
                            .id("stop-btn")
                            .cursor_pointer()
                            .flex_grow()
                            .h(gpui::px(40.))
                            .bg(rgb(SECONDARY))
                            .rounded(gpui::px(8.))
                            .border_1()
                            .border_color(rgb(BORDER))
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(gpui::px(8.))
                            .child(
                                gpui::div()
                                    .text_size(gpui::px(14.))
                                    .font_weight(gpui::FontWeight::MEDIUM)
                                    .text_color(rgb(FG))
                                    .child(text.stop),
                            )
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.stop();
                                this.save_current_config(cx);
                                cx.notify();
                            })),
                    ),
            )
            .when(block_message.is_some(), |el| {
                el.child(
                    gpui::div()
                        .text_size(gpui::px(11.))
                        .text_color(rgb(WARN))
                        .child(block_message.unwrap_or_default()),
                )
            })
    }

    fn render_status_bar(&self, text: &'static UiText) -> impl IntoElement {
        let accessibility_ready = crate::key_sender::accessibility_trusted();
        let status_text = if !accessibility_ready {
            text.accessibility_required
        } else if self.is_running {
            text.running_status
        } else {
            text.ready
        };
        let status_color = if !accessibility_ready {
            WARN
        } else if self.is_running {
            SUCCESS
        } else {
            MUTED
        };

        let mut stats_parts: Vec<String> = Vec::new();
        for task in &self.key_tasks {
            let count = self.stats.counts.get(&task.id).copied().unwrap_or(0);
            stats_parts.push(format!("{}×{}", task.vk.name(), count));
        }
        let stats_text = if stats_parts.is_empty() {
            String::new()
        } else {
            stats_parts.join("  ")
        };

        h_flex()
            .w_full()
            .h(gpui::px(36.))
            .px(gpui::px(16.))
            .items_center()
            .justify_between()
            .bg(rgb(0x252525))
            .border_t_1()
            .border_color(rgb(BORDER))
            .child(
                h_flex()
                    .gap(gpui::px(6.))
                    .items_center()
                    .child(
                        gpui::div()
                            .w(gpui::px(8.))
                            .h(gpui::px(8.))
                            .rounded(gpui::px(4.))
                            .bg(rgb(status_color)),
                    )
                    .child(
                        gpui::div()
                            .text_size(gpui::px(11.))
                            .text_color(rgb(MUTED))
                            .child(status_text.to_string()),
                    ),
            )
            .child(
                h_flex()
                    .gap(gpui::px(12.))
                    .items_center()
                    .child(
                        gpui::div()
                            .text_size(gpui::px(11.))
                            .text_color(rgb(MUTED))
                            .child(stats_text),
                    )
                    .child(
                        gpui::div()
                            .text_size(gpui::px(10.))
                            .text_color(rgb(BORDER))
                            .child(text.hotkey_toggle),
                    ),
            )
    }

    fn render_language_switch(&self, cx: &Context<Self>) -> impl IntoElement {
        let mut tabs = h_flex()
            .h(gpui::px(28.))
            .bg(rgb(SECONDARY))
            .rounded(gpui::px(6.))
            .border_1()
            .border_color(rgb(BORDER));

        for (index, language) in [Language::En, Language::ZhCn].into_iter().enumerate() {
            let active = self.language == language;
            let is_first = index == 0;
            let is_last = index == 1;

            let mut tab = gpui::div()
                .id(("lang", index))
                .cursor_pointer()
                .h_full()
                .px(gpui::px(8.))
                .flex()
                .items_center()
                .bg(rgb(if active { PRIMARY } else { SECONDARY }));

            if is_first {
                tab = tab.rounded_l(gpui::px(5.));
            }
            if is_last {
                tab = tab.rounded_r(gpui::px(5.));
            }

            tab = tab
                .child(
                    gpui::div()
                        .text_size(gpui::px(10.))
                        .text_color(rgb(if active { FG } else { MUTED }))
                        .child(language.switcher_label()),
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.language = language;
                    this.save_current_config(cx);
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation());

            tabs = tabs.child(tab);
        }

        tabs
    }
}

#[cfg(test)]
mod tests {
    use super::{advance_pick_frame, can_start, start_block_reason, StartBlockReason};

    #[cfg(target_os = "macos")]
    use super::{mac_window_level, NS_NORMAL_WINDOW_LEVEL, NS_POP_UP_WINDOW_LEVEL};

    #[test]
    fn start_requires_idle_target_and_permission() {
        assert!(can_start(false, true, true));
        assert!(!can_start(true, true, true));
        assert!(!can_start(false, false, true));
        assert!(!can_start(false, true, false));
    }

    #[test]
    fn start_block_reason_prefers_permission_then_target() {
        assert_eq!(
            start_block_reason(false, true, false),
            Some(StartBlockReason::MissingPermission)
        );
        assert_eq!(
            start_block_reason(false, false, true),
            Some(StartBlockReason::MissingTarget)
        );
        assert_eq!(
            start_block_reason(false, false, false),
            Some(StartBlockReason::MissingPermission)
        );
        assert_eq!(start_block_reason(true, false, false), None);
        assert_eq!(start_block_reason(false, true, true), None);
    }

    #[test]
    fn pick_does_not_confirm_while_waiting_for_first_release() {
        let result =
            advance_pick_frame(Some(&"old"), Some("new"), false, true, false, |a, b| a == b);

        assert_eq!(result.target, Some("new"));
        assert!(!result.ready_to_confirm);
        assert!(!result.stop_picking);
        assert!(result.changed);
    }

    #[test]
    fn pick_arms_after_mouse_is_released_once() {
        let result = advance_pick_frame(
            Some(&"current"),
            Some("hovered"),
            false,
            false,
            false,
            |a, b| a == b,
        );

        assert_eq!(result.target, Some("hovered"));
        assert!(result.ready_to_confirm);
        assert!(!result.stop_picking);
        assert!(result.changed);
    }

    #[test]
    fn pick_stops_immediately_when_mouse_is_pressed_after_arming() {
        let result = advance_pick_frame(
            Some(&"current"),
            Some("hovered"),
            true,
            true,
            false,
            |a, b| a == b,
        );

        assert_eq!(result.target, Some("hovered"));
        assert!(result.ready_to_confirm);
        assert!(result.stop_picking);
        assert!(result.changed);
    }

    #[test]
    fn pick_keeps_last_target_when_pressing_without_hovered_window() {
        let result =
            advance_pick_frame(Some(&"current"), None::<&str>, true, true, false, |a, b| {
                a == b
            });

        assert_eq!(result.target, Some("current"));
        assert!(result.ready_to_confirm);
        assert!(result.stop_picking);
        assert!(!result.changed);
    }

    #[test]
    fn pick_stops_when_click_happens_between_polls() {
        let result = advance_pick_frame(
            Some(&"current"),
            Some("hovered"),
            true,
            false,
            true,
            |a, b| a == b,
        );

        assert_eq!(result.target, Some("hovered"));
        assert!(result.ready_to_confirm);
        assert!(result.stop_picking);
        assert!(result.changed);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_always_on_top_uses_expected_window_levels() {
        assert_eq!(mac_window_level(false), NS_NORMAL_WINDOW_LEVEL);
        assert_eq!(mac_window_level(true), NS_POP_UP_WINDOW_LEVEL);
    }
}
