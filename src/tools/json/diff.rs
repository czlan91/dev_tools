//! ## JSON 比较引擎
//!
//! 本模块实现两个 JSON 值之间的结构化差异比较。
//!
//! ### 核心思路
//!
//! 与传统的逐行文本 diff 不同，本模块采用**结构化对齐**策略：
//!
//! 1. **按对象 Key 对齐** —— 如果两侧都有相同的 Key，视为同一字段进行比较。
//!    键顺序不同时会被自动忽略（排序后比较）。
//! 2. **按数组下标对齐** —— 数组按索引位置一一对应比较。
//!    这意味着 `[1,2]` 和 `[2,1]` 会被标记为修改（而非：删除 1 再添加 2）。
//! 3. **值相等时标记为 Equal** —— 只有真正不同的行才用颜色标记。
//!
//! 这种策略的优点是：差异结果稳定、可预测，不因 JSON 键顺序变化而产生噪声。
//!
//! ### 为什么不做文本 diff？
//!
//! 传统的文本 diff（如 `diff` 命令）基于行匹配，会因 JSON 格式变化
//! （如缩进、键顺序）产生大量假阳性差异。结构化对齐避免了这个问题。
//!
//! 详细的比较算法实现在 `align` 函数中。

use serde_json::Value;

/// 行变化类型枚举。
///
/// 用于标记两个 JSON 中对应行的差异，每个值对应不同的颜色高亮。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChangeKind {
    /// 两侧内容一致（不显示高亮）
    Equal,
    /// 左侧没有该行，右侧有（新增，绿色高亮）
    Added,
    /// 左侧有该行，右侧没有（删除，红色高亮）
    Removed,
    /// 两侧都有该行但值不同（修改，黄色高亮）
    Modified,
}

/// 差异行数据。
///
/// 每一行同时包含左侧和右侧的文本表示，以及对应的变化类型。
/// 用于在编辑器中逐行渲染对齐后的比较结果。
pub struct DiffRow {
    /// 左侧（基准）的该行文本，空字符串表示该行在左侧不存在。
    pub left: String,
    /// 右侧（新值）的该行文本，空字符串表示该行在右侧不存在。
    pub right: String,
    /// 该行的变化类型。
    pub kind: ChangeKind,
}

/// 比较结果，包含对齐后的行数据和原始 JSON 文本。
pub struct Comparison {
    /// 对齐后的行列表，按行号一一对应。
    pub rows: Vec<DiffRow>,
    /// 原始左侧 JSON 的格式化文本（用于「展开原始 JSON」功能）。
    pub raw_left: String,
    /// 原始右侧 JSON 的格式化文本（用于「展开原始 JSON」功能）。
    pub raw_right: String,
}

impl Comparison {
    /// 创建两个 JSON 值的比较结果。
    ///
    /// 处理流程：
    /// 1. 保存原始格式化文本（供「展开原始 JSON」使用）。
    /// 2. 对两侧的 JSON 进行 Key 排序（消除键顺序差异）。
    /// 3. 递归对齐比较。
    pub fn new(left: Value, right: Value) -> Self {
        // 保存原始格式化的 JSON 文本，保留原始键顺序
        let raw_left = serde_json::to_string_pretty(&left).expect("JSON Value 可序列化");
        let raw_right = serde_json::to_string_pretty(&right).expect("JSON Value 可序列化");

        // 对两侧进行 Key 排序，然后递归对齐
        let left = super::utils::sort_keys(left);
        let right = super::utils::sort_keys(right);
        let mut rows = Vec::new();
        // 初始调用：深度 0，无 Key，两侧都不需要尾逗号
        align(Some(&left), Some(&right), 0, "", (false, false), &mut rows);

        Self {
            rows,
            raw_left,
            raw_right,
        }
    }

    /// 提取指定侧（左侧或右侧）的文本内容。
    ///
    /// 将 `rows` 中对应侧的行文本用换行符连接起来。
    /// 空白行（缺失的条目）在文本中表现为空行，确保两侧的行号对齐。
    ///
    /// 返回的字符串是合法的 JSON 文本，可以重新解析。
    pub fn text(&self, left: bool) -> String {
        self.rows
            .iter()
            .map(|row| {
                if left {
                    row.left.as_str()
                } else {
                    row.right.as_str()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// 生成标准 diff 格式文本（带 `+`/`-` 前缀）。
    ///
    /// 格式：
    /// - 相同行：` 内容`
    /// - 删除行：`-内容`
    /// - 新增行：`+内容`
    /// - 修改行：同时输出 `-旧值` 和 `+新值`
    ///
    /// 注意：统计信息（如「新增 3 行 · 删除 2 行」）不混入此结果，
    /// 由调用方在 UI 中单独显示。
    pub fn diff(&self) -> String {
        let mut out = String::new();
        for row in &self.rows {
            if row.kind == ChangeKind::Equal {
                out.push_str(&format!(" {}\n", row.left));
            } else {
                if !row.left.is_empty() {
                    out.push_str(&format!("-{}\n", row.left));
                }
                if !row.right.is_empty() {
                    out.push_str(&format!("+{}\n", row.right));
                }
            }
        }
        out
    }
}

/// 生成一行对齐文本。
///
/// `depth` 控制缩进层级，`key` 是对象字段名（如 `"name": `），
/// `text` 是值的文本表示，`comma` 表示是否在该行末尾添加逗号。
fn line(depth: usize, key: &str, text: &str, comma: bool) -> String {
    format!(
        "{}{key}{text}{}",
        "  ".repeat(depth),
        if comma { "," } else { "" }
    )
}

/// 将多行 JSON 值展开为文本行列表。
///
/// 保持 Key 前缀只在第一行输出，后续行留空（缩进仍然对齐）。
/// 例如 `{"a": 1, "b": 2}` 展开为：
/// ```text
/// "a": 1,
/// "b": 2
/// ```
fn block(value: &Value, depth: usize, key: &str, comma: bool) -> Vec<String> {
    let text = serde_json::to_string_pretty(value).expect("JSON Value 可序列化");
    let count = text.lines().count();
    text.lines()
        .enumerate()
        .map(|(ix, text)| {
            line(
                depth,
                // 只有第一行带 Key 前缀，后续行留空
                if ix == 0 { key } else { "" },
                text,
                // 最后一行才加逗号
                comma && ix + 1 == count,
            )
        })
        .collect()
}

/// 对齐任务：显式栈中的一个工作单元。
///
/// 将递归调用栈搬到堆上，避免深层嵌套 JSON 导致的栈溢出。
/// Rust 不保证尾调用优化（TCO），且 `align` 是树形递归（先序遍历 + 收尾工作），
/// 结构上无法改写为尾递归，因此使用显式栈 + 迭代。
enum AlignTask<'a> {
    /// 对齐一对 JSON 值（对应原递归函数的入口）。
    Visit {
        left: Option<&'a Value>,
        right: Option<&'a Value>,
        depth: usize,
        key: String,
        commas: (bool, bool),
    },
    /// 输出容器闭合行（`}` 或 `]`），对应原递归函数的收尾工作。
    Close {
        depth: usize,
        bracket: char,
        commas: (bool, bool),
    },
}

/// 递归对齐两个 JSON 值，生成差异行列表。
///
/// ### 对齐策略
///
/// - **对象（Object）**：收集两侧所有 Key 的并集，按字母顺序遍历。
///   对于每个 Key，递归比较两侧的对应值。
/// - **数组（Array）**：按索引位置一一对应比较，较长数组多出的成员
///   视为新增（右侧多出）或删除（左侧多出）。
/// - **其他值**：直接比较值是否相等，按类型标记为 Equal 或 Modified。
/// - **缺失值**：如果一侧没有某个 Key 或数组元素，对应侧显示空行。
///
/// ### 尾逗号处理
///
/// 两侧的尾逗号独立计算，确保每侧生成的文本都是合法的 JSON。
/// 例如左侧有 `a,b` 右侧只有 `a`，左侧的 `b` 行需要逗号，右侧不需要。
///
/// ### 为什么用显式栈而不是尾递归
///
/// Rust 不保证尾调用优化，且本函数在 `for` 循环中多次递归、递归后还要输出
/// 闭合括号（非尾位置），无法改写为尾递归。显式栈将调用帧搬到堆上，
/// 栈深度只受内存限制，不受调用栈大小限制。
fn align(
    left: Option<&Value>,
    right: Option<&Value>,
    depth: usize,
    key: &str,
    commas: (bool, bool),
    rows: &mut Vec<DiffRow>,
) {
    let mut stack = vec![AlignTask::Visit {
        left,
        right,
        depth,
        key: key.to_owned(),
        commas,
    }];

    // 栈是 LIFO：后入栈的任务先执行，与递归的「深入子节点再收尾」顺序一致
    while let Some(task) = stack.pop() {
        match task {
            AlignTask::Visit {
                left,
                right,
                depth,
                key,
                commas,
            } => match (left, right) {
                // —— 对象（Object）对齐 ——
                (Some(Value::Object(a)), Some(Value::Object(b)))
                    if !a.is_empty() || !b.is_empty() =>
                {
                    // 输出左花括号
                    rows.push(DiffRow {
                        left: line(depth, &key, "{", false),
                        right: line(depth, &key, "{", false),
                        kind: ChangeKind::Equal,
                    });

                    // 闭合任务先入栈（后执行），子任务逆序入栈（正序执行）
                    stack.push(AlignTask::Close {
                        depth,
                        bracket: '}',
                        commas,
                    });

                    // 收集两侧所有 Key 的并集，用 BTreeSet 自动排序
                    let keys: std::collections::BTreeSet<_> =
                        a.keys().chain(b.keys()).collect();
                    // 逆序入栈，弹出时即为正序
                    for key in keys.into_iter().rev() {
                        let label =
                            format!("{}: ", serde_json::to_string(key).expect("Key 可序列化"));
                        stack.push(AlignTask::Visit {
                            left: a.get(key),
                            right: b.get(key),
                            depth: depth + 1,
                            key: label,
                            commas: (
                                // 该 Key 在左侧不是最后一个时需要逗号
                                a.keys().next_back() != Some(key),
                                // 该 Key 在右侧不是最后一个时需要逗号
                                b.keys().next_back() != Some(key),
                            ),
                        });
                    }
                }

                // —— 数组（Array）对齐 ——
                (Some(Value::Array(a)), Some(Value::Array(b)))
                    if !a.is_empty() || !b.is_empty() =>
                {
                    rows.push(DiffRow {
                        left: line(depth, &key, "[", false),
                        right: line(depth, &key, "[", false),
                        kind: ChangeKind::Equal,
                    });

                    stack.push(AlignTask::Close {
                        depth,
                        bracket: ']',
                        commas,
                    });

                    // 按索引对齐，较长数组多出的部分视为新增/删除；逆序入栈
                    for ix in (0..a.len().max(b.len())).rev() {
                        stack.push(AlignTask::Visit {
                            left: a.get(ix),
                            right: b.get(ix),
                            depth: depth + 1,
                            key: String::new(),
                            commas: (ix + 1 < a.len(), ix + 1 < b.len()),
                        });
                    }
                }

                // —— 叶子值（非容器，或空容器）对齐 ——
                _ => {
                    let kind = match (left, right) {
                        (None, _) => ChangeKind::Added,                    // 左侧无 → 新增
                        (_, None) => ChangeKind::Removed,                  // 右侧无 → 删除
                        (Some(a), Some(b)) if a == b => ChangeKind::Equal, // 相等 → 不变
                        _ => ChangeKind::Modified,                         // 不等 → 修改
                    };

                    let a = left
                        .map(|v| block(v, depth, &key, commas.0))
                        .unwrap_or_default();
                    let b = right
                        .map(|v| block(v, depth, &key, commas.1))
                        .unwrap_or_default();

                    // 两侧行数可能不同（如 `"a": 1` 一行 vs 多行对象），对齐到最大行数
                    for ix in 0..a.len().max(b.len()) {
                        rows.push(DiffRow {
                            left: a.get(ix).cloned().unwrap_or_default(),
                            right: b.get(ix).cloned().unwrap_or_default(),
                            kind,
                        });
                    }
                }
            },

            // —— 容器收尾：输出闭合行 ——
            AlignTask::Close {
                depth,
                bracket,
                commas,
            } => {
                let text = bracket.to_string();
                rows.push(DiffRow {
                    left: line(depth, "", &text, commas.0),
                    right: line(depth, "", &text, commas.1),
                    kind: ChangeKind::Equal,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 辅助函数：从 JSON 字符串创建 Comparison。
    fn compare(a: &str, b: &str) -> Comparison {
        Comparison::new(
            serde_json::from_str(a).unwrap(),
            serde_json::from_str(b).unwrap(),
        )
    }

    /// 测试：缺失的 Key 产生空行，且每侧文本仍然是合法的 JSON。
    #[test]
    fn missing_keys_get_blank_lines_and_sides_remain_json() {
        let result = compare(r#"{"a":{"x":1},"c":2}"#, r#"{"b":true,"c":3}"#);
        // 应该有删除行（a 在右侧缺失）
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Removed && row.right.is_empty())
        );
        // 应该有新增行（b 在左侧缺失）
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Added && row.left.is_empty())
        );
        // 应该有修改行（c 的值不同）
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Modified
                    && row.left.contains("c")
                    && row.right.contains("c"))
        );
        // 两侧文本都必须是合法的 JSON
        for side in [true, false] {
            assert!(serde_json::from_str::<Value>(&result.text(side)).is_ok());
        }
    }

    /// 测试：对象键顺序不影响比较结果，但数组顺序影响。
    #[test]
    fn key_order_is_ignored_but_array_order_is_preserved() {
        // 键顺序不同，但值相同 → 所有行都标记为 Equal
        assert!(
            compare(r#"{"b":2,"a":1}"#, r#"{"a":1,"b":2}"#)
                .rows
                .iter()
                .all(|r| r.kind == ChangeKind::Equal)
        );
        // 数组元素顺序不同 → 标记为 Modified
        assert!(
            compare("[1,2]", "[2,1]")
                .rows
                .iter()
                .any(|r| r.kind == ChangeKind::Modified)
        );
    }

    /// 测试：空容器和类型变化产生合法的 JSON 输出。
    #[test]
    fn empty_containers_and_type_changes_remain_valid() {
        for (a, b) in [
            ("{}", r#"{"x":1}"#),
            ("[]", "[1,2]"),
            ("null", "{\"x\":[]}"),
        ] {
            let result = compare(a, b);
            for side in [true, false] {
                assert!(serde_json::from_str::<Value>(&result.text(side)).is_ok());
            }
        }
    }

    /// 测试：深层嵌套 JSON 不会栈溢出，且迭代版结果与预期一致。
    ///
    /// 迭代版使用显式栈（堆分配），不受调用栈深度限制。
    /// 注意：serde_json 默认解析深度限制为 128 层，这里测试 100 层嵌套。
    #[test]
    fn deep_nesting_does_not_stack_overflow() {
        // 构造 100 层嵌套对象：{"a":{"a":{"a":...}}}
        let depth = 100;
        let mut left = String::from("1");
        let mut right = String::from("2");
        for _ in 0..depth {
            left = format!(r#"{{"a":{left}}}"#);
            right = format!(r#"{{"a":{right}}}"#);
        }

        let result = compare(&left, &right);

        // 最内层的值不同 → 应该有 Modified 行
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Modified)
        );
        // 两侧文本都必须是合法的 JSON
        for side in [true, false] {
            assert!(serde_json::from_str::<Value>(&result.text(side)).is_ok());
        }
    }

    /// 测试：数组元素是对象时，按索引对齐后递归按 Key 比较。
    ///
    /// 左: [{"name":"a","v":1}, {"x":1}]
    /// 右: [{"name":"a","v":2}, {"y":2}]
    /// → [0] 内 v 不同 → Modified；[1] 内 x/y 不同 → Removed + Added
    #[test]
    fn array_of_objects_aligns_by_index_then_by_key() {
        let result = compare(
            r#"[{"name":"a","v":1},{"x":1}]"#,
            r#"[{"name":"a","v":2},{"y":2}]"#,
        );

        // [0] 中 v 的值不同 → Modified 行，且两侧都包含 "v"
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Modified
                    && row.left.contains("\"v\"")
                    && row.right.contains("\"v\""))
        );
        // [0] 中 name 相同 → Equal 行
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Equal && row.left.contains("name"))
        );
        // [1] 中 x 被删除、y 被新增
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Removed && row.left.contains("\"x\""))
        );
        assert!(
            result
                .rows
                .iter()
                .any(|row| row.kind == ChangeKind::Added && row.right.contains("\"y\""))
        );
        // 两侧文本都必须是合法的 JSON
        for side in [true, false] {
            assert!(serde_json::from_str::<Value>(&result.text(side)).is_ok());
        }
    }
}