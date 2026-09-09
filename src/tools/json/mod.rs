//! ## JSON 工具模块
//!
//! 本模块包含所有 JSON 相关工具：格式化、比较，以及共享的内部实现。
//!
//! - [`compare`]：JSON 比较工具（并排编辑器，按 Key 对齐差异）
//! - [`formatter`]：JSON / JSON5 格式化工具（格式化、压缩、转义）
//! - [`diff`]（私有）：JSON 比较引擎，负责按 Key 对齐和差异标记
//! - [`utils`]（私有）：JSON 解析、排序、转义等公共工具函数
//!
//! ### 模块结构
//!
//! ```text
//! json/
//! ├── mod.rs        # 模块入口和公开导出
//! ├── compare.rs    # JsonCompareTool — 并排编辑器，按 Key 对齐
//! ├── formatter.rs  # JsonFormatterTool — 格式化/压缩/转义
//! ├── diff.rs       # 比较引擎（私有）：align、Comparison、ChangeKind
//! └── utils.rs      # 公共工具（私有）：parse、sort_keys、escape
//! ```

/// JSON 比较工具：并排显示两个 JSON 编辑器，比较时按 Key 对齐并高亮差异。
pub mod compare;
/// JSON / JSON5 格式化工具：格式化、压缩、转义/取消转义。
pub mod formatter;

// —— 以下为私有模块 ——
//
// 这些模块是 JSON 工具的内部实现，不对外暴露：
// - `diff`：JSON 比较引擎，负责按 Key 对齐和差异标记。
// - `utils`：JSON 解析、排序、转义等公共工具函数，被 compare 和 formatter 共享。

/// JSON 比较引擎：按对象 Key 和数组下标对齐，标记差异。
mod diff;
/// JSON 公共工具函数：解析（含 JSON5）、Key 排序、转义/取消转义。
mod utils;