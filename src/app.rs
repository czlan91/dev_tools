use gpui_kit::*;
use gpui_kit::component::{
    theme::{Theme, ThemeMode},
    StyledExt, h_flex, label::Label,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
};
use crate::{
    settings::{MenuPosition, Settings},
    theme::{self, ThemeChoice},
    tools::{
        image_to_base64::ImageTool, json_compare::JsonCompareTool,
        json_formatter::JsonFormatterTool, tsv_to_sql::TsvTool, ToolId,
    },
};
pub struct DevToolsApp {
    active: ToolId,
    settings: Entity<Settings>,
    tsv: Entity<TsvTool>,
    image: Entity<ImageTool>,
    json: Entity<JsonFormatterTool>,
    json_compare: Entity<JsonCompareTool>,
}
impl DevToolsApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = cx.new(|cx| Settings::new(window, cx));
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
            SidebarGroup::new("菜单位置").child(
                SidebarMenu::new().child(
                    SidebarMenuItem::new(match menu_position {
                        MenuPosition::Left => "菜单位置：左侧",
                        MenuPosition::Right => "菜单位置：右侧",
                    })
                    .active(true)
                    .on_click({
                        let settings = settings_entity.clone();
                        move |_, _, cx| {
                            let next = settings.read(cx).menu_position.toggle();
                            log::info!(target: "settings", "菜单位置切换为: {next:?}");
                            settings.update(cx, |s, cx| {
                                s.set_menu_position(next, cx);
                            });
                        }
                    }),
                ),
            ),
        )
        .child(
            SidebarGroup::new("主题").child(
                SidebarMenu::new()
                    .child(
                        SidebarMenuItem::new("白色")
                            .active(theme_choice == ThemeChoice::Light)
                            .on_click({
                                let settings = settings_entity.clone();
                                move |_, window, cx| {
                                    log::info!(target: "settings", "主题切换为: Light");
                                    settings.update(cx, |s, cx| s.set_theme(ThemeChoice::Light, cx));
                                    theme::apply(ThemeChoice::Light, window, cx);
                                }
                            }),
                    )
                    .child(
                        SidebarMenuItem::new("黑色")
                            .active(theme_choice == ThemeChoice::Dark)
                            .on_click({
                                let settings = settings_entity.clone();
                                move |_, window, cx| {
                                    log::info!(target: "settings", "主题切换为: Dark");
                                    settings.update(cx, |s, cx| s.set_theme(ThemeChoice::Dark, cx));
                                    theme::apply(ThemeChoice::Dark, window, cx);
                                }
                            }),
                    )
                    .child(
                        SidebarMenuItem::new("跟随系统")
                            .active(theme_choice == ThemeChoice::System)
                            .on_click({
                                let settings = settings_entity.clone();
                                move |_, window, cx| {
                                    log::info!(target: "settings", "主题切换为: System");
                                    settings.update(cx, |s, cx| s.set_theme(ThemeChoice::System, cx));
                                    theme::apply(ThemeChoice::System, window, cx);
                                }
                            }),
                    ),
            ),
        );
        let panel: AnyElement = match self.active {
            ToolId::TsvToSql => self.tsv.clone().into_any_element(),
            ToolId::ImageToBase64 => self.image.clone().into_any_element(),
            ToolId::JsonFormatter => self.json.clone().into_any_element(),
            ToolId::JsonCompare => self.json_compare.clone().into_any_element(),
        };
        match menu_position {
            MenuPosition::Left => h_flex().size_full().child(sidebar).child(panel),
            MenuPosition::Right => h_flex().size_full().child(panel).child(sidebar),
        }
    }
}
