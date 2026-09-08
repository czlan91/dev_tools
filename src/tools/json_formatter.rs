//! ## JSON 格式化工具
//!
//! 本工具提供 JSON 文件的多种处理功能：
//!
//! - **JSON 格式化**：将压缩的 JSON 展开为带缩进的易读格式。
//! - **JSON5 格式化**：解析 JSON5 格式（支持注释、尾逗号、单引号等）并输出为标准 JSON。
//! - **Key 排序**：递归排序对象中的所有 Key，使输出结果稳定可预测。
//! - **压缩**：移除所有空白字符，输出单行紧凑 JSON。
//! - **转义/取消转义**：将字符串转为 JSON 字符串字面量，或从 JSON 字符串字面量恢复。
//!
//! 该工具同时支持「JSON 格式化」和「JSON5 格式化」两种模式，
//! 通过 `json5_mode` 字段区分。两种模式使用相同的编辑器控件，
//! 但切换时不会丢失各自的输入内容（因为各持有独立的实体实例）。

use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    alert::Alert,
    button::Button,
    h_flex,
    input::{Editor, EditorState},
    label::Label,
    v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

/// JSON 格式化工具实体。
///
/// 持有输入编辑器和输出文本，通过按钮操作触发各种处理功能。
/// `json5_mode` 标记当前实例是 JSON 还是 JSON5 模式。
pub struct JsonFormatterTool {
    /// 输入编辑器状态（支持语法高亮）。
    input_state: Entity<EditorState>,
    /// 是否为 JSON5 模式（影响按钮标签和解析器选择）。
    json5_mode: bool,
    /// 处理后的输出文本。
    output: String,
    /// 错误信息（`None` 表示无错误）。
    error: Option<String>,
    /// 订阅集合，保持存活否则自动取消。
    _subscriptions: Vec<Subscription>,
}

impl JsonFormatterTool {
    /// 创建 JSON 格式化工具实例（标准 JSON 模式）。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")       // 启用 JSON 语法高亮
                .placeholder("在此粘贴 JSON / JSON5 文本…")
        });
        // 观察编辑器光标变化，通知工具实体重新渲染以更新状态栏
        let observe = cx.observe(&input_state, |_, _, cx| cx.notify());
        Self {
            json5_mode: false,
            input_state,
            output: String::new(),
            error: None,
            _subscriptions: vec![observe],
        }
    }

    /// 创建独立 JSON5 格式化工具实例。
    ///
    /// 复用 `JsonFormatterTool` 的处理逻辑，但标记为 JSON5 模式。
    /// 持有自己的输入编辑器，切换工具时不会丢失输入内容。
    pub fn new_json5(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut tool = Self::new(window, cx);
        tool.json5_mode = true;
        tool
    }

    // —— 以下为静态工具方法，不依赖实例状态 ——

    /// 解析 JSON5 文本为 `serde_json::Value`。
    fn parse5(text: &str) -> Result<serde_json::Value, super::json_utils::JsonError> {
        super::json_utils::parse(text, true)
    }

    /// 格式化 JSON 文本（带缩进）。
    fn format_json(text: &str) -> Result<String, super::json_utils::JsonError> {
        Ok(serde_json::to_string_pretty(&super::json_utils::parse(
            text, false,
        )?)?)
    }

    /// 格式化 JSON5 文本（解析后输出为标准 JSON 格式）。
    fn format_json5(text: &str) -> Result<String, super::json_utils::JsonError> {
        Ok(serde_json::to_string_pretty(&Self::parse5(text)?)?)
    }

    /// 递归排序 JSON 对象中的所有 Key。
    fn sort_keys(value: serde_json::Value) -> serde_json::Value {
        super::json_utils::sort_keys(value)
    }

    /// 压缩 JSON 文本（移除多余空白）。
    fn compress_json(text: &str) -> Result<String, super::json_utils::JsonError> {
        Ok(serde_json::to_string(&super::json_utils::parse(
            text, false,
        )?)?)
    }

    /// 转义字符串为 JSON 字符串字面量。
    fn escape_json(text: &str) -> String {
        super::json_utils::escape(text)
    }

    /// 取消转义 JSON 字符串字面量。
    ///
    /// 输入必须是合法的 JSON 字符串字面量（如 `"hello \"world\""`），
    /// 且必须是一个字符串值（不是对象或数组）。
    fn unescape_json(text: &str) -> Result<String, super::json_utils::JsonError> {
        match super::json_utils::parse(text, false)? {
            serde_json::Value::String(value) => Ok(value),
            _ => Err(super::json_utils::JsonError::NotString),
        }
    }

    /// 返回编辑器光标位置（行号、列号，从 1 开始）。
    pub fn cursor_position(&self, cx: &App) -> Option<(u32, u32)> {
        let pos = self.input_state.read(cx).cursor_position();
        Some((pos.line + 1, pos.character + 1))
    }
}

impl Render for JsonFormatterTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let input = self.input_state.clone();

        // —— JSON 格式化按钮 ——
        // 仅在非 JSON5 模式下显示
        let fmt_json_btn = Button::new("json-format")
            .label("JSON 格式化")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::format_json(&raw) {
                        Ok(out) => {
                            log::info!(target: "tool.json", "JSON 格式化成功，输出 {} 字符", out.len());
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.error = None;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "JSON 解析失败: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e.to_string());
                                cx.notify();
                            });
                        }
                    }
                }
            });

        // —— JSON5 格式化按钮 ——
        // 仅在 JSON5 模式下显示
        let fmt_json5_btn = Button::new("json5-format")
            .label("JSON5 格式化")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::format_json5(&raw) {
                        Ok(out) => {
                            log::info!(target: "tool.json", "JSON5 格式化成功，输出 {} 字符", out.len());
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.error = None;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "JSON5 解析失败: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e.to_string());
                                cx.notify();
                            });
                        }
                    }
                }
            });

        // —— Key 排序按钮 ——
        let sort_btn = Button::new("json-sort")
            .label("排序 key")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::parse5(&raw) {
                        Ok(value) => {
                            let out = serde_json::to_string_pretty(&Self::sort_keys(value))
                                .map_err(|e| e.to_string())
                                .unwrap_or_default();
                            log::info!(target: "tool.json", "key 排序完成，输出 {} 字符", out.len());
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.error = None;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "解析失败（排序）: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e.to_string());
                                cx.notify();
                            });
                        }
                    }
                }
            });

        // —— 压缩按钮 ——
        let compress_btn = Button::new("json-compress")
            .label("压缩")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::compress_json(&raw) {
                        Ok(out) => {
                            log::info!(target: "tool.json", "JSON 压缩成功，输出 {} 字符", out.len());
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.error = None;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "JSON 压缩失败: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e.to_string());
                                cx.notify();
                            });
                        }
                    }
                }
            });

        // —— 转义按钮 ——
        let escape_btn = Button::new("json-escape").label("转义").small().on_click({
            let tool = tool.clone();
            let input = input.clone();
            move |_, _window, cx| {
                let raw = input.read(cx).value().to_string();
                let out = Self::escape_json(&raw);
                log::info!(target: "tool.json", "JSON 转义完成，输出 {} 字符", out.len());
                tool.update(cx, |t, cx| {
                    t.output = out;
                    t.error = None;
                    cx.notify();
                });
            }
        });

        // —— 取消转义按钮 ——
        let unescape_btn = Button::new("json-unescape")
            .label("取消转义")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::unescape_json(&raw) {
                        Ok(out) => {
                            log::info!(target: "tool.json", "JSON 取消转义成功，输出 {} 字符", out.len());
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.error = None;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "JSON 取消转义失败: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e.to_string());
                                cx.notify();
                            });
                        }
                    }
                }
            });

        // —— 复制结果按钮 ——
        let output_copy = self.output.clone();
        let copy_btn = Button::new("json-copy")
            .label("复制结果")
            .disabled(self.output.is_empty() || self.error.is_some())
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.json", "已复制 JSON 工具结果到剪贴板");
            });

        let theme = cx.theme();
        // 输出框：显示处理结果或错误信息
        let output_box = div()
            .id("json-output")
            .flex_1()
            .min_h_24()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .p_3()
            .overflow_scroll()
            .font_family("JetBrains Mono")
            .text_sm()
            .child(match &self.error {
                Some(e) => Alert::error("json-error", e.clone()).into_any_element(),
                None => self.output.clone().into_any_element(),
            });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(
                        Label::new(if self.json5_mode {
                            "JSON5 格式化"
                        } else {
                            "JSON 格式化"
                        })
                        .text_lg()
                        .font_semibold(),
                    )
                    .child(
                        Label::new("格式化 / JSON5 格式化 / key 排序 / 自动转义 / 压缩")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_1()
                    .child(
                        Label::new("输入")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .child(Editor::new(&self.input_state).h_full()),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .when(!self.json5_mode, |row| row.child(fmt_json_btn))
                    .when(self.json5_mode, |row| row.child(fmt_json5_btn))
                    .child(sort_btn)
                    .child(compress_btn)
                    .child(escape_btn)
                    .child(unescape_btn)
                    .child(copy_btn),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h_24()
                    .gap_1()
                    .child(
                        Label::new("输出")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    )
                    .child(output_box),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::JsonFormatterTool;

    /// 测试：JSON 格式化生成带缩进的输出。
    #[test]
    fn format_json_pretty() {
        let out = JsonFormatterTool::format_json(r#"{"a":1,"b":[2,3]}"#).unwrap();
        assert_eq!(out, "{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}");
    }

    /// 测试：紧凑输入也能正确格式化。
    #[test]
    fn format_json_compact_input() {
        let out = JsonFormatterTool::format_json(r#"{"x":{"y":true}}"#).unwrap();
        assert!(out.contains("\n  \"x\": {\n    \"y\": true\n  }"));
    }

    /// 测试：无效 JSON 返回错误。
    #[test]
    fn format_json_invalid_returns_error() {
        let err = JsonFormatterTool::format_json(r#"{"a":}"#).unwrap_err();
        assert!(!err.to_string().is_empty());
    }

    /// 测试：空字符串应该返回错误。
    #[test]
    fn format_json_empty_errors() {
        assert!(JsonFormatterTool::format_json("").is_err());
    }

    /// 测试：Key 排序递归生效。
    #[test]
    fn sort_keys_recursive() {
        let value = serde_json::json!({
            "zebra": {"z": 1, "a": 2},
            "alpha": [
                {"b": 1, "a": 2},
                {"d": 1, "c": 2}
            ]
        });
        let sorted = JsonFormatterTool::sort_keys(value);
        // 对象 Key 按字母顺序排列
        assert_eq!(
            sorted.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["alpha", "zebra"]
        );
        let zebra = &sorted["zebra"];
        assert_eq!(
            zebra.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["a", "z"]
        );
        // 数组中的对象也递归排序
        let arr = sorted["alpha"].as_array().unwrap();
        assert_eq!(
            arr[0].as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(
            arr[1].as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["c", "d"]
        );
    }

    /// 测试：排序结果具有确定性（不同输入顺序产生相同输出）。
    #[test]
    fn sort_keys_is_deterministic() {
        let a = JsonFormatterTool::sort_keys(serde_json::json!({"b":1,"a":2}));
        let b = JsonFormatterTool::sort_keys(serde_json::json!({"a":2,"b":1}));
        assert_eq!(
            serde_json::to_string_pretty(&a).unwrap(),
            serde_json::to_string_pretty(&b).unwrap()
        );
    }

    /// 测试：压缩移除所有空白。
    #[test]
    fn compress_json_removes_whitespace() {
        let out = JsonFormatterTool::compress_json(r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        assert_eq!(out, r#"{"a":1,"b":[2,3]}"#);
    }

    /// 测试：压缩无效 JSON 返回错误。
    #[test]
    fn compress_json_invalid_errors() {
        assert!(JsonFormatterTool::compress_json("not json").is_err());
    }

    /// 测试：转义正确处理引号。
    #[test]
    fn escape_json_quotes() {
        let out = JsonFormatterTool::escape_json(r#"hello "world""#);
        assert_eq!(out, r#""hello \"world\"""#);
    }

    /// 测试：取消转义往返一致性。
    #[test]
    fn unescape_json_roundtrip() {
        let original = "hello\nworld\t\"test\"";
        let escaped = JsonFormatterTool::escape_json(original);
        let unescaped = JsonFormatterTool::unescape_json(&escaped).unwrap();
        assert_eq!(original, unescaped);
    }
}