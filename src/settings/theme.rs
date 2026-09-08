//! ## 主题管理
//!
//! 本模块负责根据用户偏好加载和应用 GPUI 主题。
//!
//! ### 主题文件
//!
//! 主题定义在 `assets/themes/` 目录下的 JSON 文件中：
//! - `light.json`：浅色主题配置
//! - `dark.json`：深色主题配置
//!
//! 使用 `include_str!` 在编译时将主题文件嵌入到二进制中，
//! 这样运行时不需要从文件系统读取主题文件，避免文件缺失问题。
//!
//! ### 主题选择逻辑
//!
//! - `ThemeChoice::Light`：始终使用浅色主题
//! - `ThemeChoice::Dark`：始终使用深色主题
//! - `ThemeChoice::System`：根据系统外观（macOS 深色/浅色模式）自动切换

use crate::settings::ThemeChoice;
use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeMode, ThemeSet};
use gpui_kit::{App, Window};
use std::rc::Rc;

/// 浅色主题 JSON（编译时嵌入）。
static LIGHT_JSON: &str = include_str!("../../assets/themes/light.json");
/// 深色主题 JSON（编译时嵌入）。
static DARK_JSON: &str = include_str!("../../assets/themes/dark.json");

/// 解析主题 JSON 字符串为 `ThemeConfig`。
///
/// `ThemeSet` 是 GPUI 主题文件的容器格式，可以包含多个主题。
/// 这里我们只取第一个主题（每个 JSON 文件只定义一个主题）。
fn parse_theme(json: &str) -> Option<Rc<ThemeConfig>> {
    let set: ThemeSet = serde_json::from_str(json).ok()?;
    set.themes.into_iter().next().map(Rc::new)
}

/// 应用主题到当前窗口。
///
/// 根据 `ThemeChoice` 决定使用浅色/深色主题，
/// 然后更新全局主题配置并刷新窗口。
///
/// 此函数在两种情况下调用：
/// 1. 应用启动时，根据已保存的设置应用主题。
/// 2. 用户通过设置面板切换主题或系统外观变化时。
pub fn apply(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    // 决定是否使用深色主题
    let want_dark = match choice {
        ThemeChoice::Light => false,
        ThemeChoice::Dark => true,
        // 跟随系统：根据窗口所在屏幕的外观模式判断
        ThemeChoice::System => ThemeMode::from(window.appearance()).is_dark(),
    };

    // 加载对应的主题 JSON
    let json = if want_dark { DARK_JSON } else { LIGHT_JSON };
    let Some(config) = parse_theme(json) else {
        log::error!(
            target: "theme",
            "主题文件解析失败，保留当前主题 (want_dark={want_dark})"
        );
        return;
    };

    // 更新全局主题
    let theme = Theme::global_mut(cx);
    theme.apply_config(&config);
    theme.mode = if want_dark {
        gpui_kit::component::theme::ThemeMode::Dark
    } else {
        gpui_kit::component::theme::ThemeMode::Light
    };
    // 设置等宽字体为 JetBrains Mono（代码输入/输出区域使用）
    theme.mono_font_family = "JetBrains Mono".into();

    // 同步主题到所有组件，并刷新窗口
    Theme::sync_base(cx);
    window.refresh();

    log::info!(target: "theme", "已应用主题: {choice:?} (dark={want_dark})");
}