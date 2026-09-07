use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    v_flex,
};
pub struct ImageTool {
    path: Option<Arc<Path>>,
    name: String,
    output: String,
    error: Option<String>,
    loading: bool,
}
impl ImageTool {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            path: None,
            name: String::new(),
            output: String::from("-- 选择图片后生成 Base64"),
            error: None,
            loading: false,
        }
    }
    fn mime(path: &Path) -> &'static str {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref()
        {
            Some("jpg") | Some("jpeg") => "image/jpeg",
            Some("gif") => "image/gif",
            Some("webp") => "image/webp",
            Some("svg") => "image/svg+xml",
            Some("bmp") => "image/bmp",
            _ => "image/png",
        }
    }
}
impl Render for ImageTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let open_btn = Button::new("img-open")
            .primary()
            .label("选择图片…")
            .small()
            .on_click(move |_, _window, cx| {
                let tool = tool.clone();
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("选择图片文件".into()),
                });
                cx.spawn(async move |cx| {
                    let paths = match rx.await {
                        Ok(Ok(Some(paths))) => paths,
                        _ => {
                            log::debug!(target: "tool.image", "用户取消了图片选择");
                            return;
                        }
                    };
                    let Some(first) = paths.into_iter().next() else {
                        return;
                    };
                    let path: PathBuf = first;
                    log::info!(target: "tool.image", "选中图片: {}", path.display());
                    let read_path = path.clone();
                    let bytes = cx
                        .background_executor()
                        .spawn(async move { std::fs::read(read_path) })
                        .await;
                    let (name, output, error) = match bytes {
                        Err(e) => {
                            log::error!(target: "tool.image", "读取图片失败 path={}: {e}", path.display());
                            (
                                file_name(&path),
                                String::new(),
                                Some(format!("读取文件失败: {e}")),
                            )
                        }
                        Ok(data) => {
                            let b64 = STANDARD.encode(&data);
                            log::info!(
                                target: "tool.image",
                                "图片读取成功: {} 字节 -> {} 字符",
                                data.len(),
                                b64.len()
                            );
                            (
                                file_name(&path),
                                format!(
                                    "{}\n\n(原始 {} 字节 · 编码后 {} 字符)",
                                    b64,
                                    data.len(),
                                    b64.len()
                                ),
                                None,
                            )
                        }
                    };
                    let _ = cx.update(|cx| {
                        tool.update(cx, |t, cx| {
                            t.path = Some(Arc::from(path.clone()));
                            t.name = name;
                            t.output = output;
                            t.error = error;
                            t.loading = false;
                            cx.notify();
                        });
                    });
                })
                .detach();
            });
        let data_uri = self.path.as_ref().and_then(|p| {
            self.output
                .lines()
                .next()
                .map(|b64| format!("data:{};base64,{}", Self::mime(p), b64))
        });
        let output_copy = self.output.clone();
        let copy_btn = Button::new("img-copy")
            .label("复制 Base64")
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.image", "已复制 Base64 到剪贴板");
            });
        let copy_uri = data_uri.clone();
        let copy_uri_btn = Button::new("img-copy-uri")
            .label("复制 Data URI")
            .small()
            .disabled(copy_uri.is_none())
            .on_click(move |_, _window, cx| {
                if let Some(uri) = copy_uri.clone() {
                    cx.write_to_clipboard(ClipboardItem::new_string(uri));
                    log::info!(target: "tool.image", "已复制 Data URI 到剪贴板");
                }
            });
        let preview: AnyElement = match &self.path {
            Some(p) => img(Arc::clone(p))
                .size_full()
                .min_w_0()
                .min_h_0()
                .object_fit(ObjectFit::Contain)
                .into_any_element(),
            None => v_flex()
                .size_full()
                .items_center()
                .justify_center()
                .child(
                    Label::new("暂无图片")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .into_any_element(),
        };
        let output_box = div()
            .id("img-output")
            .flex_1()
            .min_h(px(100.))
            .rounded(px(8.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .p_3()
            .overflow_scroll()
            .font_family("JetBrains Mono")
            .text_sm()
            .child(match &self.error {
                Some(e) => Label::new(e.clone())
                    .text_color(cx.theme().danger)
                    .into_any_element(),
                None => self.output.clone().into_any_element(),
            });
        let status = if self.loading {
            "读取中…".to_string()
        } else if !self.name.is_empty() {
            self.name.clone()
        } else {
            String::new()
        };
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new("图片 → Base64").text_lg().font_semibold())
                    .child(
                        Label::new("选择图片文件，生成 Base64 字符串 / Data URI")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(open_btn)
                    .child(copy_btn)
                    .child(copy_uri_btn)
                    .when(!status.is_empty(), |this| {
                        this.child(
                            Label::new(status)
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                    }),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(120.))
                    .gap_1()
                    .child(
                        Label::new("预览")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h(px(120.))
                            .rounded(px(8.))
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().muted)
                            .child(preview),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(100.))
                    .gap_1()
                    .child(
                        Label::new("Base64 输出")
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(output_box),
            )
    }
}
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string())
}
