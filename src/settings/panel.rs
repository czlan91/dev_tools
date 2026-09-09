//! ## 设置面板视图
//!
//! 本文件负责渲染设置弹窗的 UI 界面。`SettingsPanel` 实体持有控件状态
//! （如下拉选择框的当前选项），但所有持久偏好数据由 `Settings` 实体统一管理。
//!
//! ### 设计原则
//!
//! - **视图与状态分离**：`SettingsPanel` 只负责 UI 渲染和控件交互事件的转发，
//!   实际的设置值读写通过 `Settings` 实体完成。
//! - **可重置**：每个设置项都支持「重置为默认值」，通过 `on_reset` 回调实现。
//! - **实时保存**：用户修改任何设置后立即通过 `Settings` 实体保存到文件。
use super::{Language, MenuPosition, Settings, ThemeChoice};
use gpui_kit::component::{
    ActiveTheme,
    alert::Alert,
    group_box::GroupBoxVariant,
    h_flex,
    select::{Select, SelectEvent, SelectState},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings as SettingsView},
    switch::Switch,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use rust_i18n::t;

/// 设置面板实体。
///
/// 持有 `Settings` 实体的引用和主题、语言选择下拉框的状态。
/// 通过订阅 `Settings` 实体的变化和下拉框的选择事件，在两者之间同步状态。
pub struct SettingsPanel {
    /// 设置实体引用，用于读取和写入持久化偏好。
    settings: Entity<Settings>,
    /// 主题选择下拉框的控件状态。
    theme_select: Entity<SelectState<Vec<SharedString>>>,
    /// 语言选择下拉框的控件状态。
    language_select: Entity<SelectState<Vec<SharedString>>>,
    /// 订阅集合，必须保持存活否则订阅会被自动取消。
    _subscriptions: Vec<Subscription>,
}

impl SettingsPanel {
    /// 创建设置面板实例。
    ///
    /// 初始化主题选择下拉框，根据当前设置选中对应的选项。
    /// 设置两个订阅：
    /// 1. 监听下拉框选择事件 → 更新主题设置。
    /// 2. 监听设置实体变化 → 同步下拉框状态（当其他代码修改设置时）。
    pub fn new(settings: Entity<Settings>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 创建主题选择下拉框，提供三个选项
        let theme_select: Entity<SelectState<Vec<SharedString>>> = cx.new(|cx| {
            SelectState::new(
                vec!["白色".into(), "黑色".into(), "跟随系统".into()],
                None,
                window,
                cx,
            )
        });

        // 根据当前设置，选中对应的下拉框选项
        theme_select.update(cx, |select, cx| {
            select.set_selected_value(&theme_label(settings.read(cx).theme), window, cx)
        });

        // 创建语言选择下拉框，提供简体中文和英文两个选项
        let language_select: Entity<SelectState<Vec<SharedString>>> = cx.new(|cx| {
            SelectState::new(vec!["简体中文".into(), "English".into()], None, window, cx)
        });
        language_select.update(cx, |select, cx| {
            select.set_selected_value(&language_label(settings.read(cx).language), window, cx)
        });

        // 订阅主题下拉框的选择事件 → 更新设置实体
        let selected = cx.subscribe_in(&theme_select, window, {
            let settings = settings.clone();
            move |_, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    // 将中文标签映射为 ThemeChoice 枚举值
                    let choice = match value.as_ref() {
                        "白色" => ThemeChoice::Light,
                        "黑色" => ThemeChoice::Dark,
                        _ => ThemeChoice::System,
                    };
                    settings.update(cx, |settings, cx| settings.set_theme(choice, cx));
                }
            }
        });

        // 订阅语言下拉框的选择事件 → 更新设置实体（set_language 会同时切换全局 locale）
        let language_selected = cx.subscribe_in(&language_select, window, {
            let settings = settings.clone();
            move |_, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    let language = match value.as_ref() {
                        "English" => Language::En,
                        _ => Language::ZhCN,
                    };
                    settings.update(cx, |settings, cx| settings.set_language(language, cx));
                }
            }
        });

        // 订阅设置实体的变化 → 同步两个下拉框的状态
        // 当外部代码（如重置按钮）修改了设置时，下拉框选项需要同步更新。
        let changed = cx.observe_in(&settings, window, |panel, settings, window, cx| {
            let theme = theme_label(settings.read(cx).theme);
            let language = language_label(settings.read(cx).language);
            // 只更新控件显示，不触发选择事件，避免反馈循环
            panel.theme_select.update(cx, |select, cx| {
                select.set_selected_value(&theme, window, cx)
            });
            panel.language_select.update(cx, |select, cx| {
                select.set_selected_value(&language, window, cx)
            });
            cx.notify();
        });

        Self {
            settings,
            theme_select,
            language_select,
            _subscriptions: vec![selected, language_selected, changed],
        }
    }
}

/// 将 `ThemeChoice` 枚举值转换为显示标签。
fn theme_label(choice: ThemeChoice) -> SharedString {
    match choice {
        ThemeChoice::Light => "白色",
        ThemeChoice::Dark => "黑色",
        ThemeChoice::System => "跟随系统",
    }
    .into()
}

/// 将 `Language` 枚举值转换为显示标签。
fn language_label(language: Language) -> SharedString {
    match language {
        Language::ZhCN => "简体中文",
        Language::En => "English",
    }
    .into()
}

impl Render for SettingsPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 由于 `render` 中会多次克隆 `self.settings`，这里提前克隆好引用
        // 供闭包捕获。`Entity` 是引用计数的句柄，克隆成本很低（只增加引用计数）。
        let read_menu = self.settings.clone();
        let dirty_menu = self.settings.clone();
        let reset_menu = self.settings.clone();
        let select = self.theme_select.clone();
        let dirty_theme = self.settings.clone();
        let reset_theme = self.settings.clone();
        let language_select = self.language_select.clone();
        let dirty_language = self.settings.clone();
        let reset_language = self.settings.clone();

        // 使用 GPUI 组件库的 `SettingsView` 构建设置页面布局
        // `with_group_variant(Outline)` 让每个设置分组渲染为带边框的卡片，
        // 避免默认 Normal 变体下整个面板像一块无边界白板。
        let panel = SettingsView::new("app-settings")
            .sidebar_width(window.rem_size() * 12.)
            .with_group_variant(GroupBoxVariant::Outline)
            .page(
                // —— 通用设置页 ——
                SettingPage::new(t!("settings.general"))
                    .default_open(true)
                    .resettable(true)
                    .description(t!("settings.general_desc"))
                    .group(
                        SettingGroup::new()
                            .title(t!("settings.menu_position"))
                            .item(
                                SettingItem::new(
                                    t!("settings.tool_menu"),
                                    SettingField::render(move |_, _, cx| {
                                        let right =
                                            read_menu.read(cx).menu_position == MenuPosition::Right;
                                        let settings = read_menu.clone();
                                        // Switch 开关组件：左/右切换
                                        Switch::new("menu-position")
                                            .checked(right)
                                            .label(if right {
                                                t!("settings.right").to_string()
                                            } else {
                                                t!("settings.left").to_string()
                                            })
                                            .on_click(move |checked, _, cx| {
                                                settings.update(cx, |settings, cx| {
                                                    settings.set_menu_position(
                                                        if *checked {
                                                            MenuPosition::Right
                                                        } else {
                                                            MenuPosition::Left
                                                        },
                                                        cx,
                                                    );
                                                })
                                            })
                                    }),
                                )
                                .description(t!("settings.menu_position_desc").to_string())
                                .keywords(["菜单", "左侧", "右侧", "sidebar"])
                                // 重置为默认值（左侧）
                                .on_reset(
                                    move |cx| {
                                        dirty_menu.read(cx).menu_position != MenuPosition::Left
                                    },
                                    move |_, cx| {
                                        reset_menu.update(cx, |settings, cx| {
                                            settings.set_menu_position(MenuPosition::Left, cx)
                                        })
                                    },
                                ),
                            ),
                    ),
            )
            .page(
                // —— 外观设置页 ——
                SettingPage::new(t!("settings.appearance"))
                    .default_open(true)
                    .resettable(true)
                    .group(
                        SettingGroup::new().title(t!("settings.theme_mode")).item(
                            SettingItem::new(
                                t!("settings.theme"),
                                SettingField::render(move |_, _, _| {
                                    Select::new(&select)
                                        .accessibility_label(t!("settings.theme_mode"))
                                }),
                            )
                            .description(t!("settings.theme_desc").to_string())
                            .keywords(["主题", "白色", "黑色", "浅色", "深色", "跟随系统"])
                            // 重置为默认值（跟随系统）
                            .on_reset(
                                move |cx| dirty_theme.read(cx).theme != ThemeChoice::System,
                                move |_, cx| {
                                    reset_theme.update(cx, |settings, cx| {
                                        settings.set_theme(ThemeChoice::System, cx)
                                    })
                                },
                            ),
                        ),
                    )
                    .group(
                        SettingGroup::new().title(t!("settings.language")).item(
                            SettingItem::new(
                                t!("settings.interface_language"),
                                SettingField::render(move |_, _, _| {
                                    Select::new(&language_select)
                                        .accessibility_label(t!("settings.language"))
                                }),
                            )
                            .description(t!("settings.language_desc").to_string())
                            .keywords(["语言", "中文", "英文", "language"])
                            // 重置为默认值（简体中文）
                            .on_reset(
                                move |cx| dirty_language.read(cx).language != Language::ZhCN,
                                move |_, cx| {
                                    reset_language.update(cx, |settings, cx| {
                                        settings.set_language(Language::ZhCN, cx)
                                    })
                                },
                            ),
                        ),
                    )
                    .group(
                        SettingGroup::new().title(t!("settings.font")).item(
                            SettingItem::new(
                                t!("settings.code_font"),
                                SettingField::render(|_, _, cx| {
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            h_flex()
                                                .font_family(cx.theme().mono_font_family.clone())
                                                .child("JetBrains Mono"),
                                        )
                                        .child(
                                            div()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .child(t!("settings.code_font_desc")),
                                        )
                                }),
                            )
                            .keywords(["字体", "font", "JetBrains Mono"]),
                        ),
                    ),
            );

        // 如果存在保存错误，在设置面板顶部显示错误提示
        v_flex()
            .size_full()
            .gap_2()
            .when_some(self.settings.read(cx).save_error.clone(), |view, error| {
                view.child(Alert::error("settings-save-error", error))
            })
            .child(div().flex_1().min_h_0().p_2().child(panel))
    }
}