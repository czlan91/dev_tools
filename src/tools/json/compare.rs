//! ## JSON 比较工具
//!
//! 本工具并排显示两个 JSON 编辑器，比较时按 Key 对齐并高亮差异。
//!
//! ### 交互流程
//!
//! 1. 用户在左侧编辑器中粘贴「基准 JSON」，右侧粘贴「新 JSON」。
//! 2. 点击「比较」按钮，引擎按 Key 对齐并标记差异行。
//! 3. 比较后编辑器变为只读，差异行用颜色高亮：
//!    - 绿色：新增行（右侧有，左侧无）
//!    - 红色：删除行（左侧有，右侧无）
//!    - 黄色：修改行（两侧值不同）
//! 4. 点击「返回编辑」可恢复原始输入继续修改。
//! 5. 点击「复制 diff」可复制带 `+`/`-` 前缀的标准 diff 文本。
//! 6. 点击「展开原始 JSON」可查看原始格式化的 JSON 文本。
//!
//! 比较结果直接显示于原输入编辑器中；返回编辑时恢复未对齐的输入。
use super::{
    diff::{ChangeKind, Comparison},
    utils,
};
use gpui_kit::component::{
    ActiveTheme, Disableable, StyledExt,
    alert::Alert,
    button::Button,
    h_flex,
    input::{Editor, EditorState, TextDecoration, TextDecorationCollection},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

/// JSON 比较工具实体。
///
/// 同时持有左右两个编辑器的状态，以及比较结果和装饰标记。
/// 当比较完成后，编辑器显示对齐后的文本，差异行通过 `TextDecoration` 高亮。
pub struct JsonCompareTool {
    /// 左侧（基准）编辑器状态。
    left_input: Entity<EditorState>,
    /// 右侧（新值）编辑器状态。
    right_input: Entity<EditorState>,
    /// 左侧编辑器的文本装饰集合（用于高亮差异行）。
    left_marks: TextDecorationCollection,
    /// 右侧编辑器的文本装饰集合。
    right_marks: TextDecorationCollection,
    /// 当前的比较结果（`None` 表示未比较或已返回编辑模式）。
    comparison: Option<Comparison>,
    /// 左侧解析错误信息。
    left_error: Option<String>,
    /// 右侧解析错误信息。
    right_error: Option<String>,
    /// 是否展开显示原始 JSON。
    show_raw: bool,
    /// 原始左侧输入，比较时保存，返回编辑时恢复。
    ///
    /// 比较时编辑器会显示对齐后的文本，为了让用户能继续编辑原始输入，
    /// 需要先保存原始内容，返回编辑时恢复。
    original_left: String,
    /// 原始右侧输入。
    original_right: String,
    /// 需要保持存活的订阅集合。
    _subscriptions: Vec<Subscription>,
    /// 滚动同步重入标志：防止同步滚动时触发另一次同步，导致死循环。
    ///
    /// `set_scroll_offset` 内部使用 `deferred_scroll_offset`，在下一帧 prepaint 时才生效。
    /// 而 `scroll_offset()` 读取的是当前实际值（deferred 应用前的旧值）。
    /// 如果直接比较 `scroll_offset()`，deferred 值未生效前会读到旧值，导致来回设置形成死循环。
    /// 因此用 `syncing_scroll` 标志阻止重入，并在 effect 循环结束后自动重置。
    syncing_scroll: std::cell::Cell<bool>,
}

impl JsonCompareTool {
    /// 创建 JSON 比较工具实例。
    ///
    /// 初始化两个编辑器，创建装饰集合，并设置滚动同步和主题变化订阅。
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let left_input = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .folding(false)     // 比较模式不需要折叠
                .soft_wrap(false)   // 对齐后不应换行
                .placeholder("粘贴基准 JSON…")
        });
        let right_input = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("json")
                .folding(false)
                .soft_wrap(false)
                .placeholder("粘贴新 JSON…")
        });

        // 创建装饰集合，用于高亮差异行
        let left_marks = left_input.update(cx, |editor, cx| {
            editor.create_decorations_collection(Vec::new(), cx)
        });
        let right_marks = right_input.update(cx, |editor, cx| {
            editor.create_decorations_collection(Vec::new(), cx)
        });

        // 两侧统一行高，比较时联动垂直滚动
        // 同步时使用比较偏移避免通知循环
        let left_scroll = cx.observe(&left_input, |tool, _, cx| tool.sync_scroll(true, cx));
        let right_scroll = cx.observe(&right_input, |tool, _, cx| tool.sync_scroll(false, cx));

        // 主题变化时重新计算装饰颜色
        let theme = cx.observe_global::<gpui_kit::component::Theme>(|tool, cx| tool.decorate(cx));

        // 观察编辑器光标变化，通知工具实体重新渲染以更新状态栏
        let observe_left = cx.observe(&left_input, |_, _, cx| cx.notify());
        let observe_right = cx.observe(&right_input, |_, _, cx| cx.notify());

        Self {
            left_input,
            right_input,
            left_marks,
            right_marks,
            comparison: None,
            left_error: None,
            right_error: None,
            show_raw: false,
            original_left: String::new(),
            original_right: String::new(),
            _subscriptions: vec![left_scroll, right_scroll, theme, observe_left, observe_right],
            syncing_scroll: std::cell::Cell::new(false),
        }
    }

    /// 同步两侧编辑器的垂直滚动位置。
    ///
    /// 当用户滚动一侧编辑器时，另一侧跟随滚动，确保对齐的行始终在同一垂直位置。
    ///
    /// 注意：`set_scroll_offset` 内部使用 `deferred_scroll_offset`，在下一帧 prepaint 时才生效。
    /// 而 `scroll_offset()` 读取的是当前实际值（deferred 应用前的旧值）。
    /// 如果直接比较 `scroll_offset()`，deferred 值未生效前会读到旧值，导致来回设置形成死循环。
    ///
    /// 解决方案：用 `syncing_scroll` 标志阻止重入。同步时设置标志，在 effect 循环结束后
    /// 通过 `cx.defer` 自动重置，这样 deferred 生效后的 observe 回调不会再次触发同步。
    fn sync_scroll(&self, from_left: bool, cx: &mut Context<Self>) {
        // 没有比较结果时不需要同步（未对齐）
        if self.comparison.is_none() {
            return;
        }
        // 重入检查：如果正在同步滚动，直接返回，避免死循环
        if self.syncing_scroll.get() {
            return;
        }
        self.syncing_scroll.set(true);

        let (source, target) = if from_left {
            (&self.left_input, &self.right_input)
        } else {
            (&self.right_input, &self.left_input)
        };
        let y = source.read(cx).scroll_offset().y;
        let mut offset = target.read(cx).scroll_offset();
        // 值相同则无需同步，避免 deferred 生效后的 observe 回调触发无限循环
        // 但需要重置标志，否则后续用户滚动会被跳过
        if (offset.y - y).abs() < gpui::px(0.01) {
            self.syncing_scroll.set(false);
            return;
        }
        offset.y = y;
        target.update(cx, |editor, cx| editor.set_scroll_offset(offset, cx));

        // 在 effect 循环结束后重置标志，允许下一次用户滚动触发同步
        let weak = cx.weak_entity();
        cx.defer(move |cx| {
            if let Some(this) = weak.upgrade() {
                this.update(cx, |this, _| this.syncing_scroll.set(false));
            }
        });
    }

    /// 比较两侧 JSON 内容。
    ///
    /// 流程：
    /// 1. 读取两侧编辑器内容。
    /// 2. 分别解析为 JSON Value。
    /// 3. 如果解析成功，创建 Comparison 并更新编辑器显示对齐文本。
    /// 4. 如果解析失败，在对应侧显示错误信息，不修改编辑器内容。
    fn compare(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let left = self.left_input.read(cx).value().to_string();
        let right = self.right_input.read(cx).value().to_string();
        let a = utils::parse(&left, false);
        let b = utils::parse(&right, false);

        // 两侧的解析错误分别记录，互不遮盖
        self.left_error = a.as_ref().err().map(ToString::to_string);
        self.right_error = b.as_ref().err().map(ToString::to_string);

        if let (Ok(a), Ok(b)) = (a, b) {
            let result = Comparison::new(a, b);
            // 保存原始输入，供返回编辑时恢复
            self.original_left = left;
            self.original_right = right;
            // 更新编辑器显示对齐后的文本
            self.left_input.update(cx, |editor, cx| {
                editor.set_value(result.text(true), window, cx)
            });
            self.right_input.update(cx, |editor, cx| {
                editor.set_value(result.text(false), window, cx)
            });
            log::info!(target: "tool.json", "JSON 比较完成，对齐 {} 行", result.rows.len());
            self.comparison = Some(result);
            self.show_raw = false;
            self.decorate(cx);
        } else {
            log::warn!(target: "tool.json", "JSON 比较解析失败：左侧={}，右侧={}",
                self.left_error.is_some(), self.right_error.is_some());
        }
        cx.notify();
    }

    /// 返回编辑模式：恢复原始输入，清空比较结果和装饰。
    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.comparison = None;
        self.left_input.update(cx, |editor, cx| {
            editor.set_value(self.original_left.clone(), window, cx)
        });
        self.right_input.update(cx, |editor, cx| {
            editor.set_value(self.original_right.clone(), window, cx)
        });
        self.left_marks.clear(cx);
        self.right_marks.clear(cx);
        self.show_raw = false;
        cx.notify();
    }

    /// 根据比较结果在编辑器中添加行高亮装饰。
    ///
    /// 为每个非 `Equal` 的行添加背景色高亮：
    /// - 新增（Added）：绿色背景
    /// - 删除（Removed）：红色背景
    /// - 修改（Modified）：黄色背景
    fn decorate(&self, cx: &mut Context<Self>) {
        let Some(result) = &self.comparison else {
            return;
        };
        for (left, collection) in [(true, &self.left_marks), (false, &self.right_marks)] {
            let mut offset = 0;
            let marks = result
                .rows
                .iter()
                .filter_map(|row| {
                    let text = if left { &row.left } else { &row.right };
                    let range = offset..offset + text.len();
                    // 每行末尾的换行符也计入偏移
                    offset += text.len() + 1;
                    let color = match row.kind {
                        ChangeKind::Equal => return None,                     // 一致行不标记
                        ChangeKind::Added => cx.theme().success,              // 新增 → 绿色
                        ChangeKind::Removed => cx.theme().danger,             // 删除 → 红色
                        ChangeKind::Modified => cx.theme().warning,           // 修改 → 黄色
                    };
                    // 创建半透明背景色装饰
                    Some(TextDecoration::new(
                        range,
                        HighlightStyle {
                            background_color: Some(color.opacity(0.2)),
                            ..Default::default()
                        },
                    ))
                })
                .collect();
            collection.set(marks, cx);
        }
    }

    /// 返回左侧编辑器光标位置（行号、列号，从 1 开始）。
    pub fn cursor_position(&self, cx: &App) -> Option<(u32, u32)> {
        let pos = self.left_input.read(cx).cursor_position();
        Some((pos.line + 1, pos.character + 1))
    }
}

impl Render for JsonCompareTool {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compared = self.comparison.is_some();
        let output = self.comparison.as_ref().map(Comparison::diff);

        // 统计差异行数，用于在界面上显示摘要
        let summary = self.comparison.as_ref().map(|result| {
            let count = |kind| result.rows.iter().filter(|row| row.kind == kind).count();
            let changed =
                count(ChangeKind::Added) + count(ChangeKind::Removed) + count(ChangeKind::Modified);
            if changed == 0 {
                "两侧内容一致".to_owned()
            } else {
                format!(
                    "新增 {} 行 · 删除 {} 行 · 修改 {} 行",
                    count(ChangeKind::Added),
                    count(ChangeKind::Removed),
                    count(ChangeKind::Modified)
                )
            }
        });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .child(div().text_lg().font_semibold().child("JSON 比较"))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .child(
                        Button::new("compare")
                            // 比较完成后按钮变为「返回编辑」
                            .label(if compared { "返回编辑" } else { "比较" })
                            .on_click(cx.listener(move |tool, _, window, cx| {
                                if compared {
                                    tool.edit(window, cx);
                                } else {
                                    tool.compare(window, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new("copy-diff")
                            .label("复制 diff")
                            .disabled(!compared)
                            .on_click(move |_, _, cx| {
                                if let Some(output) = &output {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        output.clone(),
                                    ));
                                }
                            }),
                    )
                    .when_some(summary, |row, text| row.child(div().text_sm().child(text))),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("按 Key 对齐；空白表示缺失。绿色：新增 · 红色：删除 · 黄色：修改"),
            )
            .child(
                // 左右并排编辑器
                h_flex()
                    .items_stretch()
                    .flex_1()
                    .min_h_0()
                    .gap_3()
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .gap_2()
                            .child("基准（旧值）")
                            .when_some(self.left_error.clone(), |pane, error| {
                                pane.child(Alert::error("left-error", error))
                            })
                            .child(
                                div().flex_1().min_h_0().child(
                                    Editor::new(&self.left_input)
                                        .h_full()
                                        .readonly(compared)  // 比较后只读
                                        .aria_label("基准 JSON"),
                                ),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .gap_2()
                            .child("新值")
                            .when_some(self.right_error.clone(), |pane, error| {
                                pane.child(Alert::error("right-error", error))
                            })
                            .child(
                                div().flex_1().min_h_0().child(
                                    Editor::new(&self.right_input)
                                        .h_full()
                                        .readonly(compared)
                                        .aria_label("新 JSON"),
                                ),
                            ),
                    ),
            )
            // 比较完成后显示「展开原始 JSON」按钮
            .when(compared, |view| {
                view.child(
                    Button::new("toggle-raw")
                        .label(if self.show_raw {
                            "收起原始 JSON"
                        } else {
                            "展开原始 JSON"
                        })
                        .on_click(cx.listener(|tool, _, _, cx| {
                            tool.show_raw = !tool.show_raw;
                            cx.notify();
                        })),
                )
            })
            // 展开原始格式化 JSON 文本
            .when(self.show_raw, |view| {
                let result = self
                    .comparison
                    .as_ref()
                    .expect("展开原始内容时存在比较快照");
                view.child(
                    h_flex()
                        .items_stretch()
                        .gap_3()
                        .h_40()
                        .child(
                            div()
                                .id("raw-left")
                                .flex_1()
                                .min_w_0()
                                .overflow_scroll()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_sm()
                                .child(result.raw_left.clone()),
                        )
                        .child(
                            div()
                                .id("raw-right")
                                .flex_1()
                                .min_w_0()
                                .overflow_scroll()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_sm()
                                .child(result.raw_right.clone()),
                        ),
                )
            })
    }
}