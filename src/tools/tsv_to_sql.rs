//! ## TSV → SQL IN 工具
//!
//! 本工具将 TSV（制表符分隔值）数据转换为 SQL 的 `IN` 条件列表。
//!
//! ### 使用场景
//!
//! 当你从数据库查询、Excel 导出或日志中拿到一列数据，需要用它构造 SQL 查询条件时，
//! 手动加引号、加逗号、去重非常繁琐。本工具一键完成：
//!
//! **输入（TSV）**：
//! ```text
//! 1	Alice
//! 2	Bob
//! 3	Charlie
//! ```
//!
//! **输出（`IN` 列表，列 1，去重，加括号）**：
//! ```sql
//! ('Alice', 'Bob', 'Charlie')
//! ```
//!
//! ### 功能特性
//! - 固定前缀（如 `IN `、`NOT IN `）
//! - 外层括号开关
//! - 列选择（从 0 开始，UI 显示从 1 开始）
//! - 自定义分隔符（未填写时默认 Tab）
//! - 自动去重（保留首次出现顺序）
//! - NULL 值不加引号（`NULL` → `NULL`）

use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, StyledExt,
    alert::Alert,
    button::Button,
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputState, Textarea, TextareaState},
    label::Label,
    v_flex,
};
use gpui_kit::prelude::*;
use gpui_kit::*;

/// TSV → SQL 取值列表工具页。
///
/// 支持固定前缀、外层括号开关、列选择、自定义分隔符和自动去重。
/// 输入使用多行文本区（Textarea），输出显示在只读框中。
pub struct TsvTool {
    /// TSV 输入文本区状态。
    input_state: Entity<TextareaState>,
    /// 转换后的 SQL 输出文本。
    output: String,
    /// 错误信息（`None` 表示无错误）。
    error: Option<String>,
    /// 是否启用固定前缀。
    enable_prefix: bool,
    /// 固定前缀文本输入状态。
    prefix_state: Entity<InputState>,
    /// 是否添加外层括号。
    enable_parens: bool,
    /// 要转换的列索引（从 0 开始，UI 显示从 1 开始）。
    column_state: Entity<InputState>,
    /// 自定义分隔符输入状态（空字符串表示使用默认 Tab）。
    delimiter_state: Entity<InputState>,
}

impl TsvTool {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state = cx.new(|cx| {
            TextareaState::new(window, cx).placeholder("在此粘贴数据（默认制表符分隔）…")
        });
        let prefix_state = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("IN ")
                .placeholder("固定前缀…")
        });
        let column_state = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("1")
                .placeholder("列号")
        });
        let delimiter_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("分隔符，默认 Tab")
        });
        Self {
            input_state,
            output: String::new(),
            error: None,
            enable_prefix: false,
            prefix_state,
            enable_parens: false,
            column_state,
            delimiter_state,
        }
    }

    /// 纯函数：转换 TSV 为 SQL 取值列表。
    ///
    /// ### 参数
    ///
    /// - `raw`：原始输入文本（每行由分隔符分隔的字段组成）。
    /// - `column_index`：要提取的列索引（从 0 开始）。
    /// - `dedup`：是否去重（保留首次出现的顺序）。
    /// - `prefix`：可选的前缀文本（如 `"IN "`、`"NOT IN "`）。
    /// - `parens`：是否用括号包裹整个列表。
    /// - `delimiter`：字段分隔符，空字符串时自动使用 Tab（`\t`）。
    ///
    /// ### 处理规则
    ///
    /// 1. 按行分割，每行按分隔符分割为字段（分隔符为空时回退到 Tab）。
    /// 2. 提取指定列的值，去除首尾空白。
    /// 3. 空值和空行跳过。
    /// 4. `NULL`（不区分大小写）保持原样不加引号。
    /// 5. 其他值用单引号包裹，内部的单引号转义为 `''`（SQL 标准转义）。
    /// 6. 所有值用 `, ` 连接。
    fn convert(
        raw: &str,
        column_index: usize,
        dedup: bool,
        prefix: Option<&str>,
        parens: bool,
        delimiter: &str,
    ) -> String {
        // 分隔符为空时默认使用 Tab
        let delim: &str = if delimiter.is_empty() { "\t" } else { delimiter };
        let mut values: Vec<String> = raw
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split(delim).collect();
                parts.get(column_index).map(|s| s.trim())
            })
            .filter(|s| !s.is_empty())
            .map(|s| {
                // SQL 中 NULL 值不加引号
                if s.to_uppercase() == "NULL" {
                    "NULL".to_string()
                } else {
                    // SQL 单引号转义：`'` → `''`
                    format!("'{}'", s.replace('\'', "''"))
                }
            })
            .collect();

        if values.is_empty() {
            return String::new();
        }

        // 去重（保留首次出现的顺序）
        if dedup {
            let mut seen = std::collections::HashSet::new();
            values.retain(|v| seen.insert(v.clone()));
        }

        let mut result = values.join(", ");

        // 添加外层括号
        if parens {
            result = format!("({})", result);
        }
        // 添加固定前缀
        if let Some(p) = prefix {
            result = format!("{}{}", p, result);
        }

        let line_count = raw.lines().count();
        log::debug!(target: "tool.tsv", "TSV 转换：解析到 {} 行（列 {}，去重后 {} 个值）", line_count, column_index + 1, values.len());
        result
    }
}

impl Render for TsvTool {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let tool = cx.entity();
        let input_state = self.input_state.clone();
        let prefix_state = self.prefix_state.clone();
        let column_state = self.column_state.clone();
        let delimiter_state = self.delimiter_state.clone();

        let theme = cx.theme();

        // —— 转换按钮 ——
        let convert_btn = Button::new("tsv-convert").label("转换").small().on_click({
            let tool = tool.clone();
            let input = input_state.clone();
            let prefix_input = prefix_state.clone();
            let column_input = column_state.clone();
            let delimiter_input = delimiter_state.clone();
            let enable_prefix = self.enable_prefix;
            let enable_parens = self.enable_parens;
            move |_, _window, cx| {
                let raw = input.read(cx).value().to_string();
                let prefix = if enable_prefix {
                    let p = prefix_input.read(cx).value().to_string();
                    if p.is_empty() { None } else { Some(p) }
                } else {
                    None
                };
                let delimiter = delimiter_input.read(cx).value().to_string();
                let col_str = column_input.read(cx).value().to_string();
                // 列号从 1 开始解析（用户友好）
                let Ok(column) = col_str.trim().parse::<usize>() else {
                    tool.update(cx, |t, cx| {
                        t.error = Some("列号必须是从 1 开始的整数".into());
                        cx.notify();
                    });
                    return;
                };
                if column == 0 {
                    tool.update(cx, |t, cx| {
                        t.error = Some("列号必须大于 0".into());
                        cx.notify();
                    });
                    return;
                }
                // 内部从 0 开始
                let column_index = column - 1;
                let out = Self::convert(&raw, column_index, true, prefix.as_deref(), enable_parens, &delimiter);
                log::info!(target: "tool.tsv", "TSV 转换完成，结果 {} 字符", out.len());
                tool.update(cx, |t, cx| {
                    t.error = out
                        .is_empty()
                        .then(|| "所选列没有有效数据，请检查列号和输入内容。".into());
                    t.output = out;
                    cx.notify();
                });
            }
        });

        // —— 复制结果按钮 ——
        let output_copy = self.output.clone();
        let copy_btn = Button::new("tsv-copy")
            .label("复制结果")
            .disabled(self.output.is_empty() || self.error.is_some())
            .small()
            .on_click(move |_, _window, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(output_copy.clone()));
                log::info!(target: "tool.tsv", "已复制 TSV 转换结果到剪贴板");
            });

        // —— 固定前缀复选框 ——
        let toggle_prefix = Checkbox::new("tsv-toggle-prefix")
            .label("固定前缀")
            .checked(self.enable_prefix)
            .on_click(cx.listener(|tool, checked, _, cx| {
                tool.enable_prefix = *checked;
                cx.notify();
            }));

        // —— 外层括号复选框 ——
        let toggle_parens = Checkbox::new("tsv-toggle-parens")
            .label("添加外层括号")
            .checked(self.enable_parens)
            .on_click(cx.listener(|tool, checked, _, cx| {
                tool.enable_parens = *checked;
                cx.notify();
            }));

        // —— 输出框 ——
        let output_box = div()
            .id("tsv-output")
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
            .child(self.output.clone())
            .when_some(self.error.clone(), |view, error| {
                view.child(Alert::error("tsv-error", error))
            });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(
                v_flex()
                    .gap_0p5()
                    .child(Label::new("TSV → SQL IN").text_lg().font_semibold())
                    .child(
                        Label::new("粘贴数据，选择列和分隔符，生成逗号分隔的带引号值")
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
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(toggle_prefix)
                    .when(self.enable_prefix, |this| {
                        this.child(div().w_40().child(Input::new(&self.prefix_state)))
                    })
                    .child(toggle_parens)
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Label::new("分隔符")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(div().w_16().child(Input::new(&self.delimiter_state))),
                    )
                    .child(
                        h_flex()
                            .gap_1()
                            .items_center()
                            .child(
                                Label::new("列")
                                    .text_sm()
                                    .text_color(theme.muted_foreground),
                            )
                            .child(div().w_16().child(Input::new(&self.column_state))),
                    )
                    .child(convert_btn)
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
    use super::TsvTool;

    /// 测试：前缀和括号的独立组合。
    #[test]
    fn independent_prefix_and_parentheses() {
        for (prefix, parens, expected) in [
            (None, false, "'1', '2'"),
            (Some("IN "), false, "IN '1', '2'"),
            (None, true, "('1', '2')"),
            (Some("NOT IN "), true, "NOT IN ('1', '2')"),
        ] {
            assert_eq!(
                TsvTool::convert("1\ta\n2\tb", 0, true, prefix, parens, "\t"),
                expected
            );
        }
    }

    /// 测试：去重、NULL 处理、SQL 转义和空行。
    #[test]
    fn tab_only_null_dedup_and_escaping() {
        // 去重测试：两行相同的 a|b，去重后只保留一个
        assert_eq!(
            TsvTool::convert("a|b\nO'Reilly\nNULL\nnull\n\na|b", 0, true, None, false, "\t"),
            "'a|b', 'O''Reilly', NULL"
        );
        // 列选择测试：选择第 2 列
        assert_eq!(
            TsvTool::convert("1\tAlice\n2\tBob", 1, true, None, false, "\t"),
            "'Alice', 'Bob'"
        );
        // 空结果测试：列索引超出范围
        assert!(TsvTool::convert("1", 1, true, None, false, "\t").is_empty());
        // 空分隔符应回退到 Tab
        assert_eq!(
            TsvTool::convert("1\tAlice\n2\tBob", 1, true, None, false, ""),
            "'Alice', 'Bob'"
        );
    }

    /// 测试：自定义分隔符（逗号、竖线、空格）。
    #[test]
    fn custom_delimiter_works() {
        // 逗号分隔
        assert_eq!(
            TsvTool::convert("1,Alice\n2,Bob\n3,Charlie", 1, true, None, false, ","),
            "'Alice', 'Bob', 'Charlie'"
        );
        // 竖线分隔
        assert_eq!(
            TsvTool::convert("1|Alice\n2|Bob", 1, true, None, false, "|"),
            "'Alice', 'Bob'"
        );
        // 空格分隔（注意空格作为分隔符）
        assert_eq!(
            TsvTool::convert("1 Alice\n2 Bob", 1, true, None, false, " "),
            "'Alice', 'Bob'"
        );
        // 自定义分隔符 + 前缀 + 括号
        assert_eq!(
            TsvTool::convert("1,Alice\n2,Bob", 1, true, Some("IN "), true, ","),
            "IN ('Alice', 'Bob')"
        );
    }
}