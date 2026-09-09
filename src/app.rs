//! ## 应用主界面
//!
//! 本文件定义了应用的核心壳层 —— `DevToolsApp`，负责：
//!
//! - **工具切换**：通过侧边栏在不同的工具页面之间切换。
//! - **设置管理**：持有设置实体，观察主题变化并同步到界面。
//! - **布局组织**：侧边栏、工具面板、状态栏的布局组合。
//!
//! ### 结构概览
//!
//! ```text
//! ┌──────────────────────────────────────┐
//! │           侧边栏（Sidebar）            │
//! │  ┌ SQL 工具 ───────────────┐          │
//! │  │ ○ TSV → SQL IN          │          │
//! │  ├ 图片工具 ───────────────┤          │
//! │  │ ○ 图片 → Base64        │          │
//! │  ├ 数据工具 ───────────────┤          │
//! │  │ ○ JSON 格式化           │          │
//! │  │ ○ JSON5 格式化          │          │
//! │  │ ○ JSON 比较            │          │
//! │  └─────────────────────────┘          │
//! ├──────────────────────────────────────┤
//! │           工具面板（当前选中）          │
//! ├──────────────────────────────────────┤
//! │ 状态栏（StatusBar）                    │
//! │ Dev Tools v0.1.0       TSV → SQL IN  │
//! └──────────────────────────────────────┘
//! ```

use crate::{
    settings::{self, MenuPosition, Settings as AppSettings, ThemeChoice},
    tools::{
        ToolId, image_to_base64::ImageTool, json_compare::JsonCompareTool,
        json_formatter::JsonFormatterTool, tsv_to_sql::TsvTool,
    },
};
use gpui_kit::component::{
    Root, StyledExt, WindowExt, h_flex,
    label::Label,
    sidebar::{Sidebar, SidebarGroup, SidebarMenu, SidebarMenuItem},
    status_bar::StatusBar,
    v_flex,
};
use gpui_kit::*;

// ===== 宏说明：`actions!` =====
//
// ### 为什么需要用宏？
//
// GPUI 的 Action 系统要求每个动作（Action）是一个 Rust 类型，并且实现 `Action` trait。
// 手动为每个动作写 `struct Quit; impl Action for Quit {}` 非常繁琐。
//
// `actions!` 宏解决了这个问题：它同时完成三件事：
// 1. 定义结构体（`Quit`、`OpenSettings`）
// 2. 为它们实现 `Action` trait
// 3. 将动作注册到 GPUI 的全局动作系统中
//
// ### 主要成本
//
// - 宏展开后的代码对调试器不可见，开发者需要理解宏的语义才能知道生成了什么。
// - `#[action(no_json)]` 属性告诉 GPUI 不需要为这个动作生成 JSON 序列化代码，
//   因为我们的动作只在本进程内分发，不需要跨进程传递。
//
// 本项目中，`actions!` 用于定义两个全局快捷键对应的动作：
// - `Quit`：Cmd+Q 退出应用
// - `OpenSettings`：Cmd+, 打开设置
actions!([
    // 不在 JSON 中序列化这些动作 —— 它们只在本地进程中分发
    #[action(no_json)]
    Quit,
    #[action(no_json)]
    OpenSettings,
    #[action(no_json)]
    NewFile, // 新增：文件 → 新建
    #[action(no_json)]
    CloseWindow,
    #[action(no_json)]
    Cut,
    #[action(no_json)]
    Copy,
    #[action(no_json)]
    Paste,
    #[action(no_json)]
    SelectAll,
    #[action(no_json)]
    Undo,
    #[action(no_json)]
    Redo,
]);

/// 全局句柄，持有主应用实体和窗口句柄。
///
/// 之所以需要这个全局状态，是因为全局快捷键（如 Cmd+,）是在应用上下文
/// （application context）中分发的，而应用上下文无法直接访问 `DevToolsApp` 实例。
/// 通过 `cx.set_global(AppRoot(...))` 将主应用实体存放到全局状态中，
/// 快捷键处理器就可以通过 `cx.try_global::<AppRoot>()` 获取它。
#[derive(Clone)]
pub struct AppRoot(pub Entity<DevToolsApp>, pub AnyWindowHandle);

/// 实现 `Global` trait 表示该类型可以存放在 GPUI 的全局状态中。
///
/// 全局状态类似于一个类型安全的 HashMap：每种类型只能有一个实例，
/// 通过 `cx.set_global(...)` 写入，`cx.try_global::<T>()` 读取。
impl Global for AppRoot {}

/// 应用主界面实体。
///
/// 自己持有所用工具实体的引用，通过 `active` 字段控制当前显示哪个工具面板。
/// 之所以不切换时销毁重建，是因为每个工具都维护着自己的输入状态
/// （编辑器内容、复选框选项等），重建会导致用户输入丢失。
pub struct DevToolsApp {
    /// 当前激活的工具 ID，`render` 会根据此值选择显示哪个工具面板。
    pub(crate) active: ToolId,
    /// 设置实体，存储用户偏好（主题、菜单位置等）。
    settings: Entity<AppSettings>,
    /// 上次应用的主题，用于设置订阅的去重比较。
    ///
    /// 主题变化在事件阶段（`observe_in`）处理，`render` 只负责展示，
    /// 这样即使 `render` 被多次调用也不会重复应用主题。
    last_applied_theme: ThemeChoice,
    /// 设置面板实体（弹窗内容）。
    settings_panel: Entity<crate::settings::SettingsPanel>,
    /// 订阅集合，必须保持存活否则订阅会被自动取消。
    _subscriptions: Vec<Subscription>,
    /// 当前编辑器的光标位置文本（如"行 3，列 12"），空字符串表示无编辑器。
    cursor_position: String,
    // —— 以下为各工具实体，每个工具独立管理自己的状态 ——
    tsv: Entity<TsvTool>,
    image: Entity<ImageTool>,
    json: Entity<JsonFormatterTool>,
    json5: Entity<JsonFormatterTool>,
    json_compare: Entity<JsonCompareTool>,
}

impl DevToolsApp {
    /// 创建 `DevToolsApp` 实例。
    ///
    /// ### 初始化顺序
    ///
    /// 1. 创建设置实体，读取已保存的偏好。
    /// 2. 应用已保存的主题。
    /// 3. 创建设置面板。
    /// 4. 订阅设置变化和系统外观变化，以便在主题变化时自动更新。
    /// 5. 创建所有工具实体。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // 创建设置实体，从 ~/.config/dev_tools/settings.json 读取已保存的偏好
        let settings = cx.new(|cx| AppSettings::new(window, cx));
        let saved_theme = settings.read(cx).theme;

        // 应用保存的主题到当前窗口
        settings::apply(saved_theme, window, cx);

        // 创建设置面板（弹窗内容）
        let settings_panel =
            cx.new(|cx| crate::settings::SettingsPanel::new(settings.clone(), window, cx));

        // —— 订阅设置变化 ——
        // 使用 `observe_in` 在事件阶段（而非渲染阶段）处理主题变化。
        // 当 `settings` 实体发出 `cx.notify()` 时，这个回调会被调用。
        // 通过比较 `last_applied_theme` 避免重复应用相同的主题。
        let changed = cx.observe_in(&settings, window, |app, settings, window, cx| {
            let choice = settings.read(cx).theme;
            if choice != app.last_applied_theme {
                settings::apply(choice, window, cx);
                app.last_applied_theme = choice;
            }
            cx.notify();
        });

        // —— 订阅系统外观变化 ——
        // 当用户通过系统偏好切换深色/浅色模式时，自动更新主题。
        // 仅在设置为「跟随系统」模式时有效。
        let appearance = cx.observe_window_appearance(window, |app, window, cx| {
            if app.settings.read(cx).theme == ThemeChoice::System {
                settings::apply(ThemeChoice::System, window, cx);
            }
        });

        // —— 创建各工具实体 ——
        let tsv = cx.new(|cx| TsvTool::new(window, cx));
        let image = cx.new(|cx| ImageTool::new(window, cx));
        let json = cx.new(|cx| JsonFormatterTool::new(window, cx));
        let json5 = cx.new(|cx| JsonFormatterTool::new_json5(window, cx));
        let json_compare = cx.new(|cx| JsonCompareTool::new(window, cx));

        // —— 观察各工具实体，光标变化时重新渲染以更新状态栏 ——
        let observe_tsv = cx.observe(&tsv, |app, _, cx| {
            app.update_cursor(cx);
            cx.notify();
        });
        let observe_image = cx.observe(&image, |app, _, cx| {
            app.update_cursor(cx);
            cx.notify();
        });
        let observe_json = cx.observe(&json, |app, _, cx| {
            app.update_cursor(cx);
            cx.notify();
        });
        let observe_json5 = cx.observe(&json5, |app, _, cx| {
            app.update_cursor(cx);
            cx.notify();
        });
        let observe_compare = cx.observe(&json_compare, |app, _, cx| {
            app.update_cursor(cx);
            cx.notify();
        });

        Self {
            settings_panel,
            _subscriptions: vec![
                changed,
                appearance,
                observe_tsv,
                observe_image,
                observe_json,
                observe_json5,
                observe_compare,
            ],
            // 默认显示第一个工具：TSV → SQL IN
            active: ToolId::TsvToSql,
            settings,
            last_applied_theme: saved_theme,
            cursor_position: String::new(),
            tsv,
            image,
            json,
            json5,
            json_compare,
        }
    }

    /// 更新光标位置文本。
    fn update_cursor(&mut self, cx: &Context<Self>) {
        self.cursor_position = match self.active {
            ToolId::TsvToSql => self.tsv.read(cx).cursor_position(cx),
            ToolId::ImageToBase64 => self.image.read(cx).cursor_position(cx),
            ToolId::JsonFormatter => self.json.read(cx).cursor_position(cx),
            ToolId::Json5Formatter => self.json5.read(cx).cursor_position(cx),
            ToolId::JsonCompare => self.json_compare.read(cx).cursor_position(cx),
        }
        .map(|(line, col)| format!("行 {line}，列 {col}"))
        .unwrap_or_default();
    }

    /// 返回当前激活工具的名称，用于在状态栏右侧显示。
    fn tool_name(&self) -> &'static str {
        match self.active {
            ToolId::TsvToSql => "TSV → SQL IN",
            ToolId::ImageToBase64 => "图片 → Base64",
            ToolId::JsonFormatter => "JSON 格式化",
            ToolId::Json5Formatter => "JSON5 格式化",
            ToolId::JsonCompare => "JSON 比较",
        }
    }

    /// 打开设置弹窗。
    ///
    /// 使用 GPUI 的 `open_dialog` 方法在窗口上打开一个模态对话框。
    /// 对话框的尺寸会根据当前窗口大小和 rem 值动态计算，确保在不同
    /// 缩放比例下都能合理显示。
    pub fn open_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // 如果已经有一个活动的对话框，不再重复打开
        if window.has_active_dialog(cx) {
            return;
        }
        let panel = self.settings_panel.clone();
        log::info!(target: "settings", "打开设置弹窗");
        window.open_dialog(cx, move |dialog, window, _| {
            // 对话框宽度：最多 52 rem，但不能超过窗口宽度减去 4 rem 边距
            let width =
                (window.rem_size() * 52.).min(window.bounds().size.width - window.rem_size() * 4.);
            // 对话框高度：最多 29 rem，但不能超过窗口高度减去 10 rem 边距
            let height = (window.rem_size() * 29.)
                .min(window.bounds().size.height - window.rem_size() * 10.);
            dialog
                .title("设置")
                .width(width)
                .margin_top(window.rem_size() * 2.)
                .child(div().h(height).min_h_0().child(panel.clone()))
        });
    }
}

// ===== 辅助函数：构建侧边栏菜单项 =====
//
// 提取为单独的辅助函数，避免在 render 方法中重复相同的闭包模式。
// 每个菜单项都使用 `cx.entity().clone()` 模式来捕获实体引用，
// 而不是混合使用 `cx.listener` —— 保持一致性能减少读者的认知负担。

/// 创建一个侧边栏菜单项，点击后切换到指定工具。
///
/// 所有菜单项共享相同的模板：显示名称、当前激活状态和点击切换逻辑。
/// 提取为函数避免了在 `render_sidebar` 中重复五次类似的闭包代码。
fn sidebar_item(
    label: &'static str,
    tool_id: ToolId,
    active: bool,
    root: &Entity<DevToolsApp>,
) -> SidebarMenuItem {
    SidebarMenuItem::new(label).active(active).on_click({
        let root = root.clone();
        move |_, _, cx| {
            log::info!("切换到工具: {label}");
            root.update(cx, |app, cx| {
                app.active = tool_id;
                cx.notify();
            });
        }
    })
}

impl DevToolsApp {
    /// 渲染侧边栏（导航菜单）。
    ///
    /// 包含所有可用工具的列表，按类别分组：
    /// - SQL 工具：TSV → SQL IN
    /// - 图片工具：图片 → Base64
    /// - 数据工具：JSON 格式化、JSON5 格式化、JSON 比较
    fn render_sidebar(&self, cx: &mut Context<Self>) -> Sidebar<SidebarGroup<SidebarMenu>> {
        let root = cx.entity();
        let menu_position = self.settings.read(cx).menu_position;

        Sidebar::<SidebarGroup<SidebarMenu>>::new("sidebar")
            .side(match menu_position {
                MenuPosition::Left => gpui_kit::component::Side::Left,
                MenuPosition::Right => gpui_kit::component::Side::Right,
            })
            .collapsible(false)
            .w_56()
            .header(Label::new("Dev Tools").text_base().font_semibold())
            // —— SQL 工具组 ——
            .child(
                SidebarGroup::new("SQL 工具").child(SidebarMenu::new().child(sidebar_item(
                    "TSV → SQL IN",
                    ToolId::TsvToSql,
                    self.active == ToolId::TsvToSql,
                    &root,
                ))),
            )
            // —— 图片工具组 ——
            .child(
                SidebarGroup::new("图片工具").child(SidebarMenu::new().child(sidebar_item(
                    "图片 → Base64",
                    ToolId::ImageToBase64,
                    self.active == ToolId::ImageToBase64,
                    &root,
                ))),
            )
            // —— 数据工具组 ——
            .child(
                SidebarGroup::new("数据工具").child(
                    SidebarMenu::new()
                        .child(sidebar_item(
                            "JSON 格式化",
                            ToolId::JsonFormatter,
                            self.active == ToolId::JsonFormatter,
                            &root,
                        ))
                        .child(sidebar_item(
                            "JSON5 格式化",
                            ToolId::Json5Formatter,
                            self.active == ToolId::Json5Formatter,
                            &root,
                        ))
                        .child(sidebar_item(
                            "JSON 比较",
                            ToolId::JsonCompare,
                            self.active == ToolId::JsonCompare,
                            &root,
                        )),
                ),
            )
    }

    /// 渲染当前激活的工具面板。
    ///
    /// 根据 `self.active` 的值选择对应的工具实体进行渲染。
    /// 每个工具实体在 `DevToolsApp::new` 中创建并持有，切换时不会销毁重建。
    fn render_active_panel(&self) -> AnyElement {
        match self.active {
            ToolId::TsvToSql => self.tsv.clone().into_any_element(),
            ToolId::ImageToBase64 => self.image.clone().into_any_element(),
            ToolId::JsonFormatter => self.json.clone().into_any_element(),
            ToolId::Json5Formatter => self.json5.clone().into_any_element(),
            ToolId::JsonCompare => self.json_compare.clone().into_any_element(),
        }
    }
}

impl Render for DevToolsApp {
    /// 渲染应用界面。
    ///
    /// 每次调用 `cx.notify()` 时，GPUI 会重新调用此方法生成新的界面描述。
    /// 布局分为三部分：
    /// 1. **侧边栏** —— 列出所有可用的工具，点击切换。
    /// 2. **工具面板** —— 当前选中工具的具体界面。
    /// 3. **状态栏** —— 底部状态信息。
    ///
    /// 根据 gpui-kit 编码指南的建议，将 render 保持声明式，
    /// 复杂区域提取为 `render_xxx` 辅助方法。
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_position = self.settings.read(cx).menu_position;

        // 使用辅助方法构建各区域（替代内联的长链调用）
        let sidebar = self.render_sidebar(cx);
        let panel = self.render_active_panel();

        // 根据菜单位置决定侧边栏和内容区的排列顺序
        let content = match menu_position {
            MenuPosition::Left => h_flex()
                .items_stretch()
                .flex_1()
                .min_h_0()
                .child(sidebar) // 侧边栏在左
                .child(div().flex_1().min_w_0().child(panel)), // 内容区在右
            MenuPosition::Right => h_flex()
                .items_stretch()
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().child(panel)) // 内容区在左
                .child(sidebar), // 侧边栏在右
        };

        // 构建状态栏：左侧显示版本号，右侧显示工具名 + 光标位置
        let right_text = if self.cursor_position.is_empty() {
            self.tool_name().to_string()
        } else {
            format!("{} | {}", self.tool_name(), self.cursor_position)
        };
        let status_bar = StatusBar::new().left("Dev Tools v0.1.0").right(right_text);

        // 组合完整布局
        // `Root::render_dialog_layer` 和 `Root::render_notification_layer` 分别渲染
        // 对话框层和通知层，它们覆盖在主内容之上。
        v_flex()
            .size_full()
            .relative()
            .on_action(
                cx.listener(|app, _: &OpenSettings, window, cx| app.open_settings(window, cx)),
            )
            .child(content)
            .child(status_bar)
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
