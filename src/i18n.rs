#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    En,
    ZhCn,
}

pub struct UiText {
    pub app_title: &'static str,
    pub no_window_selected: &'static str,
    pub target_window: &'static str,
    pub accessibility_required: &'static str,
    pub accessibility_hint: &'static str,
    pub open_settings: &'static str,
    pub start_needs_permission: &'static str,
    pub start_needs_window: &'static str,
    pub pick: &'static str,
    pub key_tasks: &'static str,
    pub add_key: &'static str,
    pub press_key: &'static str,
    pub interval: &'static str,
    pub send_mode: &'static str,
    pub start: &'static str,
    pub running: &'static str,
    pub running_status: &'static str,
    pub stop: &'static str,
    pub ready: &'static str,
    pub hotkey_toggle: &'static str,
    pub milliseconds: &'static str,
}

const EN_TEXT: UiText = UiText {
    app_title: "Auto Keypress",
    no_window_selected: "No window selected",
    target_window: "Target Window",
    accessibility_required: "Accessibility permission required",
    accessibility_hint: "Allow this app in System Settings -> Privacy & Security -> Accessibility",
    open_settings: "Open Settings",
    start_needs_permission: "Grant Accessibility permission before starting",
    start_needs_window: "Pick a target window before starting",
    pick: "Pick",
    key_tasks: "Key Tasks",
    add_key: "+ Add Key",
    press_key: "Press...",
    interval: "Interval",
    send_mode: "Send Mode",
    start: "▶ Start",
    running: "Running...",
    running_status: "Running",
    stop: "■ Stop",
    ready: "Ready",
    hotkey_toggle: "F9: Toggle",
    milliseconds: "ms",
};

const ZH_CN_TEXT: UiText = UiText {
    app_title: "自动按键",
    no_window_selected: "未选择窗口",
    target_window: "目标窗口",
    accessibility_required: "需要辅助功能权限",
    accessibility_hint: "请在 系统设置 -> 隐私与安全性 -> 辅助功能 中允许当前应用",
    open_settings: "打开设置",
    start_needs_permission: "开始前先授予辅助功能权限",
    start_needs_window: "开始前先选择目标窗口",
    pick: "拾取",
    key_tasks: "按键任务",
    add_key: "+ 添加按键",
    press_key: "按键中...",
    interval: "间隔",
    send_mode: "发送模式",
    start: "▶ 开始",
    running: "运行中...",
    running_status: "运行中",
    stop: "■ 停止",
    ready: "就绪",
    hotkey_toggle: "F9：切换启停",
    milliseconds: "毫秒",
};

impl Language {
    pub fn from_code(code: &str) -> Self {
        match code.trim().to_ascii_lowercase().as_str() {
            "zh" | "zh-cn" | "zh_hans" | "zh-hans" | "cn" => Self::ZhCn,
            _ => Self::En,
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::ZhCn => "zh-CN",
        }
    }

    pub fn toggle(self) -> Self {
        match self {
            Self::En => Self::ZhCn,
            Self::ZhCn => Self::En,
        }
    }

    pub fn labels(self) -> &'static UiText {
        match self {
            Self::En => &EN_TEXT,
            Self::ZhCn => &ZH_CN_TEXT,
        }
    }

    pub fn switcher_label(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::ZhCn => "中文",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Language;

    #[test]
    fn language_code_parsing_and_toggle_work() {
        assert_eq!(Language::from_code("zh-CN"), Language::ZhCn);
        assert_eq!(Language::from_code("ZH"), Language::ZhCn);
        assert_eq!(Language::from_code("unknown"), Language::En);
        assert_eq!(Language::En.toggle(), Language::ZhCn);
        assert_eq!(Language::ZhCn.toggle(), Language::En);
    }

    #[test]
    fn ui_copy_switches_between_english_and_chinese() {
        let en = Language::En.labels();
        let zh = Language::ZhCn.labels();

        assert_eq!(en.app_title, "Auto Keypress");
        assert_eq!(zh.app_title, "自动按键");
        assert_eq!(zh.ready, "就绪");
        assert_eq!(en.hotkey_toggle, "F9: Toggle");
        assert_eq!(zh.accessibility_required, "需要辅助功能权限");
        assert_eq!(
            en.accessibility_hint,
            "Allow this app in System Settings -> Privacy & Security -> Accessibility"
        );
        assert_eq!(zh.open_settings, "打开设置");
        assert_eq!(zh.start_needs_window, "开始前先选择目标窗口");
    }
}
