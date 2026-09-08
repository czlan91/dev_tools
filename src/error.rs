//! ## 项目级错误类型
//!
//! 本模块使用 `thiserror` 派生宏（derive macro）来定义统一的错误枚举。
//!
//! ### 为什么用 `thiserror` 而不是 `anyhow`？
//!
//! - `thiserror` 是「定义错误」的库：它让你为每个失败原因创建语义清晰的枚举变体，
//!   调用方可以根据不同变体做不同的恢复处理（例如区分「文件不存在」和「格式错误」）。
//! - `anyhow` 是「传播错误」的库：它适合在应用边界快速向上传递错误，
//!   但调用方无法区分错误原因，只能统一打印或弹框 —— 对工具类应用来说不够精确。
//! - 本项目在**业务逻辑层**统一使用 `thiserror`（即在 `from` 和 `?` 传播时保留错误链），
//!   仅在 `main.rs` 中因 gpui trait 签名要求而保留 `anyhow`（见该文件注释）。
//!
//! ### `thiserror` 的常用宏属性
//!
//! - `#[error("...")]` —— 定义该变体的 Display 文本，`{0}` `{1}` 引用字段，`{field}` 引用命名参数。
//! - `#[from]` —— 自动为源类型实现 `From` trait，允许直接用 `?` 转换。
//! - `#[source]` —— 显式标记底层错误链（默认自动推断 `source` 为名为 `source` 的字段）。

use std::error::Error;

/// 应用级错误枚举，覆盖所有工具模块的失败场景。
///
/// 每个变体都包含语义化的错误消息和（可选的）底层错误链，
/// 使得 UI 层可以精确展示错误原因，同时日志保留完整上下文。
///
/// 注意：`thiserror` 派生宏自动为 `AppError` 实现了 `std::error::Error` trait，
/// 而 `anyhow::Error` 有一个 blanket impl `From<E: StdError + Send + Sync + 'static>`，
/// 所以 `AppError` 可以直接转换为 `anyhow::Error`，无需手动实现 `From`。
///
/// 部分变体（Parse、AssetLoad、MissingHome）当前未使用，但保留为项目扩展预留。
/// 新增工具或功能时，应优先使用这些语义化的错误变体而非创建新的错误类型。
#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AppError {
    /// 文件 I/O 操作失败，例如读取图片或设置文件时权限不足或磁盘错误。
    #[error("文件操作失败：{context}（{source}）")]
    Io {
        /// 描述正在尝试的操作，例如「读取设置文件」。
        context: String,
        /// 底层 I/O 错误，保留了系统错误码等信息。
        #[source]
        source: std::io::Error,
    },

    /// 设置文件或主题文件解析失败。
    #[error("解析失败：{context}（{source}）")]
    Parse {
        /// 描述正在解析的内容，例如「设置文件 JSON」。
        context: String,
        /// 底层解析错误。
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },

    /// 资源加载失败，例如图片文件不存在或格式不支持。
    #[error("资源加载失败：{0}")]
    AssetLoad(String),

    /// 无法确定系统配置目录，通常意味着 HOME 环境变量未设置。
    #[error("无法确定配置目录：{0}")]
    MissingHome(String),
}

// 为 `AppError` 实现从 `std::io::Error` 的自动转换，
// 当文件操作返回 `std::io::Error` 时可以用 `?` 直接传播。
// 这里的 `context` 使用默认值，调用方应在更高层级补充具体操作描述。
impl From<std::io::Error> for AppError {
    fn from(source: std::io::Error) -> Self {
        AppError::Io {
            context: "文件读写".into(),
            source,
        }
    }
}