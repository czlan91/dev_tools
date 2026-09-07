use std::rc::Rc;
use gpui_kit::{App, Window};
use gpui_kit::component::theme::{Theme, ThemeConfig, ThemeMode, ThemeSet};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Default)]
pub enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}
static LIGHT_JSON: &str = include_str!("../assets/themes/light.json");
static DARK_JSON: &str = include_str!("../assets/themes/dark.json");
fn parse_theme(json: &str) -> Option<Rc<ThemeConfig>> {
    let set: ThemeSet = serde_json::from_str(json).ok()?;
    set.themes.into_iter().next().map(Rc::new)
}
pub fn apply(choice: ThemeChoice, window: &mut Window, cx: &mut App) {
    let want_dark = match choice {
        ThemeChoice::Light => false,
        ThemeChoice::Dark => true,
        ThemeChoice::System => {
            ThemeMode::from(window.appearance()).is_dark()
        }
    };
    let json = if want_dark { DARK_JSON } else { LIGHT_JSON };
    let Some(config) = parse_theme(json) else {
        log::error!(
            target: "theme",
            "主题文件解析失败，保留当前主题 (want_dark={want_dark})"
        );
        return;
    };
    let theme = Theme::global_mut(cx);
    theme.apply_config(&config);
    theme.mode = if want_dark {
        gpui_kit::component::theme::ThemeMode::Dark
    } else {
        gpui_kit::component::theme::ThemeMode::Light
    };
    window.refresh();
    log::info!(target: "theme", "已应用主题: {choice:?} (dark={want_dark})");
}
