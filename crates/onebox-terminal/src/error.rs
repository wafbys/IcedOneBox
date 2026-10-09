//! 终端模块的内部错误类型。

use std::fmt;

/// 终端模块统一结果类型。
pub type Result<T> = std::result::Result<T, TerminalError>;

/// 终端模块内部错误。
///
/// FFI 层把 [`TerminalError`] 映射为返回码：`InvalidArgument` /
/// `InvalidJson` 各有对应码；`Spawn` / `Io` / `Internal` 统一归到
/// `ONEBOX_TERMINAL_ERR_INTERNAL`（需要区分时再新增返回码）。
#[derive(Debug)]
pub enum TerminalError {
    /// 参数缺失或取值非法（如 cols/rows 为 0）。
    InvalidArgument(String),
    /// 配置 JSON 解析失败或含未知字段。
    InvalidJson(String),
    /// 打开 PTY 或启动子进程失败。
    Spawn(String),
    /// PTY 读写、resize 等 IO 失败。
    Io(String),
    /// 未归类的内部错误。
    Internal(String),
}

impl TerminalError {
    /// 包装一个 spawn 错误。
    pub fn spawn(err: impl fmt::Display) -> Self {
        TerminalError::Spawn(err.to_string())
    }

    /// 包装一个 IO 错误。
    pub fn io(err: impl fmt::Display) -> Self {
        TerminalError::Io(err.to_string())
    }
}

impl fmt::Display for TerminalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalError::InvalidArgument(m) => write!(f, "参数非法: {m}"),
            TerminalError::InvalidJson(m) => write!(f, "JSON 非法: {m}"),
            TerminalError::Spawn(m) => write!(f, "启动失败: {m}"),
            TerminalError::Io(m) => write!(f, "IO 错误: {m}"),
            TerminalError::Internal(m) => write!(f, "内部错误: {m}"),
        }
    }
}

impl std::error::Error for TerminalError {}
