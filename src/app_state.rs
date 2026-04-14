use std::collections::HashMap;
use std::time::Duration;

use gpui::{
    AppContext, Context, Entity, FocusHandle, KeyDownEvent, MouseButton, Render, WeakEntity, Window,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled,
              StatefulInteractiveElement},
    rgb,
};
use gpui_component::{Sizable, h_flex, v_flex};
use gpui_component::input::{Input, InputState};

use crate::key_sender::VirtualKey;
use crate::scheduler::{KeyTask, Scheduler, SendStats};
use crate::window_picker::WindowInfo;

#[cfg(target_os = "windows")]
fn start_window_drag(window: &Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{HTCAPTION, PostMessageA, WM_NCLBUTTONDOWN},
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
    pub scheduler: Option<Scheduler>,
    pub is_running: bool,
    pub is_picking: bool,
    pub stats: SendStats,
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        let first_id = next_id();
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("200"));

        let mut interval_inputs = HashMap::new();
        interval_inputs.insert(first_id, input);

        Self {
            focus_handle,
            target_window: None,
            key_tasks: vec![
                KeyTask { id: first_id, vk: VirtualKey(0x0D), interval_ms: 200 },
            ],
            interval_inputs,
            recording_task_id: None,
            scheduler: None,
            is_running: false,
            is_picking: false,
            stats: SendStats::default(),
        }
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
        let hwnd = match &self.target_window {
            Some(w) => w.hwnd,
            None => return,
        };
        if self.key_tasks.is_empty() {
            return;
        }

        self.sync_intervals_from_inputs(cx);

        let mut scheduler = Scheduler::new(hwnd, self.key_tasks.clone());
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
        if self.is_picking {
            self.start_picking(cx);
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
        cx.spawn(async move |entity: WeakEntity<Self>, async_app| {
            loop {
                async_app.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                let should_continue = entity
                    .update(async_app, |state, cx| {
                        if !state.is_picking {
                            return false;
                        }
                        if let Some(info) = crate::window_picker::get_window_under_cursor() {
                            let changed = state.target_window.as_ref()
                                .map_or(true, |w| w.hwnd != info.hwnd);
                            if changed {
                                state.target_window = Some(info);
                                cx.notify();
                            }
                        }
                        true
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
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

        let target_text = self.target_window
            .as_ref()
            .map_or("No window selected".to_string(), |w| w.title.clone());

        let pick_label = if self.is_picking { "Stop" } else { "Pick" };
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
            .child(self.render_header(cx))
            .child(self.render_body(&target_text, pick_label, cx))
            .child(self.render_status_bar())
    }
}

impl AppState {
    fn render_header(&self, cx: &Context<Self>) -> impl IntoElement {
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
                h_flex().gap(gpui::px(8.)).items_center()
                    .child(
                        gpui::div()
                            .text_size(gpui::px(16.))
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(rgb(FG))
                            .child("Auto Keypress")
                    )
            )
            .child(
                h_flex().gap(gpui::px(4.)).items_center()
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
                                    .child("—")
                            )
                            .on_click(|_, window, _cx| {
                                window.minimize_window();
                            })
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
                                    .child("×")
                            )
                            .on_click(cx.listener(|_this, _, _window, cx| {
                                cx.quit();
                            }))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    )
            )
    }

    fn render_body(
        &self,
        target_text: &str,
        pick_label: &str,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let is_picking = self.is_picking;
        let is_running = self.is_running;
        let has_target = self.target_window.is_some();
        let tasks = self.key_tasks.clone();

        v_flex()
            .w_full()
            .flex_grow()
            .p(gpui::px(16.))
            .gap(gpui::px(16.))
            .child(self.render_target_section(target_text, pick_label, is_picking, cx))
            .child(self.render_keys_section(&tasks, cx))
            .child(gpui::div().w_full().h(gpui::px(1.)).bg(rgb(BORDER)))
            .child(self.render_controls(is_running, has_target, cx))
    }

    fn render_target_section(
        &self,
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
                    .child("Target Window")
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
                                    .text_color(rgb(if self.target_window.is_some() { FG } else { MUTED }))
                                    .overflow_x_hidden()
                                    .child(target_owned)
                            )
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
                                    .child(pick_label.to_string())
                            )
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.toggle_pick(cx);
                                cx.notify();
                            }))
                    )
            )
    }

    fn render_keys_section(&self, tasks: &[KeyTask], cx: &Context<Self>) -> impl IntoElement {
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
                        .child("Key Tasks")
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
                                .child("+ Add Key")
                        )
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.add_key_task(window, cx);
                            cx.notify();
                        }))
                )
        );

        let mut list = v_flex().w_full().gap(gpui::px(6.));
        for task in tasks {
            list = list.child(self.render_key_row(task, cx));
        }
        section = section.child(list);

        section
    }

    fn render_key_row(&self, task: &KeyTask, cx: &Context<Self>) -> impl IntoElement {
        let task_id = task.id;
        let key_name = task.vk.name().to_string();
        let is_recording = self.recording_task_id == Some(task_id);

        let badge_bg = if is_recording { WARN } else { SECONDARY };
        let badge_text = if is_recording { "Press..." } else { &key_name };

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
                        .child(badge_text.to_string())
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    if this.recording_task_id == Some(task_id) {
                        this.recording_task_id = None;
                    } else {
                        this.recording_task_id = Some(task_id);
                    }
                    cx.notify();
                }))
        );

        row = row
            .child(gpui::div().w(gpui::px(1.)).h(gpui::px(20.)).bg(rgb(BORDER)))
            .child(
                gpui::div()
                    .text_size(gpui::px(11.))
                    .text_color(rgb(MUTED))
                    .child("Interval")
            );

        if let Some(input_entity) = self.interval_inputs.get(&task_id) {
            row = row.child(
                gpui::div()
                    .w(gpui::px(80.))
                    .child(
                        Input::new(input_entity)
                            .appearance(false)
                            .small()
                    )
            );
        }

        row = row.child(
            gpui::div()
                .text_size(gpui::px(11.))
                .text_color(rgb(MUTED))
                .child("ms")
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
                        .child("×")
                )
                .on_click(cx.listener(move |this, _, _window, cx| {
                    this.remove_key_task(task_id);
                    cx.notify();
                }))
        );

        row
    }

    fn render_controls(
        &self,
        is_running: bool,
        _has_target: bool,
        cx: &Context<Self>,
    ) -> impl IntoElement {
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
                    .bg(rgb(if is_running { SECONDARY } else { PRIMARY }))
                    .rounded(gpui::px(8.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .gap(gpui::px(8.))
                    .when(!is_running, |el| {
                        el.child(
                            gpui::div()
                                .text_size(gpui::px(14.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(FG))
                                .child("▶ Start")
                        )
                    })
                    .when(is_running, |el| {
                        el.child(
                            gpui::div()
                                .text_size(gpui::px(14.))
                                .font_weight(gpui::FontWeight::MEDIUM)
                                .text_color(rgb(MUTED))
                                .child("Running...")
                        )
                    })
                    .on_click(cx.listener(|this, _, _window, cx| {
                        if !this.is_running {
                            this.start(cx);
                        }
                        cx.notify();
                    }))
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
                            .child("■ Stop")
                    )
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.stop();
                        cx.notify();
                    }))
            )
    }

    fn render_status_bar(&self) -> impl IntoElement {
        let status_text = if self.is_running { "Running" } else { "Ready" };
        let status_color = if self.is_running { SUCCESS } else { MUTED };

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
                            .bg(rgb(status_color))
                    )
                    .child(
                        gpui::div()
                            .text_size(gpui::px(11.))
                            .text_color(rgb(MUTED))
                            .child(status_text.to_string())
                    )
            )
            .child(
                gpui::div()
                    .text_size(gpui::px(11.))
                    .text_color(rgb(MUTED))
                    .child(stats_text)
            )
    }
}
