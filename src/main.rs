//! ## 应用入口
//!
//! 本文件是 Dev Tools 的启动入口，职责包括：
//!
//! 1. **资源加载器（`FsAssetSource`）** —— 告诉 gpui 如何从文件系统加载资源文件
//!    （字体、图标等 GPUI 组件资源，以及运行时需要的其他文件）。
//! 2. **主函数（`main`）** —— 初始化日志、创建窗口、注册全局快捷键和菜单，
//!    然后启动 GPUI 事件循环。
//!
//! ### 关于 `anyhow`
//!
//! 本模块是项目中唯一保留 `anyhow` 的地方。原因：
//! gpui 的 `AssetSource` trait 的 `load` 方法签名返回 `anyhow::Result<Option<Cow<'_, [u8]>>>`，
//! 这是上游框架选用的错误类型，我们无法 —— 也不应该 —— 改变它。
//! 项目内部（`src/error.rs`）统一使用 `thiserror`，仅在 trait 边界处通过 `From` impl 转换。

mod app;
mod error;
mod settings;
mod tools;

use std::borrow::Cow;

use gpui_kit::prelude::*;
use gpui_kit::{px, size, AssetSource, Bounds, KeyBinding, Menu, MenuItem, OsAction, SharedString, TitlebarOptions, WindowBounds, WindowOptions};

use crate::app::{CloseWindow, Copy, Cut, NewFile, Paste, Redo, SelectAll, Undo};
use app::{AppRoot, OpenSettings, Quit};
use error::AppError;

/// 基于文件系统的资源加载器。
///
/// 实现了 `gpui::AssetSource` trait，用于加载 GPUI 组件库所需的资源文件。
///
/// ### 加载顺序
///
/// 1. 先尝试从 `gpui_kit` 内置的 `Assets` 中加载（组件自带的图标/字体等）。
/// 2. 如果内置资源中没有，则从文件系统路径读取（用户图片等运行时数据）。
///
/// 这种「先内嵌后文件系统」的优先级设计，确保组件依赖的图标资源不会因为
/// 文件路径问题而丢失，同时允许用户文件通过路径灵活加载。
struct FsAssetSource;

impl AssetSource for FsAssetSource {
    /// 加载指定路径的资源。
    ///
    /// 返回 `Ok(None)` 表示资源不存在（不是错误，调用方会做降级处理）；
    /// 返回 `Err` 表示真正发生了 I/O 错误。
    ///
    /// 注意：trait 签名要求 `anyhow::Result`，但内部错误统一用 `AppError` 处理，
    /// 在 return 时通过 `From<AppError> for anyhow::Error` 转换。
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        // 第 1 步：尝试从 GPUI 组件内置资源中加载
        // 组件图标必须由内嵌 Assets 提供，不能依赖文件系统路径
        if let Some(asset) = gpui_kit::assets::Assets.load(path)? {
            return Ok(Some(asset));
        }

        // 第 2 步：从文件系统读取（用户图片等运行时数据）
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(Cow::Owned(bytes))),
            Err(e) => {
                // 将 I/O 错误转换为 AppError，保留上下文
                let err = AppError::Io {
                    context: format!("加载资源: {path}"),
                    source: e,
                };
                log::warn!(target: "assets", "资源加载失败: {err}");
                Ok(None)
            }
        }
    }

    /// 列出指定路径下的资源文件。
    ///
    /// 委托给 GPUI 组件内置的 `Assets::list` 实现。
    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(_path)
    }
}

/// 应用入口函数。
///
/// ### 初始化流程
///
/// 1. **日志初始化** —— 配置 `env_logger`，默认级别为 `info`，可通过 `RUST_LOG` 环境变量控制。
/// 2. **创建应用** —— 通过 `gpui_kit::application()` 创建 GPUI 应用实例，注入资源加载器。
/// 3. **注册全局快捷键** —— `Cmd+Q` 退出、`Cmd+,` 打开设置。
/// 4. **设置系统菜单** —— macOS 顶部菜单栏（仅 macOS 平台可见）。
/// 5. **创建主窗口** —— 1120×720 的居中窗口，最小尺寸 920×560。
///
/// ### 关于 `env_logger`
///
/// `env_logger` 是一个轻量级的日志输出实现，它将 `log` crate 的日志消息
/// 写入 stderr。日志级别由环境变量 `RUST_LOG` 控制：
/// - 未设置时默认 `info`（关键操作与错误）
/// - `RUST_LOG=debug` 显示更详细的过程信息
/// - `RUST_LOG=tool.image=debug` 只过滤特定模块的日志
fn main() {
    // 初始化日志系统：从环境变量 RUST_LOG 读取配置，未设置时默认 info 级别
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Dev Tools 启动");

    // 创建 GPUI 应用实例，并注入自定义的资源加载器
    let application = gpui_kit::application().with_assets(FsAssetSource);

    application.run(move |cx| {
        // 初始化 GPUI 组件库（注册主题、样式系统等）
        gpui_kit::init(cx);
        // 设置组件库的语言环境为中文
        gpui_kit::component::set_locale("zh-CN");

        // ===== 注册全局快捷键 =====
        // `bind_keys` 将动作（Action）与键盘快捷键关联起来。
        // 当用户按下快捷键时，GPUI 会分发对应的 Action 事件。
        // `cmd-q` = Command + Q，`cmd-,` = Command + 逗号（打开设置）
        cx.bind_keys([
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("cmd-,", OpenSettings, None),
        ]);

        // ===== 设置系统菜单栏 =====
        // 仅在 macOS 上显示为顶部菜单栏，其他平台可能忽略。
        // 菜单项通过 `MenuItem::action` 与 Action 类型关联，
        // 按下时触发对应的 Action 事件。
        cx.set_menus(vec![
            Menu {
                name: "Dev Tools".into(),
                disabled: false,
                items: vec![
                    MenuItem::action("设置…", OpenSettings),
                    MenuItem::separator(),
                    MenuItem::action("退出 Dev Tools", Quit),
                ],
            },
            Menu {
                name: "文件".into(),
                items: vec![
                    MenuItem::action("新建", NewFile),
                    MenuItem::separator(),
                    MenuItem::os_action("剪切", Cut, OsAction::Cut),
                    MenuItem::os_action("复制", Copy, OsAction::Copy),
                    MenuItem::os_action("粘贴", Paste, OsAction::Paste),
                    MenuItem::separator(),
                    MenuItem::os_action("全选", SelectAll, OsAction::SelectAll),
                    MenuItem::separator(),
                    MenuItem::os_action("撤销", Undo, OsAction::Undo),
                    MenuItem::os_action("重做", Redo, OsAction::Redo),
                    MenuItem::separator(),
                    MenuItem::action("关闭窗口", CloseWindow),
                ],
                disabled: false,
            },
        ]);

        // —— 注册 Action 处理器 ——

        // `Quit` 动作：退出应用
        cx.on_action(|_: &Quit, cx| {
            log::info!("收到退出快捷键，退出应用");
            cx.quit();
        });

        // `OpenSettings` 动作：打开设置弹窗
        // 使用 `cx.defer` 将操作延迟到当前事件分发结束后执行，
        // 避免在动作分发过程中修改窗口状态。
        cx.on_action(|_: &OpenSettings, cx| {
            // 从全局状态中获取 AppRoot，它持有主应用的 Entity 和窗口句柄
            if let Some(root) = cx.try_global::<AppRoot>().cloned() {
                cx.defer(move |cx| {
                    if let Err(error) = cx.update_window(root.1, |_, window, cx| {
                        root.0.update(cx, |app, cx| app.open_settings(window, cx));
                    }) {
                        log::warn!(target: "settings", "打开设置窗口失败: {error}");
                    }
                });
            }
        });

        // ===== 创建主窗口 =====
        let options = WindowOptions {
            // 窗口尺寸：1120×720，居中显示
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.), px(720.)),
                cx,
            ))),
            // 最小窗口尺寸，防止拖得太小导致布局错乱
            window_min_size: Some(size(px(920.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Dev Tools".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        // 使用 `cx.spawn` 在异步上下文中打开窗口，
        // 使得窗口创建不阻塞主线程的事件循环初始化。
        cx.spawn(async move |cx| {
            let result = cx.open_window(options, |window, cx| {
                // 创建主应用实例（DevToolsApp），并放到全局状态中
                let app = cx.new(|cx| app::DevToolsApp::new(window, cx));
                cx.set_global(AppRoot(app.clone(), window.window_handle()));
                // 包裹在 gpui 的 Root 组件中（提供对话框层、通知层等）
                cx.new(|cx| gpui_kit::component::Root::new(app, window, cx))
            });

            match &result {
                Ok(_) => log::info!("主窗口已创建"),
                Err(e) => log::error!("创建主窗口失败: {e:#}"),
            }
            result.map(|_| ())
        })
        .detach();
    });
}
