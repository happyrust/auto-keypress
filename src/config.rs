use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::key_sender::{SendMode, VirtualKey};

#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub send_mode: String,
    pub always_on_top: bool,
    pub tasks: Vec<TaskConfig>,
}

#[derive(Serialize, Deserialize)]
pub struct TaskConfig {
    pub key_name: String,
    pub vk: u16,
    pub interval_ms: u64,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            send_mode: "PostMessage".to_string(),
            always_on_top: false,
            tasks: vec![TaskConfig {
                key_name: "ENTER".to_string(),
                vk: 0x0D,
                interval_ms: 200,
            }],
        }
    }
}

impl AppConfig {
    pub fn send_mode_enum(&self) -> SendMode {
        match self.send_mode.as_str() {
            "SendMessage" => SendMode::SendMessage,
            _ => SendMode::PostMessage,
        }
    }
}

fn config_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("auto-keypress-config.json")
}

pub fn load_config() -> AppConfig {
    let path = config_path();
    if !path.exists() {
        return AppConfig::default();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

pub fn save_config(config: &AppConfig) {
    let path = config_path();
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(path, json);
    }
}

pub fn config_from_state(
    send_mode: SendMode,
    always_on_top: bool,
    tasks: &[(VirtualKey, u64)],
) -> AppConfig {
    AppConfig {
        send_mode: send_mode.label().to_string(),
        always_on_top,
        tasks: tasks
            .iter()
            .map(|(vk, interval)| TaskConfig {
                key_name: vk.name().to_string(),
                vk: vk.0,
                interval_ms: *interval,
            })
            .collect(),
    }
}
