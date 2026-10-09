//! 核的错误类型。
//!
//! FFI 层负责把 [`CoreError`] 映射为 `onebox_rc` 返回码，并通过
//! `onebox_last_error()` 暴露文本详情。
//!
//! 错误只用于“契约被违反”或“存储失败”，不用于表达业务判断
//! （核不解析消息内容，也不判断对错）。

use std::fmt;

/// 核内部统一结果类型。
pub type Result<T> = std::result::Result<T, CoreError>;

/// 核内部错误。
#[derive(Debug)]
pub enum CoreError {
    /// 参数缺失、为空或取值非法。
    InvalidArgument(String),
    /// JSON 解析失败，或结构不符合契约。
    InvalidJson(String),
    /// 查询目标不存在。
    NotFound(String),
    /// 存储层（SQLite）失败。
    Storage(String),
    /// 未归类的内部错误（例如核已关闭）。
    Internal(String),
}

impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::InvalidArgument(m) => write!(f, "参数非法: {m}"),
            CoreError::InvalidJson(m) => write!(f, "JSON 非法: {m}"),
            CoreError::NotFound(m) => write!(f, "未找到: {m}"),
            CoreError::Storage(m) => write!(f, "存储错误: {m}"),
            CoreError::Internal(m) => write!(f, "内部错误: {m}"),
        }
    }
}

impl std::error::Error for CoreError {}

impl From<rusqlite::Error> for CoreError {
    fn from(err: rusqlite::Error) -> Self {
        CoreError::Storage(err.to_string())
    }
}
