use std::{fs, path::PathBuf};
use gpui_kit::prelude::*;
use gpui_kit::Window;
use serde::{Deserialize, Serialize};
use crate::theme::ThemeChoice;
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum MenuPosition {
    #[default]
    Left,
    Right,
}

impl MenuPosition {
    /// 在 Left 和 Right 之间来回切换。
    pub fn toggle(self) -> Self {
        match self {
            MenuPosition::Left => MenuPosition::Right,
            MenuPosition::Right => MenuPosition::Left,
        }
    }
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Settings {
    pub menu_position: MenuPosition,
    pub theme: ThemeChoice,
}
impl Settings {
    fn file_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        Some(
            PathBuf::from(home)
                .join(".config")
                .join("dev_tools")
                .join("settings.json"),
        )
    }
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self::load()
    }
    fn load() -> Self {
        match Self::file_path().and_then(|p| fs::read_to_string(p).ok()) {
            Some(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => {
                    log::info!(target: "settings", "已恢复设置: {s:?}");
                    s
                }
                Err(e) => {
                    log::warn!(target: "settings", "设置文件损坏，使用默认值: {e}");
                    Self::default()
                }
            },
            None => Self::default(),
        }
    }
    pub fn save(&self) {
        let Some(path) = Self::file_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                log::error!(target: "settings", "创建设置目录失败 {:?}: {e}", parent);
                return;
            }
        }
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(e) = fs::write(&path, text) {
                    log::error!(target: "settings", "保存设置失败 {:?}: {e}", path);
                }
            }
            Err(e) => log::error!(target: "settings", "序列化设置失败: {e}"),
        }
    }
}
impl Settings {
    pub fn set_menu_position(&mut self, position: MenuPosition, cx: &mut Context<Self>) {
        self.menu_position = position;
        self.save();
        cx.notify();
    }
    pub fn set_theme(&mut self, choice: ThemeChoice, cx: &mut Context<Self>) {
        self.theme = choice;
        self.save();
        cx.notify();
    }
}
