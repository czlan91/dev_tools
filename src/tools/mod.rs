//! ## 工具模块
//!
//! 本模块是 Dev Tools 中所有工具页面的集合。每个工具对应一个独立的文件，
//! 拥有自己的状态管理和渲染逻辑。
//!
//! ### 如何添加新工具
//!
//! 1. 在 `src/tools/` 下创建新文件（如 `my_tool.rs`），实现 `Render` trait。
//! 2. 在本文件中添加 `pub mod my_tool;` 声明。
//! 3. 在 `ToolId` 枚举中添加新的变体。
//! 4. 在 `src/app.rs` 的 `DevToolsApp` 中添加对应实体字段，并在 `render` 中匹配处理。
//! 5. 在 `src/app.rs` 的侧边栏中添加对应的菜单项。
//!
//! 详见 [ARCHITECTURE.md](../../ARCHITECTURE.md) 中的「新增工具步骤」。
//!
//! ### 工具与 `ToolId`
//!
//! `ToolId` 枚举用于标识当前激活的工具。它在 `DevToolsApp` 的 `active` 字段中保存，
//! `render` 方法根据其值决定显示哪个工具面板。
//! 使用 `PartialEq` 派生使得比较操作（如 `self.active == ToolId::TsvToSql`）是可行的。

// 公开声明各工具模块，使得 `crate::tools::tsv_to_sql` 等路径可被外部访问
pub mod image_to_base64;
pub mod json_compare;
pub mod json_formatter;
pub mod tsv_to_sql;

/// 工具标识符枚举。
///
/// 每个变体对应一个独立的工具页面。`Clone` 和 `Copy` 派生使得该枚举
/// 可以在不涉及所有权的情况下被复制，`PartialEq` 和 `Eq` 使得比较操作
/// （如 `==` 和 `match` 模式匹配）成为可能。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolId {
    /// TSV 数据 → SQL IN 条件列表
    TsvToSql,
    /// 图片文件 → Base64 编码字符串
    ImageToBase64,
    /// JSON 格式化 / 压缩 / 转义
    JsonFormatter,
    /// JSON5 格式化（JSON 超集，支持注释、尾逗号等）
    Json5Formatter,
    /// JSON 两侧比较（按 Key 对齐，高亮差异）
    JsonCompare,
}

// —— 以下为私有模块 ——
//
// 这些模块是工具的内部实现，不对外暴露：
// - `json_diff`：JSON 比较引擎，负责按 Key 对齐和差异标记。
// - `json_utils`：JSON 解析、排序、转义等公共工具函数。

/// JSON 比较引擎：按对象 Key 和数组下标对齐，标记差异。
mod json_diff;
/// JSON 公共工具函数：解析（含 JSON5）、Key 排序、转义/取消转义。
mod json_utils;