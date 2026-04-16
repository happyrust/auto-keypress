#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app_state;
mod config;
mod i18n;
mod key_sender;
mod scheduler;
mod window_picker;

use gpui::{px, size, AppContext, Bounds, WindowBounds, WindowOptions};
use gpui_component::{theme::Theme, Root};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    #[cfg(target_os = "macos")]
    tracing::info!(
        "macOS 键盘事件权限就绪: {}",
        crate::key_sender::accessibility_trusted()
    );

    gpui::Application::new().run(move |cx| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(420.), px(520.)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: None,
                ..Default::default()
            },
            |window, cx| {
                Theme::change(gpui_component::ThemeMode::Dark, Some(window), cx);
                let view = cx.new(|cx| app_state::AppState::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("failed to open window");
    });
}
