use gpui_kit::prelude::*;
use gpui_kit::*;
use gpui_kit::component::{
    ActiveTheme, Sizable, StyledExt,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Textarea, TextareaState},
    label::Label,
    v_flex,
};

/// TSV → SQL 取值列表 工具页。
///
/// 支持固定前缀、外层括号开关和自动去重。
pub struct TsvTool {
    input_state: Entity<TextareaState>,
    output: String,
    /// 是否启用固定前缀。
    enable_prefix: bool,
    /// 固定前缀文本（默认 "IN "）。
    prefix: String,
    /// 是否添加外层括号。
    enable_parens: bool,
}

impl TsvTool {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("在此粘贴 TSV 数据（制表符分隔，自动取第一列）…")
        });
        Self {
            input_state,
            output: String::from("-- 点击「转换」生成取值列表"),
            enable_prefix: false,
            prefix: String::from("IN "),
            enable_parens: false,
        }
    }

    /// 纯函数：转换 TSV 为 SQL 取值列表。
    ///
    /// `dedup` 控制是否去重；`prefix` 为 Some 时在前面添加固定文本；
    /// `parens` 控制是否用括号包裹。
    fn convert(raw: &str, dedup: bool, prefix: Option<&str>, parens: bool) -> String {
        let mut values: Vec<String> = raw
            .lines()
            .map(|line| line.split(['\t', '|']).next().unwrap_or("").trim())
            .filter(|s| !s.is_empty())
            .map(|s| format!("'{}'", s.replace('\'', "''")))
            .collect();

        if values.is_empty() {
            return String::from("-- 未检测到有效数据");
        }

        // 去重（保留首次出现的顺序）。
        if dedup {
            let mut seen = std::collections::HashSet::new();
            values.retain(|v| seen.insert(v.clone()));
        }

        let mut result = values.join(", ");

        if parens {
            result = format!("({})", result);
        }
        if let Some(p) = prefix {
            result = format!("{}{}", p, result);
        }

        log::debug!(target: "tool.tsv", "TSV 转换：解析到 {} 个值（去重后 {} 个）", raw.lines().count(), values.len());
        result
    }
}

impl Render for TsvTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let input_state = self.input_state.clone();

        let theme = cx.theme();

        let convert_btn = Button::new("tsv-convert")
            .primary()
            .label("转换")
            .small()
            .on_click({
                let tool = tool.clone();
                let input = input_state.clone();
                let enable_prefix = self.enable_prefix;
                let prefix = self.prefix.clone();
                let enable_parens = self.enable_parens;
                move |_, _window, cx| {
                    let raw = input.read(cx).value().to_string();
                    let prefix = if enable_prefix { Some(prefix.as_str()) } else { None };
                    let out = Self::convert(&raw, true, prefix, enable_parens);
                    log::info!(target: "tool.tsv", "TSV 转换完成，结果 {} 字符", out.len());
                    tool.update(cx, |t, cx| {
                        t.output = out;
                        cx.notify();
                    });
                }
            });

        let output_copy = self.output.clone();
        let copy_btn = Button::new("tsv-copy")
            .label("复制结果")
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.tsv", "已复制 TSV 转换结果到剪贴板");
            });

        // 前缀开关按钮
        let toggle_prefix = if self.enable_prefix {
            Button::new("tsv-toggle-prefix")
                .primary()
                .label("前缀：开")
                .small()
                .on_click({
                    let tool = tool.clone();
                    move |_, _window, cx| {
                        tool.update(cx, |t, cx| {
                            t.enable_prefix = !t.enable_prefix;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        } else {
            Button::new("tsv-toggle-prefix")
                .label("前缀：关")
                .small()
                .on_click({
                    let tool = tool.clone();
                    move |_, _window, cx| {
                        tool.update(cx, |t, cx| {
                            t.enable_prefix = !t.enable_prefix;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        };

        // 括号开关按钮
        let toggle_parens = if self.enable_parens {
            Button::new("tsv-toggle-parens")
                .primary()
                .label("括号：开")
                .small()
                .on_click({
                    let tool = tool.clone();
                    move |_, _window, cx| {
                        tool.update(cx, |t, cx| {
                            t.enable_parens = !t.enable_parens;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        } else {
            Button::new("tsv-toggle-parens")
                .label("括号：关")
                .small()
                .on_click({
                    let tool = tool.clone();
                    move |_, _window, cx| {
                        tool.update(cx, |t, cx| {
                            t.enable_parens = !t.enable_parens;
                            cx.notify();
                        });
                    }
                })
                .into_any_element()
        };

        let output_box = div()
            .id("tsv-output")
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
            .child(self.output.clone());

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new("TSV → SQL IN").text_lg().font_semibold())
                    .child(
                        Label::new("粘贴 TSV 数据，取第一列生成逗号分隔的带引号值")
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
                    .child(toggle_prefix)
                    .child(toggle_parens)
                    .child(convert_btn)
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