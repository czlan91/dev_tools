//! ## 图片 → Base64 工具
//!
//! 本工具将图片文件转换为 Base64 编码字符串和 Data URL。
//! 支持两种输入方式：文件选择器选择或拖入文件。
//!
//! ### Base64 是什么？
//!
//! Base64 是一种将二进制数据编码为可打印 ASCII 字符的编码方式，
//! 常用于在 HTML/CSS 中内嵌图片（Data URL）、通过 JSON 传输图片等场景。
//! 编码后的数据比原始二进制大约 33%。
//!
//! ### Data URL 格式
//!
//! ```text
//! data:image/png;base64,iVBORw0KGgoAAAANSUhEUg...
//! ```
//!
//! 这种格式可以直接在 HTML 的 `<img src="...">` 中使用，无需单独的文件请求。

use base64::{Engine, engine::general_purpose::STANDARD};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt, alert::Alert, button::Button, h_flex,
    label::Label, v_flex,
};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// 图片 → Base64 工具实体。
///
/// 支持选择/拖入图片，读取文件并在后台线程中编码为 Base64。
/// 使用 `request` 计数器确保并发场景下不会用旧结果覆盖新图片。
pub struct ImageTool {
    /// 当前加载的图片文件路径（`None` 表示尚无图片）。
    path: Option<Arc<Path>>,
    /// 文件名（用于状态栏显示）。
    name: String,
    /// Base64 编码后的字符串。
    output: String,
    /// 错误信息。
    error: Option<String>,
    /// 是否正在加载/编码中。
    loading: bool,
    /// 请求计数器。每次 `load` 调用递增，后台任务完成时检查
    /// 是否与当前计数一致，避免旧请求覆盖新请求的结果。
    request: u64,
    /// 检测到的 MIME 类型（如 `image/png`、`image/jpeg`）。
    mime_type: &'static str,
}

impl ImageTool {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            path: None,
            name: String::new(),
            output: String::new(),
            error: None,
            loading: false,
            request: 0,
            mime_type: "image/png",
        }
    }

    /// 加载并编码图片。
    ///
    /// 文件读取及编码在后台线程执行，避免阻塞 UI 线程。
    /// 选择文件和拖入文件共享同一条处理路径。
    /// 传入的路径会被规范化为绝对路径，确保图片预览能正确加载。
    fn load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        // 将路径规范化为绝对路径，确保 img() 预览能正确加载
        let path = path.canonicalize().unwrap_or(path);
        // 递增请求计数器，使之前的后台任务在完成时自动失效
        self.request += 1;
        let request = self.request;
        self.loading = true;
        self.error = None;
        self.path = None;
        self.output.clear();
        self.name = file_name(&path);
        cx.notify();

        // 在后台线程中执行文件读取和编码
        cx.spawn(async move |tool, cx| {
            let result = cx.background_executor().spawn(async move {
                let bytes = std::fs::read(&path).map_err(ImageError::Read)?;
                let mime = image_mime(&bytes).ok_or(ImageError::Unsupported)?;
                let output = STANDARD.encode(bytes);
                Ok::<_, ImageError>((path, mime, output))
            }).await;

            let _ = tool.update(cx, |tool, cx| {
                // 请求计数器不匹配，说明已有新请求，忽略旧结果
                if request != tool.request {
                    return;
                }
                tool.loading = false;
                match result {
                    Ok((path, mime, output)) => {
                        tool.path = Some(Arc::from(path));
                        tool.mime_type = mime;
                        tool.output = output;
                        log::info!(target: "tool.image", "图片编码完成，输出 {} 字符", tool.output.len());
                    }
                    Err(error) => {
                        log::warn!(target: "tool.image", "图片读取或验证失败: {error}");
                        tool.error = Some(error.to_string());
                    }
                }
                cx.notify();
            });
        }).detach();
    }

    /// 图片工具无编辑器，始终返回 `None`。
    pub fn cursor_position(&self, _cx: &App) -> Option<(u32, u32)> {
        None
    }
}

impl Render for ImageTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // —— 选择图片按钮 ——
        let open_btn = Button::new("img-open")
            .label("选择图片…")
            .small()
            .disabled(self.loading)
            .on_click(cx.listener(|_, _, _, cx| {
                let rx = cx.prompt_for_paths(PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: Some("选择图片文件".into()),
                });
                cx.spawn(async move |tool, cx| match rx.await {
                    Ok(Ok(Some(paths))) => {
                        if let Some(path) = paths.into_iter().next() {
                            let _ = tool.update(cx, |tool, cx| tool.load(path, cx));
                        }
                    }
                    Ok(Ok(None)) => {}
                    _ => {
                        let _ = tool.update(cx, |tool, cx| {
                            tool.error = Some("无法打开文件选择器，请尝试拖入图片。".into());
                            cx.notify();
                        });
                    }
                })
                .detach();
            }));

        // 构建 Data URL（用于复制和预览）
        let data_uri = (!self.output.is_empty() && self.error.is_none())
            .then(|| format!("data:{};base64,{}", self.mime_type, self.output));

        // —— 复制 Base64 按钮 ——
        let output_copy = self.output.clone();
        let copy_btn = Button::new("img-copy")
            .label("复制 Base64")
            .disabled(self.output.is_empty() || self.loading || self.error.is_some())
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.image", "已复制 Base64 到剪贴板");
            });

        // —— 复制 Data URL 按钮 ——
        let copy_uri = data_uri.clone();
        let copy_uri_btn = Button::new("img-copy-uri")
            .label("复制 Data URL")
            .small()
            .disabled(copy_uri.is_none())
            .on_click(move |_, _window, cx| {
                if let Some(uri) = copy_uri.clone() {
                    cx.write_to_clipboard(ClipboardItem::new_string(uri));
                    log::info!(target: "tool.image", "已复制 Data URL 到剪贴板");
                }
            });

        // —— 预览容器（居中显示图片，防止溢出） ——
        // 有图片时弹性伸缩，无图片时固定 150px 高度
        let preview_box = div()
            .when(self.path.is_some(), |this| this.flex_1().min_h_0())
            .when(self.path.is_none(), |this| this.min_h(px(150.)))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .overflow_hidden()
            .items_center()
            .justify_center()
            .child(match &self.path {
                Some(p) => img(Arc::clone(p))
                    .size_full()
                    .min_w_0()
                    .min_h_0()
                    .object_fit(ObjectFit::ScaleDown)
                    .into_any_element(),
                None => Label::new("暂无图片")
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .into_any_element(),
            });

        // —— Base64 输出框 ——
        let output_box = div()
            .id("img-output")
            .flex_1()
            .min_h_24()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .p_3()
            .overflow_scroll()
            .font_family(cx.theme().mono_font_family.clone())
            .text_sm()
            .child(match &self.error {
                Some(e) => Alert::error("image-error", e.clone()).into_any_element(),
                None => self.output.clone().into_any_element(),
            });

        // 状态提示：加载中/文件名
        let status = if self.loading {
            "读取中…".to_string()
        } else if !self.name.is_empty() {
            self.name.clone()
        } else {
            String::new()
        };

        v_flex()
            .id("image-drop-target")
            // 支持拖入图片文件
            .on_drop(cx.listener(|tool, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    tool.load(path.clone(), cx);
                }
            }))
            .size_full()
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .p_4()
                    .gap_3()
                    .child(
                        v_flex()
                            .gap_0p5()
                            .child(Label::new("图片 → Base64").text_lg().font_semibold())
                            .child(
                                Label::new("选择或拖入图片，生成 Base64 字符串 / Data URL")
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
                            .gap_1()
                            .child(Label::new("预览").text_sm().text_color(cx.theme().muted_foreground))
                            .child(preview_box),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Label::new("Base64 输出")
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(output_box),
                    ),
            )
    }
}

/// 从路径中提取文件名。
fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "image".to_string())
}

/// 图片处理错误类型。
///
/// 使用 `thiserror` 派生宏，提供语义化的错误消息。
/// `#[source]` 属性保留底层错误链，方便调试时查看完整上下文。
#[derive(Debug, thiserror::Error)]
enum ImageError {
    /// 文件读取失败（权限不足、文件不存在等）。
    #[error("读取图片失败，请检查文件是否存在及访问权限")]
    Read(#[source] std::io::Error),
    /// 文件内容不是支持的图片格式。
    #[error("无法识别图片格式，请选择 PNG、JPEG、GIF、WebP、BMP 或 SVG 图片")]
    Unsupported,
}

/// 通过文件内容（Magic Bytes）检测图片 MIME 类型。
///
/// 每种图片格式的文件头都有固定的字节序列，称为「魔数」（Magic Number）：
/// - PNG：`\x89PNG\r\n\x1a\n`
/// - JPEG：`\xff\xd8\xff`
/// - GIF：`GIF87a` 或 `GIF89a`
/// - WebP：`RIFF....WEBP`
/// - BMP：`BM`
/// - SVG：`<svg` 或 `<?xml` 包含 `<svg`
///
/// 通过检查文件头而不是文件扩展名，可以避免将任意文件或错误扩展名当成图片处理。
fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else if std::str::from_utf8(bytes).ok().is_some_and(|text| {
        let text = text.trim_start_matches('\u{feff}').trim_start();
        text.starts_with("<svg")
            || ((text.starts_with("<?xml") || text.starts_with("<!--")) && text.contains("<svg"))
    }) {
        Some("image/svg+xml")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::image_mime;

    /// 测试：MIME 类型检测基于文件内容而非扩展名。
    #[test]
    fn mime_comes_from_content() {
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\n"), Some("image/png"));
        assert_eq!(image_mime(b"RIFFxxxxWEBP"), Some("image/webp"));
        assert_eq!(image_mime(b"plain text"), None);
        assert_eq!(
            image_mime(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>"),
            Some("image/svg+xml")
        );
    }
}