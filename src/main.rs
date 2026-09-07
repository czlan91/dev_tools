mod app;
mod error;
mod settings;
mod theme;
mod tools;

use std::borrow::Cow;

use gpui_kit::prelude::*;
use gpui_kit::{
    actions, AssetSource, Bounds, KeyBinding, SharedString, TitlebarOptions,
    WindowBounds, WindowOptions, px, size,
};

use error::AppError;

actions!([#[action(no_json)] Quit]);

struct FsAssetSource;

impl AssetSource for FsAssetSource {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(Cow::Owned(bytes))),
            Err(e) => {
                let _err = AppError::from(e);
                log::warn!(target: "assets", "加载资源失败 path={path}: {_err}");
                Ok(None)
            }
        }
    }

    fn list(&self, _path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(vec![])
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    log::info!("Dev Tools 启动");

    let application = gpui_kit::application().with_assets(FsAssetSource);

    application.run(move |cx| {
        // 注册全局快捷键：Cmd+Q 退出应用。
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.on_action(|_: &Quit, cx| {
            log::info!("收到退出快捷键，退出应用");
            cx.quit();
        });

        gpui_kit::init(cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1120.), px(720.)),
                cx,
            ))),
            window_min_size: Some(size(px(920.), px(560.))),
            titlebar: Some(TitlebarOptions {
                title: Some("Dev Tools".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            let result = cx.open_window(options, |window, cx| {
                let app = cx.new(|cx| app::DevToolsApp::new(window, cx));
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