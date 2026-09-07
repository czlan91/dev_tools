use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::{
    ActiveTheme, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    input::{Textarea, TextareaState},
    label::Label,
    h_flex, v_flex,
};
use similar::{ChangeTag, TextDiff};

/// JSON 比较工具页：比较两份 JSON 数据，自动排序 key，diff 展示差异。
///
/// 设计说明：
/// - 两个输入框分别对应「基准（左侧/旧值）」和「新值（右侧）」。
/// - 比较时自动对两侧 JSON 做 key 递归排序，确保 diff 结果不受 key 顺序影响。
/// - 原始 JSON 展示在可折叠区域中，默认收起，点击展开。
pub struct JsonCompareTool {
    /// 基准输入框（左侧/旧值）。
    left_input: Entity<TextareaState>,
    /// 新值输入框（右侧/新值）。
    right_input: Entity<TextareaState>,
    /// diff 结果文本。
    output: String,
    /// 错误信息（解析失败时）。
    error: Option<String>,
    /// 是否展开原始 JSON 区域。
    show_raw: bool,
    /// 左侧原始 JSON（格式化后，用于展示）。
    raw_left: String,
    /// 右侧原始 JSON（格式化后，用于展示）。
    raw_right: String,
}

impl JsonCompareTool {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            left_input: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("在此粘贴基准 JSON（左侧 / 旧值）…")
            }),
            right_input: cx.new(|cx| {
                TextareaState::new(window, cx)
                    .placeholder("在此粘贴新 JSON（右侧 / 新值）…")
            }),
            output: String::from("-- 点击「比较」生成 diff"),
            error: None,
            show_raw: false,
            raw_left: String::new(),
            raw_right: String::new(),
        }
    }

    /// 解析 JSON5（JSON 的超集），统一返回 `serde_json::Value`。
    fn parse5(text: &str) -> Result<serde_json::Value, String> {
        json5::from_str(text).map_err(|e| {
            let msg = e.to_string();
            if let Some(pos) = e.position() {
                format!("第 {} 行第 {} 列：{}", pos.line + 1, pos.column + 1, msg)
            } else {
                msg
            }
        })
    }

    /// 递归排序对象 key（字典序）。
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

    /// 格式化 + 排序 key 后输出标准 JSON。
    fn format_and_sort(text: &str) -> Result<String, String> {
        let value = Self::parse5(text)?;
        let sorted = Self::sort_keys(value);
        serde_json::to_string_pretty(&sorted).map_err(|e| e.to_string())
    }

    /// 纯函数：比较两份 JSON，自动排序 key，返回 git 风格 diff。
    fn compare(left: &str, right: &str) -> Result<(String, String, String), String> {
        let left = Self::format_and_sort(left)?;
        let right = Self::format_and_sort(right)?;

        if left == right {
            return Ok(("-- 两侧内容完全一致".to_string(), left, right));
        }

        let diff = TextDiff::from_lines(&left, &right);
        let mut out = String::new();
        for change in diff.iter_all_changes() {
            out.push_str(match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            });
            out.push_str(change.value().trim_end_matches('\n'));
            out.push('\n');
        }
        Ok((out, left, right))
    }
}

impl Render for JsonCompareTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let left = self.left_input.clone();
        let right = self.right_input.clone();

        let compare_btn = Button::new("json-compare-compare")
            .primary()
            .label("比较")
            .small()
            .on_click({
                let tool = tool.clone();
                let left = left.clone();
                let right = right.clone();
                move |_, _window, cx| {
                    let left_raw = left.read(cx).value().to_string();
                    let right_raw = right.read(cx).value().to_string();
                    match Self::compare(&left_raw, &right_raw) {
                        Ok((out, raw_left, raw_right)) => {
                            log::info!(
                                target: "tool.json",
                                "JSON 比较完成，diff 输出 {} 字符",
                                out.len()
                            );
                            tool.update(cx, |t, cx| {
                                t.output = out;
                                t.raw_left = raw_left;
                                t.raw_right = raw_right;
                                t.error = None;
                                t.show_raw = true;
                                cx.notify();
                            });
                        }
                        Err(e) => {
                            log::warn!(target: "tool.json", "JSON 比较失败: {e}");
                            tool.update(cx, |t, cx| {
                                t.error = Some(e);
                                cx.notify();
                            });
                        }
                    }
                }
            });

        let output_copy = self.output.clone();
        let copy_btn = Button::new("json-compare-copy")
            .label("复制结果")
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.json", "已复制 JSON 比较结果到剪贴板");
            });

        // 输出区：diff 行着色（'-' 红、'+' 绿）
        let theme = cx.theme();
        let lines: Vec<AnyElement> = self
            .output
            .lines()
            .map(|line| {
                let (bg, text) = if let Some(rest) = line.strip_prefix('-') {
                    (theme.danger.opacity(0.18), format!("-{rest}"))
                } else if let Some(rest) = line.strip_prefix('+') {
                    (theme.success.opacity(0.18), format!("+{rest}"))
                } else {
                    (gpui_kit::transparent_white(), line.to_string())
                };
                div()
                    .w_full()
                    .bg(bg)
                    .px_1()
                    .child(text)
                    .into_any_element()
            })
            .collect();

        let output_box = div()
            .id("json-compare-output")
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
                None => v_flex()
                    .w_full()
                    .gap_0()
                    .children(lines)
                    .into_any_element(),
            });

        // 原始 JSON 可折叠区域
        let has_raw = !self.raw_left.is_empty() && !self.raw_right.is_empty();
        let raw_section = has_raw.then(|| {
            let expanded = self.show_raw;
            let toggle_label = if expanded {
                "原始 JSON ▾"
            } else {
                "原始 JSON ▸"
            };

            let header = div()
                .id("json-compare-raw")
                .cursor_pointer()
                .on_click({
                    let tool = tool.clone();
                    move |_, _, cx| {
                        tool.update(cx, |t, cx| {
                            t.show_raw = !t.show_raw;
                            cx.notify();
                        });
                    }
                })
                .child(
                    h_flex()
                        .gap_1()
                        .items_center()
                        .child(
                            Label::new(toggle_label)
                                .text_sm()
                                .text_color(theme.muted_foreground),
                        ),
                );

            let content = expanded.then(|| {
                v_flex()
                    .gap_2()
                    .mt_2()
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Label::new("左侧（基准）")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .id("json-compare-raw-left")
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.muted)
                                    .p_2()
                                    .overflow_scroll()
                                    .max_h(px(200.))
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .child(self.raw_left.clone()),
                            ),
                    )
                    .child(
                        v_flex()
                            .gap_1()
                            .child(
                                Label::new("右侧（新值）")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .id("json-compare-raw-right")
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(theme.border)
                                    .bg(theme.muted)
                                    .p_2()
                                    .overflow_scroll()
                                    .max_h(px(200.))
                                    .font_family("JetBrains Mono")
                                    .text_xs()
                                    .child(self.raw_right.clone()),
                            ),
                    )
            });

            v_flex()
                .gap_0()
                .child(header)
                .when_some(content, |this, c| this.child(c))
        });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new("JSON 比较").text_lg().font_semibold())
                    .child(
                        Label::new("比较两份 JSON 数据，自动排序 key，git 风格 diff 展示差异")
                            .text_sm()
                            .text_color(theme.muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .gap_2()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_1()
                            .child(
                                Label::new("基准（左侧 / 旧值）")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(Textarea::new(&self.left_input).h_full()),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_h_0()
                            .gap_1()
                            .child(
                                Label::new("新值（右侧 / 新值）")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_h_0()
                                    .child(Textarea::new(&self.right_input).h_full()),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(compare_btn)
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
            .when_some(raw_section, |this, section| this.child(section))
    }
}