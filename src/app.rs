use gpui_kit::*;
use gpui_kit::component::{
    status_bar::StatusBar,
    theme::{Theme, ThemeMode},
    StyledExt, h_flex, v_flex, label::Label,
    setting::{self, SettingGroup, SettingItem, SettingPage, Settings},
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
};
use crate::{
    settings::{MenuPosition, Settings as AppSettings},
    theme::{self, ThemeChoice},
    tools::{
        image_to_base64::ImageTool, json_compare::JsonCompareTool,
        json_formatter::JsonFormatterTool, tsv_to_sql::TsvTool, ToolId,
    },
};

pub struct DevToolsApp {
    active: ToolId,
    settings: Entity<AppSettings>,
    tsv: Entity<TsvTool>,
    image: Entity<ImageTool>,
    json: Entity<JsonFormatterTool>,
    json_compare: Entity<JsonCompareTool>,
}

impl DevToolsApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = cx.new(|cx| AppSettings::new(window, cx));
        let saved_theme = settings.read(cx).theme;
        theme::apply(saved_theme, window, cx);
        Self {
            active: ToolId::TsvToSql,
            settings,
            tsv: cx.new(|cx| TsvTool::new(window, cx)),
            image: cx.new(|cx| ImageTool::new(window, cx)),
            json: cx.new(|cx| JsonFormatterTool::new(window, cx)),
            json_compare: cx.new(|cx| JsonCompareTool::new(window, cx)),
        }
    }

    fn tool_name(&self) -> &'static str {
        match self.active {
            ToolId::TsvToSql => "TSV → SQL IN",
            ToolId::ImageToBase64 => "图片 → Base64",
            ToolId::JsonFormatter => "JSON 格式化",
            ToolId::JsonCompare => "JSON 比较",
            ToolId::Settings => "设置",
        }
    }
}

impl Render for DevToolsApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = cx.entity();
        let settings_entity = self.settings.clone();
        let (menu_position, theme_choice) = {
            let s = self.settings.read(cx);
            (s.menu_position, s.theme)
        };
        if theme_choice == ThemeChoice::System {
            let target = ThemeMode::from(window.appearance());
            let current = Theme::global(cx).mode;
            if target != current {
                theme::apply(ThemeChoice::System, window, cx);
            }
        }
        let sidebar = Sidebar::<SidebarGroup<SidebarMenu>>::new("sidebar")
            .side(match menu_position {
                MenuPosition::Left => gpui_kit::component::Side::Left,
                MenuPosition::Right => gpui_kit::component::Side::Right,
            })
        .collapsible(false)
        .w(px(216.))
        .header(Label::new("Dev Tools").text_base().font_semibold())
        .child(
            SidebarGroup::new("SQL 工具").child(SidebarMenu::new().child(
                SidebarMenuItem::new("TSV → SQL IN")
                    .active(self.active == ToolId::TsvToSql)
                    .on_click({
                        let root = root.clone();
                        move |_, _, cx| {
                            log::info!("切换到工具: TsvToSql");
                            root.update(cx, |app, cx| {
                                app.active = ToolId::TsvToSql;
                                cx.notify();
                            });
                        }
                    }),
            )),
        )
        .child(
            SidebarGroup::new("图片工具").child(SidebarMenu::new().child(
                SidebarMenuItem::new("图片 → Base64")
                    .active(self.active == ToolId::ImageToBase64)
                    .on_click({
                        let root = root.clone();
                        move |_, _, cx| {
                            log::info!("切换到工具: ImageToBase64");
                            root.update(cx, |app, cx| {
                                app.active = ToolId::ImageToBase64;
                                cx.notify();
                            });
                        }
                    }),
            )),
        )
        .child(
            SidebarGroup::new("数据工具").child(SidebarMenu::new()
                .child(
                    SidebarMenuItem::new("JSON 格式化")
                        .active(self.active == ToolId::JsonFormatter)
                        .on_click({
                            let root = root.clone();
                            move |_, _, cx| {
                                log::info!("切换到工具: JsonFormatter");
                                root.update(cx, |app, cx| {
                                    app.active = ToolId::JsonFormatter;
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    SidebarMenuItem::new("JSON 比较")
                        .active(self.active == ToolId::JsonCompare)
                        .on_click({
                            let root = root.clone();
                            move |_, _, cx| {
                                log::info!("切换到工具: JsonCompare");
                                root.update(cx, |app, cx| {
                                    app.active = ToolId::JsonCompare;
                                    cx.notify();
                                });
                            }
                        }),
                ),
            ),
        )
        .child(
            SidebarGroup::new("设置").child(SidebarMenu::new().child(
                SidebarMenuItem::new("设置")
                    .active(self.active == ToolId::Settings)
                    .on_click({
                        let root = root.clone();
                        move |_, _, cx| {
                            log::info!("切换到工具: Settings");
                            root.update(cx, |app, cx| {
                                app.active = ToolId::Settings;
                                cx.notify();
                            });
                        }
                    }),
            )),
        );

        let panel: AnyElement = match self.active {
            ToolId::TsvToSql => self.tsv.clone().into_any_element(),
            ToolId::ImageToBase64 => self.image.clone().into_any_element(),
            ToolId::JsonFormatter => self.json.clone().into_any_element(),
            ToolId::JsonCompare => self.json_compare.clone().into_any_element(),
            ToolId::Settings => {
                let s = settings_entity.clone();
                let app_root = root.clone();
                Settings::new("app-settings")
                    .sidebar_width(px(180.))
                    .page(
                        SettingPage::new("通用设置")
                            .description("菜单位置、主题等全局设置")
                            .resettable(true)
                            .group(
                                SettingGroup::new()
                                    .title("菜单位置")
                                    .description("切换菜单在左侧和右侧之间显示")
                                    .item({
                                        let s1 = s.clone();
                                        let s2 = s.clone();
                                        let r1 = app_root.clone();
                                        SettingItem::new("菜单位置", setting::SettingField::switch(
                                            move |cx| {
                                                s1.read(cx).menu_position == MenuPosition::Left
                                            },
                                            move |checked, cx| {
                                                let pos = if checked { MenuPosition::Left } else { MenuPosition::Right };
                                                s2.update(cx, |s, cx| s.set_menu_position(pos, cx));
                                                r1.update(cx, |_, cx| cx.notify());
                                            },
                                        ).default_value(true))
                                    })
                            )
                            .group(
                                SettingGroup::new()
                                    .title("主题模式")
                                    .description("选择浅色、深色或跟随系统主题")
                                    .item({
                                        let s1 = s.clone();
                                        let s2 = s.clone();
                                        let r1 = app_root.clone();
                                        let theme_names: Vec<(SharedString, SharedString)> = vec![
                                            ("light".into(), "白色".into()),
                                            ("dark".into(), "黑色".into()),
                                            ("system".into(), "跟随系统".into()),
                                        ];
                                        SettingItem::new("主题", setting::SettingField::<SharedString>::dropdown(
                                            theme_names,
                                            move |cx| {
                                                let t = s1.read(cx).theme;
                                                match t {
                                                    ThemeChoice::Light => "light",
                                                    ThemeChoice::Dark => "dark",
                                                    ThemeChoice::System => "system",
                                                }.into()
                                            },
                                            move |val, cx| {
                                                let choice = match val.as_ref() {
                                                    "light" => ThemeChoice::Light,
                                                    "dark" => ThemeChoice::Dark,
                                                    _ => ThemeChoice::System,
                                                };
                                                s2.update(cx, |s, cx| {
                                                    s.set_theme(choice, cx);
                                                });
                                                r1.update(cx, |_, cx| cx.notify());
                                            },
                                        ).default_value("system"))
                                    }),
                            ),
                    )
                    .into_any_element()
            }
        };

        let content = match menu_position {
            MenuPosition::Left => h_flex()
                .flex_1()
                .min_h_0()
                .child(sidebar)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .child(panel),
                ),
            MenuPosition::Right => h_flex()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .child(panel),
                )
                .child(sidebar),
        };

        let status_bar = StatusBar::new()
            .left("Dev Tools v0.1.0")
            .right(self.tool_name());

        v_flex()
            .size_full()
            .child(content)
            .child(status_bar)
    }
}