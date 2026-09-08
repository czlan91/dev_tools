//! ## JSON 公共工具函数
//!
//! 本模块提供 JSON 解析、Key 排序、转义/取消转义等工具函数，
//! 被 `json_formatter` 和 `json_compare` 两个工具共享。
//!
//! ### 错误处理
//!
//! 本模块使用 `thiserror` 定义了统一的 `JsonError` 枚举。
//! 错误信息中包含行号、列号和字符位置，方便用户在界面上定位问题。
//! 日志中不会输出解析器附带的用户原文，避免泄露敏感数据。
//!
//! ### 关于 JSON5
//!
//! JSON5 是 JSON 的超集，扩展了以下语法：
//! - 允许单引号字符串（`'...'`）
//! - 允许注释（`//` 和 `/* */`）
//! - 允许尾逗号（`{a: 1,}`）
//! - 允许无引号 Key（`{a: 1}`）
//! - 允许十六进制数（`0xFF`）
//!
//! 严格 JSON 解析失败时，可以尝试 JSON5 解析。

use serde_json::Value;

/// JSON 处理错误，包含语义化的错误消息和字符位置信息。
///
/// 每个变体都包含 `line`、`column` 和 `character` 三个位置信息，
/// 其中 `character` 是从文件开头到错误位置的字符计数（而非字节计数），
/// 适合在 UI 中直接显示给用户。
#[derive(Debug, thiserror::Error)]
pub enum JsonError {
    /// 严格 JSON 解析错误。
    #[error(
        "JSON 语法错误：第 {line} 行，第 {column} 列（字符位置 {character}）。请检查该位置附近的内容。"
    )]
    Json {
        /// 错误所在行号（从 1 开始）。
        line: usize,
        /// 错误所在列号（从 1 开始，基于字符计数）。
        column: usize,
        /// 错误所在字符位置（从文件开头计数，从 1 开始）。
        character: usize,
        /// 底层的 `serde_json` 解析错误。
        #[source]
        source: serde_json::Error,
    },
    /// JSON5 解析错误。
    #[error(
        "JSON5 语法错误：第 {line} 行，第 {column} 列（字符位置 {character}）。请检查该位置附近的内容。"
    )]
    Json5 {
        line: usize,
        column: usize,
        character: usize,
        #[source]
        source: json5::Error,
    },
    /// JSON 序列化错误（如格式化输出时失败）。
    #[error("无法序列化 JSON")]
    Serialize(#[from] serde_json::Error),
    /// 取消转义时，输入不是合法的 JSON 字符串字面量。
    #[error("取消转义需要一个合法的 JSON 字符串字面量")]
    NotString,
}

/// 将 byte 偏移量转换为字符位置。
///
/// `serde_json` 的 `column()` 方法返回的是 UTF-8 字节偏移量，
/// 而中文字符每个占 3 个字节，直接显示字节数对用户不友好。
/// 本函数将字节偏移量转换为直观的「第几列」和「字符位置」。
///
/// ### 参数
///
/// - `text`：原始输入文本。
/// - `line`：行号（从 1 开始）。
/// - `byte_column`：`serde_json` 报告的字节列号（从 1 开始）。
///
/// ### 返回
///
/// `(user_column, character_position)` —— 用户友好的列号和字符位置。
fn location(text: &str, line: usize, byte_column: usize) -> (usize, usize) {
    // 计算到错误行之前的字符总数
    let prefix: usize = text
        .split_inclusive('\n')
        .take(line.saturating_sub(1))
        .map(str::chars)
        .map(Iterator::count)
        .sum();
    // 获取错误行
    let row = text.lines().nth(line.saturating_sub(1)).unwrap_or("");
    // 将字节列号转换为字符列号
    let byte = byte_column.saturating_sub(1).min(row.len());
    let column = row.char_indices().take_while(|(ix, _)| *ix < byte).count() + 1;
    (column, prefix + column)
}

/// 解析 JSON 文本。
///
/// 根据 `json5` 参数选择解析器：
/// - `json5 = false`：使用 `serde_json` 严格解析（不接受注释、尾逗号等）。
/// - `json5 = true`：使用 `json5` crate 宽松解析。
///
/// 失败时返回 `JsonError`，包含精确的字符位置信息。
pub fn parse(text: &str, json5: bool) -> Result<Value, JsonError> {
    if json5 {
        json5::from_str(text).map_err(|source| {
            // json5 crate 的 Error 提供了 `position()` 方法返回行号/列号
            let (line, column) = source
                .position()
                .map(|p| (p.line + 1, p.column + 1))
                .unwrap_or((1, 1));
            // 计算字符位置
            let character = text
                .split_inclusive('\n')
                .take(line - 1)
                .map(str::chars)
                .map(Iterator::count)
                .sum::<usize>()
                + column;
            JsonError::Json5 {
                line,
                column,
                character,
                source,
            }
        })
    } else {
        serde_json::from_str(text).map_err(|source| {
            let line = source.line().max(1);
            let (column, character) = location(text, line, source.column());
            JsonError::Json {
                line,
                column,
                character,
                source,
            }
        })
    }
}

/// 递归排序 JSON 对象中的所有 Key。
///
/// - **对象**：将 Key 收集到 `BTreeMap` 中自动排序，然后递归处理每个值。
/// - **数组**：保留数组元素顺序，只递归处理每个元素。
/// - **其他类型**（字符串、数字、布尔、null）：保持不变。
///
/// 这对于 JSON 比较很重要：消除键顺序差异，使比较结果稳定可预测。
pub fn sort_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .map(|(k, v)| (k, sort_keys(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(sort_keys).collect()),
        value => value,
    }
}

/// 转义字符串为 JSON 字符串字面量。
///
/// 例如 `hello "world"` → `"hello \"world\""`。
///
/// ### 幂等性
///
/// 如果输入已经是合法的 JSON 字符串字面量（即 `serde_json::from_str` 可以解码），
/// 则先解码再编码，确保多次调用 `escape` 不会产生重复转义：
/// `escape(escape("hello"))` == `escape("hello")`。
pub fn escape(text: &str) -> String {
    // 尝试解码，如果已经是合法字符串字面量则先解码
    let decoded = serde_json::from_str::<String>(text).unwrap_or_else(|_| text.to_owned());
    // 编码为 JSON 字符串字面量
    serde_json::to_string(&decoded).expect("字符串序列化不会失败")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试：严格 JSON 和 JSON5 解析的差异。
    #[test]
    fn strict_json_and_json5_are_distinct() {
        // 尾逗号在严格 JSON 中非法，在 JSON5 中合法
        assert!(parse("{a:1,}", false).is_err());
        assert!(parse("{a:1,}", true).is_ok());
    }

    /// 测试：转义函数具有幂等性。
    #[test]
    fn escape_is_idempotent() {
        let raw = "hello\n\t\"world\"\\";
        let encoded = escape(raw);
        // 多次转义结果不变
        assert_eq!(escape(&encoded), encoded);
        // 转义结果可以解码回原始字符串
        assert_eq!(serde_json::from_str::<String>(&encoded).unwrap(), raw);
    }

    /// 测试：Unicode 字符的错误位置计算正确。
    #[test]
    fn unicode_error_location_counts_characters() {
        // "中文" 是 2 个字符，} 在字符位置 7
        let error = parse("{\"中文\":}", false).unwrap_err();
        assert!(error.to_string().contains("字符位置 7"));
    }
}