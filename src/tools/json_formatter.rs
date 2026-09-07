use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::{
    ActiveTheme, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Textarea, TextareaState},
    label::Label,
    h_flex, v_flex,
};

/// JSON 格式化工具页：格式化 / JSON5 格式化 / key 排序 / 自动转义 / 压缩。
pub struct JsonFormatterTool {
    input_state: Entity<TextareaState>,
    output: String,
    error: Option<String>,
}

impl JsonFormatterTool {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            input_state: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("在此粘贴 JSON / JSON5 文本…")
            }),
            output: String::from("-- 点击按钮生成结果"),
            error: None,
        }
    }

    /// 解析 JSON5（JSON 的超集），统一返回 `serde_json::Value`。
    fn parse5(text: &str) -> Result<serde_json::Value, String> {
        json5::from_str(text).map_err(|e| e.to_string())
    }

    /// 纯函数：格式化为标准 JSON（2 空格缩进）。
    fn format_json(text: &str) -> Result<String, String> {
        let value = Self::parse5(text)?;
        serde_json::to_string_pretty(&value).map_err(|e| e.to_string())
    }

    /// 递归排序对象 key（字典序）后输出标准 JSON。
    fn sort_keys(value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(map) => {
                let sorted: std::collections::BTreeMap<String, serde_json::Value> =
                    map.into_iter().collect();
                serde_json::Value::Object(
                    sorted
                        .into_iter()
                        .map(|(k, v)| (k, Self::sort_keys(v)))
                        .collect(),
                )
            }
            serde_json::Value::Array(items) => {
                serde_json::Value::Array(items.into_iter().map(Self::sort_keys).collect())
            }
            other => other,
        }
    }

    /// 纯函数：压缩 JSON 为单行。
    fn compress_json(text: &str) -> Result<String, String> {
        let value = Self::parse5(text)?;
        serde_json::to_string(&value).map_err(|e| e.to_string())
    }

    /// 纯函数：将输入文本转义为 JSON 字符串字面量。
    /// 例如输入 `hello"world` → `"hello\"world"`。
    fn escape_json(text: &str) -> String {
        serde_json::to_string(text).unwrap_or_else(|_| format!("\"{}\"", text))
    }

    /// 纯函数：将 JSON 字符串字面量还原为原始文本。
    /// 例如输入 `"hello\"world"` → `hello"world`。
    fn unescape_json(text: &str) -> Result<String, String> {
        serde_json::from_str::<String>(text).map_err(|e| e.to_string())
    }
}

impl Render for JsonFormatterTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let input = self.input_state.clone();

        let fmt_json_btn = Button::new("json-format")
            .primary()
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
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

        let fmt_json5_btn = Button::new("json5-format")
            .label("JSON5 格式化")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input.clone();
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    match Self::format_json(&raw) {
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
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

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
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

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
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

        let escape_btn = Button::new("json-escape")
            .label("转义")
            .small()
            .on_click({
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
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

        let output_copy = self.output.clone();
        let copy_btn = Button::new("json-copy")
            .label("复制结果")
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.json", "已复制 JSON 工具结果到剪贴板");
            });

        let theme = cx.theme();
        let output_box = div()
            .id("json-output")
            .flex_1()
            .min_h(px(120.))
            .rounded(px(8.))
            .border_1()
            .border_color(theme.border)
            .bg(theme.background)
            .p_3()
            .overflow_scroll()
            .font_family("JetBrains Mono")
            .text_sm()
            .child(match &self.error {
                Some(e) => Label::new(e.clone())
                    .text_color(theme.danger)
                    .into_any_element(),
                None => self.output.clone().into_any_element(),
            });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new("JSON 格式化").text_lg().font_semibold())
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
                            .child(Textarea::new(&self.input_state).h_full()),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(fmt_json_btn)
                    .child(fmt_json5_btn)
                    .child(sort_btn)
                    .child(compress_btn)
                    .child(escape_btn)
                    .child(unescape_btn)
                    .child(copy_btn),
            )
            .child(
                v_flex()
                    .flex_1()
                    .min_h(px(120.))
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

    #[test]
    fn format_json_pretty() {
        let out = JsonFormatterTool::format_json(r#"{"a":1,"b":[2,3]}"#).unwrap();
        assert_eq!(
            out,
            "{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}"
        );
    }

    #[test]
    fn format_json_compact_input() {
        let out = JsonFormatterTool::format_json(r#"{"x":{"y":true}}"#).unwrap();
        assert!(out.contains("\n  \"x\": {\n    \"y\": true\n  }"));
    }

    #[test]
    fn format_json_invalid_returns_error() {
        let err = JsonFormatterTool::format_json(r#"{"a":}"#).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn format_json_empty_errors() {
        assert!(JsonFormatterTool::format_json("").is_err());
    }

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
        assert_eq!(
            sorted.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["alpha", "zebra"]
        );
        let zebra = &sorted["zebra"];
        assert_eq!(
            zebra.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["a", "z"]
        );
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

    #[test]
    fn sort_keys_is_deterministic() {
        let a = JsonFormatterTool::sort_keys(serde_json::json!({"b":1,"a":2}));
        let b = JsonFormatterTool::sort_keys(serde_json::json!({"a":2,"b":1}));
        assert_eq!(
            serde_json::to_string_pretty(&a).unwrap(),
            serde_json::to_string_pretty(&b).unwrap()
        );
    }

    #[test]
    fn compress_json_removes_whitespace() {
        let out = JsonFormatterTool::compress_json(r#"{"a": 1, "b": [2, 3]}"#).unwrap();
        // 压缩后应为单行，不含空格和换行。
        assert_eq!(out, r#"{"a":1,"b":[2,3]}"#);
    }

    #[test]
    fn compress_json_invalid_errors() {
        assert!(JsonFormatterTool::compress_json("not json").is_err());
    }

    #[test]
    fn escape_json_quotes() {
        let out = JsonFormatterTool::escape_json(r#"hello "world""#);
        assert_eq!(out, r#""hello \"world\"""#);
    }

    #[test]
    fn unescape_json_roundtrip() {
        let original = "hello\nworld\t\"test\"";
        let escaped = JsonFormatterTool::escape_json(original);
        let unescaped = JsonFormatterTool::unescape_json(&escaped).unwrap();
        assert_eq!(original, unescaped);
    }
}