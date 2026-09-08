//! ## 设置模块
//!
//! 本模块负责用户偏好的持久化存储和读取，分为两层：
//!
//! 1. **`Settings` 实体**（本文件）—— 状态管理和文件 I/O。
//!    控件变化时通知外壳，文件读写失败时反馈给设置面板。
//! 2. **`SettingsPanel` 视图**（`panel.rs`）—— UI 渲染。
//!    只持有控件状态，不直接操作文件。
//!
//! ### 存储方式
//!
//! 设置以 JSON 格式保存在 `~/.config/dev_tools/settings.json`。
//! 每次用户修改偏好后立即写入（「保存即生效」策略），而不是等到应用退出时批量保存。
//! 写入时使用「写入临时文件 → 原子重命名」策略，避免中途退出留下半份 JSON 文件。
//!
//! ### 为什么这样设计？
//!
//! - **值类型 + 序列化**：`Settings` 是一个纯值类型（struct of values），
//!   通过 `serde` 序列化/反序列化，不涉及任何 GPUI 实体机制。
//! - **实体包装**：`Settings` 作为 GPUI Entity，使得界面可以订阅其变化
//!   （通过 `cx.observe`），当设置被修改时自动触发 UI 更新。
//! - **运行时状态不持久化**：`save_error` 字段标注了 `#[serde(skip)]`，
//!   它只存在于运行时，不会写入文件。这样用户不会看到上次运行时的错误提示。
mod theme;
pub use theme::apply;

mod panel;

use gpui_kit::{Context, Window};
pub use panel::SettingsPanel;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

/// 主题选择枚举。
///
/// `System` 为默认值，表示跟随系统外观（macOS 的深色/浅色模式自动切换）。
/// `Light` 和 `Dark` 为手动指定，不会随系统变化。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    /// 跟随系统外观（默认）
    #[default]
    System,
    /// 始终使用浅色主题
    Light,
    /// 始终使用深色主题
    Dark,
}

/// 侧边栏菜单位置枚举。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum MenuPosition {
    /// 侧边栏在左侧（默认）
    #[default]
    Left,
    /// 侧边栏在右侧
    Right,
}

/// 应用设置结构体。
///
/// 直接对应 JSON 文件的字段。`#[serde(default)]` 属性确保当 JSON 中缺失某个字段时，
/// 使用该字段的 `Default` 值，而不是直接报错 —— 这使得旧版本设置文件可以向前兼容。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// 侧边栏菜单位置：左侧或右侧
    pub menu_position: MenuPosition,
    /// 主题选择：浅色/深色/跟随系统
    pub theme: ThemeChoice,
    /// 运行时错误提示（`#[serde(skip)]` 表示不序列化到文件）。
    /// 用于在设置面板中显示保存失败信息，但不会污染持久化文件。
    #[serde(skip)]
    pub save_error: Option<String>,
}

/// 设置模块内部错误类型。
///
/// 使用 `thiserror` 派生宏，为设置操作中的三种失败场景提供语义化的错误描述。
#[derive(Debug, thiserror::Error)]
enum SettingsError {
    /// `$HOME` 环境变量未设置，无法确定配置文件路径。
    #[error("无法确定设置目录")]
    MissingHome,
    /// 文件 I/O 操作失败（读取或写入）。
    #[error("无法读写设置文件")]
    Io(#[from] std::io::Error),
    /// 设置文件 JSON 格式无效。
    #[error("设置文件格式无效")]
    Json(#[from] serde_json::Error),
}

impl Settings {
    /// 返回设置文件的路径：`~/.config/dev_tools/settings.json`。
    fn file_path() -> Result<PathBuf, SettingsError> {
        // `std::env::var_os` 读取环境变量，返回 `Option<OsString>`。
        // 在 macOS/Linux 上 `$HOME` 通常指向 `/Users/<username>`。
        let home = std::env::var_os("HOME").ok_or(SettingsError::MissingHome)?;
        Ok(PathBuf::from(home).join(".config/dev_tools/settings.json"))
    }

    /// 创建并加载设置。
    ///
    /// 尝试从配置文件读取已保存的设置，如果文件不存在或格式无效则使用默认值。
    /// 读取失败时在 `save_error` 中记录错误信息，供设置面板显示。
    pub fn new(_: &mut Window, _: &mut Context<Self>) -> Self {
        let result = Self::file_path().and_then(|path| Self::load_from(&path));
        match result {
            Ok(settings) => settings,
            Err(error) => {
                log::warn!(target: "settings", "读取设置失败，使用默认值: {error}");
                Self {
                    save_error: Some(
                        "无法读取已保存的设置，当前使用默认值。更改任一设置后将重新保存。".into(),
                    ),
                    ..Self::default()
                }
            }
        }
    }

    /// 从指定路径加载设置。
    ///
    /// - 如果文件不存在，返回默认设置（首次使用场景）。
    /// - 如果文件存在但格式错误，返回错误。
    fn load_from(path: &Path) -> Result<Self, SettingsError> {
        match fs::read_to_string(path) {
            Ok(text) => Ok(serde_json::from_str(&text)?),
            // 文件不存在不是错误 —— 首次使用时还没有配置文件
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into()),
        }
    }

    /// 保存设置到指定路径。
    ///
    /// ### 原子写入策略
    ///
    /// 1. 先写入到同目录下的 `.json.tmp` 临时文件。
    /// 2. 然后通过 `fs::rename` 原子替换原文件。
    ///
    /// 这样即使写入过程中程序崩溃，也不会留下半份 JSON 文件。
    /// 如果直接写入原文件，写入中途崩溃会导致设置文件损坏。
    fn save_to(&self, path: &Path) -> Result<(), SettingsError> {
        // 确保父目录存在（首次保存时目录可能不存在）
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // 写入临时文件
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        // 原子替换，避免写一半崩溃
        fs::rename(temporary, path)?;
        Ok(())
    }

    /// 保存当前设置到文件。
    ///
    /// 保存成功时清除 `save_error`，失败时记录错误信息供 UI 显示。
    fn save(&mut self) {
        match Self::file_path().and_then(|path| self.save_to(&path)) {
            Ok(()) => {
                self.save_error = None;
                log::info!(target: "settings", "设置已保存");
            }
            Err(error) => {
                log::error!(target: "settings", "保存设置失败: {error}");
                self.save_error =
                    Some("设置已在本次运行中生效，但保存失败。请检查设置目录权限后重试。".into());
            }
        }
    }

    /// 设置侧边栏菜单位置并保存。
    pub fn set_menu_position(&mut self, position: MenuPosition, cx: &mut Context<Self>) {
        self.menu_position = position;
        self.save();
        cx.notify();
    }

    /// 设置主题选择并保存。
    pub fn set_theme(&mut self, choice: ThemeChoice, cx: &mut Context<Self>) {
        self.theme = choice;
        self.save();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：未指定的字段应使用默认值填充。
    #[test]
    fn partial_settings_preserve_known_fields() {
        let settings: Settings = serde_json::from_str(r#"{"theme":"Dark"}"#).unwrap();
        assert_eq!(settings.theme, ThemeChoice::Dark);
        assert_eq!(settings.menu_position, MenuPosition::Left);
    }

    /// 测试：保存 → 加载 → 验证往返，以及损坏文件的处理。
    #[test]
    fn save_roundtrip_and_corrupt_file() {
        // 使用临时目录，避免污染实际配置
        let directory =
            std::env::temp_dir().join(format!("dev-tools-settings-{}", std::process::id()));
        let path = directory.join("settings.json");

        // 保存设置
        let settings = Settings {
            theme: ThemeChoice::Dark,
            menu_position: MenuPosition::Right,
            save_error: None,
        };
        settings.save_to(&path).unwrap();

        // 重新加载，验证字段完整
        let restored = Settings::load_from(&path).unwrap();
        assert_eq!(restored.theme, ThemeChoice::Dark);
        assert_eq!(restored.menu_position, MenuPosition::Right);

        // 写入损坏内容，验证加载失败
        fs::write(&path, "broken").unwrap();
        assert!(Settings::load_from(&path).is_err());

        // 清理临时文件
        fs::remove_dir_all(directory).unwrap();
    }
}