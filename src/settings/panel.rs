//! ## 设置面板视图
//!
//! 本文件负责渲染设置弹窗的 UI 界面。`SettingsPanel` 实体持有控件状态
//! （如下拉选择框的当前选项），但所有持久偏好数据由 `Settings` 实体统一管理。
//!
//! ### 设计原则
//!
//! - **视图与状态分离**：`SettingsPanel` 只负责 UI 渲染和控件交互事件的转发，
//!   实际的设置值读写通过 `Settings` 实体完成。
//! ### 设置项分布说明
//!
//! `Settings` 结构体中定义了以下持久化设置：
//!
//! | 字段 | 在 `SettingsPanel` 中 | 实际控制位置 |
//! |-----------------|----------------------|----------------------------|
//! | `theme` | 主题下拉框 + 重置 | 设置面板「外观」页 |
//! | `language` | 语言下拉框 + 重置 | 设置面板「外观」页 |
//! | `menu_position` | ❌ 不在面板中 | `app.rs` 状态栏 Toggle 按钮 |
//!
//! `menu_position`（侧边栏在左/右）之所以没有放在设置面板中，是因为它是一个
//! **频繁切换的布局偏好**——用户可能一天内多次调整侧边栏位置来适应不同工作场景。
//! 放在状态栏的 Toggle 按钮中只需一次点击即可切换，比打开设置面板再找到对应选项
//! 要快捷得多。这遵循了「高频操作用快捷入口，低频配置用设置面板」的设计原则。
//! - **可重置**：每个设置项都支持「重置为默认值」，通过 `on_reset` 回调实现。
//! - **实时保存**：用户修改任何设置后立即通过 `Settings` 实体保存到文件。
//!
//! ### 架构说明
//!
//! 本文件采用 GPUI 框架的组件化模式构建：
//! - `SettingsPanel` 实现 `Render` trait 来声明式描述 UI 结构
//! - 使用 `subscribe_in` 监听子控件事件（自下而上通信）
//! - 使用 `observe_in` 监听数据模型变化（自上而下同步）
//! - `SelectState` 作为受控组件，其选中值由外部驱动而非内部维护
//!
//! ### 页面布局结构
//!
//! 设置面板整体的页面布局分为两层：
//!
//! ```text
//! ┌─ SettingsView ────────────────────────────────────┐
//! │  ┌─── 侧边栏 ───┐┌─── 内容区 ───────────────────┐ │
//! │  │  [搜索框]     ││  SettingPage                  │ │
//! │  │  ─────────    ││  ├─ SettingGroup (卡片)       │ │
//! │  │  [外观]  ◀选中 ││  │  └─ SettingItem           │ │
//! │  │  [快捷键]     ││  │     ├── 控件（靠右）       │ │
//! │  │  ...         ││  │     ├── 描述文字           │ │
//! │  │              ││  │     └── [重置] 按钮         │ │
//! │  │              ││  └─ SettingGroup               │ │
//! │  └──────────────┘└───────────────────────────────┘ │
//! └────────────────────────────────────────────────────┘
//! ```
//!
//! - **`SettingsView`**：设置面板的核心容器，自带侧边栏导航 + 内容区域的双栏布局。
//! - **`SettingPage`**：一个设置页 = 侧边栏的一个导航条目 + 右侧一整块内容区。
//! - **`SettingGroup`**：页内分组，视觉上以卡片（Outline 变体）分隔不同类别的设置项。
//! - **`SettingItem`**：单个设置项，包含标签、控件、描述、关键词、重置按钮。
//! - **`SettingPage`**：一个设置页 = 侧边栏的一个导航条目 + 右侧一整块内容区。
//! - **`SettingGroup`**：页内分组，视觉上以卡片（Outline 变体）分隔不同类别的设置项。
//! - **`SettingItem`**：单个设置项，包含标签、控件、描述、关键词、重置按钮。

use super::{Language, Settings, ThemeChoice};

// GPUI Kit 组件库 —— 这些组件封装了 GPUI 原语，提供开箱即用的交互行为。
// 注意它们的引入路径层级：component 模块下是具体控件，prelude 下是 FluentBuilder
// 这样的 trait 扩展，lib 下是 gpui 核心类型的重导出。
use gpui_kit::component::{
    ActiveTheme,
    group_box::GroupBoxVariant,
    h_flex,
    select::{Select, SelectEvent, SelectState},
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings as SettingsView},
    v_flex,
};
use gpui_kit::*;
use rust_i18n::t;

/// 设置面板实体。
///
/// ### 职责边界
/// 本实体只处理「显示什么」和「用户点了怎么办」，不处理「设置值存哪里、怎么存」。
/// 所有持久化逻辑委托给 `Settings` 实体 —— 这是 GPUI 中常见的「视图实体 + 数据实体」分离模式。
///
/// ### 字段设计
/// 字段都是 `Entity<T>` 而非 `T` 本身，原因有二：
/// 1. `Entity` 是 GPUI 的引用计数句柄，可以在闭包中 clone 后独立使用，绕过借用检查器的生命周期约束
/// 2. 下拉框的 `SelectState` 本身是一个独立实体，GPUI 框架要求在事件订阅时传 `Entity`，
///    这样才能通过 `cx.subscribe_in` 建立起稳定的监听通道
pub struct SettingsPanel {
    /// 设置实体引用，用于读取和写入持久化偏好。
    ///
    /// 通过 `Entity::read(cx)` 获取不可变引用读取当前设置值，
    /// 通过 `Entity::update(cx, ...)` 获取可变引用写入新设置值。
    /// 注意：不能在 `Render::render` 中调用 `update`（渲染期间不允许修改状态），
    /// 所以所有修改操作都在事件回调中执行。
    settings: Entity<Settings>,

    /// 主题选择下拉框的控件状态。
    ///
    /// `SelectState<Vec<SharedString>>` 表示一个选项列表为字符串列表的下拉框。
    /// 泛型参数 `Vec<SharedString>` 是选项的数据类型，`SelectState` 封装了选中索引、
    /// 展开/收起状态等交互逻辑。
    theme_select: Entity<SelectState<Vec<SharedString>>>,

    /// 语言选择下拉框的控件状态。
    language_select: Entity<SelectState<Vec<SharedString>>>,

    /// 订阅集合，必须保持存活否则订阅会被自动取消。
    ///
    /// GPUI 的 `Subscription` 遵循 RAII 模式：当 `Subscription` 值被 drop 时，
    /// 对应的订阅会自动取消。因此必须将订阅保存在实体的字段中，
    /// 而不能在 `new` 中创建后丢弃。这就是 `let _ = cx.subscribe_in(...)` 不可行的原因。
    _subscriptions: Vec<Subscription>,
}

impl SettingsPanel {
    /// 创建设置面板实例。
    ///
    /// ### 初始化流程
    /// 1. 创建两个下拉框的 `SelectState` 实体
    /// 2. 从 `Settings` 中读取当前值，同步设置下拉框的选中项
    /// 3. 建立双向同步订阅：
    ///    - 下拉框选择事件 → 更新设置（用户触发）
    ///    - 设置实体变化 → 更新下拉框（代码触发，如重置操作）
    ///
    /// ### 参数说明
    /// - `settings`：设置数据实体，由调用方（通常是父组件）创建并传入
    /// - `window`：GPUI 窗口引用，某些控件（如下拉框的弹出层定位）需要窗口信息
    /// - `cx`：当前实体的可变上下文，用于创建子实体、订阅事件等
    pub fn new(settings: Entity<Settings>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // ===== 1. 创建主题选择下拉框 =====
        // 提供三个选项：白色（Light）、黑色（Dark）、跟随系统（System）。
        // `None` 表示初始没有选中项，后续通过 `set_selected_value` 设置。
        // 这个设计模式是「先创建控件，再同步数据」—— 因为创建时需要 `&mut Context`，
        // 而 `settings.read(cx)` 也需要 `&Context`，所以分两步做。
        let theme_select: Entity<SelectState<Vec<SharedString>>> = cx.new(|cx| {
            SelectState::new(
                vec!["白色".into(), "黑色".into(), "跟随系统".into()],
                None,
                window,
                cx,
            )
        });

        // 根据当前设置，选中对应的下拉框选项。
        // 这里用 `update` 而不是 `read`，因为 `set_selected_value` 需要修改内部状态。
        // `cx` 是 `&mut Context<Self>`，但 `theme_select` 是独立的实体，
        // GPUI 允许多个实体之间的借用，只要不在同一层级同时持有可变引用。
        theme_select.update(cx, |select, cx| {
            select.set_selected_value(&theme_label(settings.read(cx).theme), window, cx)
        });

        // ===== 2. 创建语言选择下拉框 =====
        let language_select: Entity<SelectState<Vec<SharedString>>> = cx.new(|cx| {
            SelectState::new(vec!["简体中文".into(), "English".into()], None, window, cx)
        });
        language_select.update(cx, |select, cx| {
            select.set_selected_value(&language_label(settings.read(cx).language), window, cx)
        });

        // ===== 3. 建立双向同步订阅 =====

        // --- 订阅 A：下拉框事件 → 更新设置 ---
        //
        // `cx.subscribe_in(&theme_select, window, ...)` 的含义是：
        // 「监听 theme_select 实体发到当前窗口的所有事件」。
        // 第一个泛型参数 `Entity<SelectState<Vec<SharedString>>>` 是发布者类型，
        // 由编译器推导，无需显式标注。
        //
        // 回调参数 `(publisher, event, window, cx)`：
        // - `publisher` 是事件发布者的 `Entity` 引用，这里不需要使用
        // - `event` 是 `SelectState` 发出的事件枚举，我们只关心 `SelectEvent::Confirm`
        // - `window` 和 `cx` 是当前实体的上下文，用于访问或修改状态
        //
        // 为什么需要 `let settings = settings.clone()`？
            // 因为闭包通过 move 捕获 `settings`，而 `settings` 在后续还会被使用。
        // `Entity::clone()` 只增加引用计数，不会克隆底层数据，成本极低。
        let selected = cx.subscribe_in(&theme_select, window, {
            let settings = settings.clone();
            move |_, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    // 将中文标签映射为 ThemeChoice 枚举值
                    // 注意：如果将来支持更多主题或更改了选项标签，这里需要同步更新
                    let choice = match value.as_ref() {
                        "白色" => ThemeChoice::Light,
                        "黑色" => ThemeChoice::Dark,
                        _ => ThemeChoice::System,
                    };
                    settings.update(cx, |settings, cx| settings.set_theme(choice, cx));
                }
            }
        });

        // 订阅语言下拉框的选择事件 → 更新设置实体。
        // `set_language` 内部会调用 `rust_i18n::set_locale` 切换全局 locale，
        // 因此语言的切换不仅影响此面板，还会刷新整个 UI 的本地化字符串。
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

        // --- 订阅 B：设置变化 → 同步下拉框 ---
        //
        // `cx.observe_in(&settings, window, ...)` 与 `subscribe_in` 不同：
        // - `subscribe_in` 监听实体发出的任意事件（事件驱动）
        // - `observe_in` 监听实体的「变更通知」（由 `cx.notify()` 触发），
        //   即每当被观察实体调用了 `cx.notify()` 后，观察者会在下一帧渲染前收到回调。
        //
        // 这里监听 `Settings` 实体的变化，在「重置」按钮通过代码修改设置时，
        // 同步更新下拉框的选中项。如果不做这一步，点击重置后下拉框会显示
        // 旧值，与实际生效的设置不一致。
        //
        // 注意：回调中只更新下拉框的显示值，不会再次触发选择事件
        // （`set_selected_value` 不会发出 `SelectEvent::Confirm`），
        // 因此不会有「设置→同步→再触发设置」的反馈循环。
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
            // 通知当前实体重新渲染，以下拉框的新状态更新 UI
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
///
/// 这是一个纯函数，没有副作用。之所以提取为单独函数而不是在 `Render` 中 inline，
/// 因为 `new` 和 `Render` 两个地方都需要用到这个转换逻辑。
fn theme_label(choice: ThemeChoice) -> SharedString {
    match choice {
        ThemeChoice::Light => "白色",
        ThemeChoice::Dark => "黑色",
        ThemeChoice::System => "跟随系统",
    }
    .into()
}

/// 将 `Language` 枚举值转换为显示标签。
///
/// 标签使用用户的母语（而非代码中的英文枚举名），
/// 这样在「重置」按钮的脏状态判断（比较字符串）时语义一致。
fn language_label(language: Language) -> SharedString {
    match language {
        Language::ZhCN => "简体中文",
        Language::En => "English",
    }
    .into()
}

impl Render for SettingsPanel {
    /// 渲染设置面板的 UI 结构。
    ///
    /// 注意：GPUI 的 `render` 方法在每一帧都可能被调用，
    /// 因此这里不能有副作用（不能修改状态、不能发起异步操作等），
    /// 只能根据当前状态构建 UI 元素树并返回。
    ///
    /// ### 提前 clone 的原因
    /// 由于 `render` 中会构建多个闭包（每个 `.item(...)` 内的 `.on_reset(...)` 等），
    /// 每个闭包 move 捕获自己的变量。如果直接在闭包中引用 `self.settings.clone()`，
    /// 会导致闭包捕获 `self` 的引用。GPUI 的渲染阶段已经持有 `&mut self`，
    /// 不允许在闭包中再借用 `self`。因此必须在闭包外提前 clone 好 `Entity` 句柄。
    ///
    /// 命名约定：
    /// - `select`：用于 Select 控件渲染的句柄
    /// - `dirty_*`：用于脏状态检查（`on_reset` 的第一个闭包，判断是否可重置）
    /// - `reset_*`：用于执行重置操作（`on_reset` 的第二个闭包）
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // 由于 `render` 中会多次克隆 `self.settings`，这里提前克隆好引用
        // 供闭包捕获。`Entity` 是引用计数的句柄，克隆成本很低（只增加引用计数）。
        let select = self.theme_select.clone();
        let dirty_theme = self.settings.clone();
        let reset_theme = self.settings.clone();
        let language_select = self.language_select.clone();
        let dirty_language = self.settings.clone();
        let reset_language = self.settings.clone();

        // ==========================================================
        // SettingsView：设置面板的核心布局
        // ==========================================================
        //
        // `SettingsView` 是 GPUI Kit 提供的设置面板容器组件，内部实现了一个
        // **侧边栏-内容区双栏布局**。下面详细说明其各层级的页面布局设计。
        //
        // ┌─────────────────────────────────────────────────────┐
        // │ SettingsView (整个设置面板)                          │
        // │  ┌──────────┬──────────────────────────────────────┐│
        // │  │ 侧边栏    │ 内容区域                             ││
        // │  │ (12rem)  │ (flex 1)                            ││
        // │  │          │                                      ││
        // │  │ [搜索框]  │ ┌─ SettingPage ────────────────────┐││
        // │  │ ───────── │ │  ┌─ SettingGroup (卡片) ───────┐ │││
        // │  │ [外观]    │ │  │  SettingItem                 │ │││
        // │  │  ◀ 选中   │ │  │  ├── [控件]                 │ │││
        // │  │ [快捷键]  │ │  │  ├── 描述文字               │ │││
        // │  │           │ │  │  └── [重置] 按钮            │ │││
        // │  │           │ │  └─────────────────────────────┘ │││
        // │  │           │ └──────────────────────────────────┘││
        // │  └──────────┴──────────────────────────────────────┘│
        // └─────────────────────────────────────────────────────┘
        //
        // ### 布局参数
        //
        // `sidebar_width(window.rem_size() * 12.)` —— 侧边栏宽度为 12 rem。
        // 使用 `rem_size()` 乘以倍数，而非固定像素值，使得布局能跟随窗口的
        // 字体大小缩放。`rem_size()` 返回窗口当前的根字体大小（以像素为单位），
        // 因此 12 rem 意味着「大约 12 个字符的宽度」。
        //
        // ### 分组变体
        //
        // `with_group_variant(GroupBoxVariant::Outline)` 控制 `SettingGroup`
        // 的视觉样式。Outline 变体会在每个 Group 周围渲染一个带标题的边框卡片，
        // 视觉上清晰区分不同类别的设置。如果不设置（默认 Normal 变体），
        // 各个 Group 之间只通过间距分隔，缺乏明确的边界。
        //
        // ### 页面管理
        //
        // 通过链式调用 `.page(...)` 添加设置页。每个 `.page(...)` 调用会：
        // 1. 在侧边栏中注册一个导航条目（显示 `SettingPage::new` 的标题）
        // 2. 在内容区域中创建对应的设置内容
        // 3. 用户点击侧边栏条目时，自动切换内容区显示对应的页面
        //
        // 这些 `.page(...)` 之间的顺序决定了侧边栏导航条目的排列顺序。

        let panel = SettingsView::new("app-settings")
            .sidebar_width(window.rem_size() * 12.)
            .with_group_variant(GroupBoxVariant::Outline)
            .page(
                // =====================================================
                // SettingPage：单个设置页
                // =====================================================
                //
                // `SettingPage::new(t!("settings.appearance"))` 创建一页设置。
                // 参数是页面的标题，显示在左侧侧边栏的导航条目中，以及内容区顶部的标题。
                //
                // ### 页面级属性
                //
                // - `default_open(true)`：此页在面板首次打开时默认展开。如果只有一个页面，
                //   通常设为 true；有多个页面时可以只让第一个页面默认展开。
                // - `resettable(true)`：在页面标题旁显示一个「重置所有」按钮。
                //   点击后遍历页面下所有 `SettingItem`，对有脏状态的项执行重置操作。
                //
                // ### 页面内的布局
                //
                // 内容区从上到下依次渲染 page 标题 → 搜索过滤 → 各 `SettingGroup`。
                // 每个 `SettingGroup` 占据一整行宽度，垂直排列。
                SettingPage::new(t!("settings.appearance"))
                    .default_open(true)
                    .resettable(true)
                    .group(
                        // =============================================
                        // SettingGroup：设置分组（带卡片边框）
                        // =============================================
                        //
                        // `SettingGroup::new().title(...)` 创建一个设置分组。
                        // title 显示在卡片边框的左上角，作为分组标题。
                        //
                        // 当 `with_group_variant(Outline)` 生效时，每个 Group
                        // 渲染为一个带圆角边框的卡片容器，title 浮在边框线上。
                        // 这种布局类似于 macOS 系统偏好设置的分类方式。
                        //
                        // 一个 Group 内可以包含多个 `SettingItem`，垂直堆叠排列。
                        // 如果 Group 内有多个 Item，它们之间会自动添加分隔线。
                        //
                        // ┌── 主题模式 ─────────────────────────────────┐
                        // │  主题       [白色 ▾]                        │
                        // │  选择应用的界面主题配色                     │
                        // │                           [重置]            │
                        // └─────────────────────────────────────────────┘
                        SettingGroup::new().title(t!("settings.theme_mode")).item(
                            // =========================================
                            // SettingItem：单个设置项
                            // =========================================
                            //
                            // `SettingItem::new(label, control)` 创建一个设置项。
                            //
                            // 内部布局从上到下分三层：
                            // 1. 标签行：`label` 文字 + 右侧的控件（如 Select 下拉框）
                            // 2. 描述行：`description(...)` 设置的灰色小字说明文字
                            // 3. 重置按钮：`.on_reset(...)` 注册，当脏状态为 true 时显示
                            //
                            // 这三层在同一行中按以下方式排列：
                            // ┌─ 标签 ──────────────┬─ 控件（下拉框等） ─┐
                            // │  主题               │  [白色 ▾]         │
                            // ├─ 描述 ──────────────┴────────────────────┤
                            // │  选择应用的界面主题配色                     │
                            // ├─────────────── [重置] ────────────────────┤
                            //
                            // `SettingField::render(closure)` 用于渲染控件部分。
                            // 闭包接收 `(window, SettingItemState, cx)` 三个参数，
                            // 通常第一个和第二个参数用 `_` 忽略，因为我们不需要
                            // 窗口引用和 Item 状态来判断如何渲染控件。
                            SettingItem::new(
                                t!("settings.theme"),
                                SettingField::render(move |_, _, _| {
                                    Select::new(&select)
                                        .accessibility_label(t!("settings.theme_mode"))
                                }),
                            )
                            .description(t!("settings.theme_desc").to_string())
                            .keywords(["主题", "白色", "黑色", "浅色", "深色", "跟随系统"])
                            // === 重置逻辑详解 ===
                            //
                            // `on_reset` 接收两个闭包，形成「脏状态判定 → 重置执行」的两段式模式：
                            //
                            // 闭包 1（脏状态检查）：`move |cx| dirty_theme.read(cx).theme != ThemeChoice::System`
                            //   - 返回 `true` 时，UI 上会显示一个「重置」按钮。
                            //   - 返回 `false` 时，不显示重置按钮，表示此项已经是默认值。
                            //   - 每次面板渲染时都会调用此闭包，确保重置按钮的显隐即时更新。
                            //
                            // 闭包 2（重置执行）：`move |_, cx| { reset_theme.update(cx, ...) }`
                            //   - 仅在用户点击重置按钮时被调用。
                            //   - 第一个参数是 `Window` 引用（这里用不到，因为设置修改不需要窗口操作）。
                            //   - 内部通过 `settings.set_theme(ThemeChoice::System, cx)` 将主题设回默认值。
                            //
                            // 两个闭包各有一个 `Entity<Settings>` 的 clone（`dirty_theme` 和 `reset_theme`），
                            // 它们指向同一个底层数据。`dirty_theme` 只用 `read`（读取检查），
                            // `reset_theme` 只用 `update`（写入修改），分工明确。
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
                        // —— 语言分组 ——
                        //
                        // 结构与主题分组完全对称：
                        // - 使用 Select 下拉框选择语言
                        // - 支持重置为默认值（简体中文）
                        //
                        // 重置脏状态判断：`language != Language::ZhCN`
                        // 之所以用 `!=` 而非 `>` 等比较，是因为只有两种语言选项，
                        // 不存在「比默认值更靠后」这样的语义。
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
                        // —— 字体分组 ——
                        //
                        // 这是只读展示项，没有交互控件，也没有重置按钮。
                        // 用于向用户展示当前代码字体，并说明如何自定义。
                        //
                        // 注意不同于上面的两个 Group，这里的 `SettingField::render`
                        // 闭包**没有**使用 `move` 关键字 —— 因为不需要捕获任何外部变量。
                        //
                        // 此 Group 展示了另一种 Item 形态：不包含 Select 等标准控件，
                        // 而是用 `v_flex` + `h_flex` 自定义布局，渲染字体预览效果。
                        //
                        // 字体预览使用 `cx.theme().mono_font_family` 设置等宽字体，
                        // 让用户看到「JetBrains Mono」用实际等宽字体渲染的效果。
                        SettingGroup::new().title(t!("settings.font")).item(
                            SettingItem::new(
                                t!("settings.code_font"),
                                SettingField::render(|_, _, cx| {
                                    v_flex()
                                        .gap_2()
                                        .child(
                                            // 等宽字体名称预览：
                                            // 使用 `mono_font_family` 字体族渲染，
                                            // 让用户在设置面板中直观看到代码字体的实际效果。
                                            h_flex()
                                                .font_family(cx.theme().mono_font_family.clone())
                                                .child("JetBrains Mono"),
                                        )
                                        .child(
                                            // 辅助说明文字：
                                            // 使用 `muted_foreground` 颜色和 `text_sm` 小字号，
                                            // 在视觉上弱化，作为次要信息层级。
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

        panel
    }
}