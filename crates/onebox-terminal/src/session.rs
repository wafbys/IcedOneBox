//! 终端会话的纯逻辑：PTY 生命周期 + 读线程 + 输出缓冲。
//!
//! 本模块禁止 `unsafe`：只依赖 `portable-pty` 与标准库。
//!
//! 线程模型：一个会话 = 一个 PTY master + 一个子进程 + 一条读线程。
//! 读线程阻塞在 PTY 上，把输出推进一个带上限的共享缓冲；UI 线程通过
//! [`Session::read`] 非阻塞取走。缓冲满时读线程等待，形成背压，既不丢数据
//! 也不无限涨内存。

#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize};
use serde::Deserialize;

use crate::error::{Result, TerminalError};

/// 输出缓冲上限（字节）。超过后读线程阻塞，等待 UI 取走。
const OUTPUT_BUFFER_LIMIT: usize = 4 * 1024 * 1024;

/// 默认程序。Windows 上的 PowerShell；客场（macOS/Linux）需另行配置。
const DEFAULT_PROGRAM: &str = "powershell.exe";
/// 默认终端列数。
const DEFAULT_COLS: u16 = 80;
/// 默认终端行数。
const DEFAULT_ROWS: u16 = 24;

/// 关闭时等待读线程退出的上限；超时则放弃 join，避免阻塞 UI。
const READER_JOIN_TIMEOUT: Duration = Duration::from_secs(2);

/// 启动配置（已从 JSON 解析并校验）。
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    /// 要启动的程序。
    pub program: String,
    /// 程序参数。
    pub args: Vec<String>,
    /// 工作目录；`None` 表示继承当前目录。
    pub cwd: Option<String>,
    /// 终端列数。
    pub cols: u16,
    /// 终端行数。
    pub rows: u16,
}

impl SpawnConfig {
    /// 从 JSON 文本解析。`None` 或空串表示全部使用默认值。
    ///
    /// 未知字段一律拒绝（与核的严格解析风格一致），避免「写了但没生效」。
    pub fn from_json(json: Option<&str>) -> Result<Self> {
        let raw = match json {
            None => RawConfig::default(),
            Some(text) if text.trim().is_empty() => RawConfig::default(),
            Some(text) => serde_json::from_str::<RawConfig>(text)
                .map_err(|err| TerminalError::InvalidJson(err.to_string()))?,
        };

        if raw.program.trim().is_empty() {
            return Err(TerminalError::InvalidArgument("program 不能为空".to_string()));
        }
        if raw.cols == 0 || raw.rows == 0 {
            return Err(TerminalError::InvalidArgument(
                "cols 与 rows 必须 >= 1".to_string(),
            ));
        }

        Ok(SpawnConfig {
            program: raw.program,
            args: raw.args,
            cwd: raw.cwd,
            cols: raw.cols,
            rows: raw.rows,
        })
    }
}

/// JSON 原始结构（仅内部使用）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    #[serde(default = "default_program")]
    program: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default = "default_cols")]
    cols: u16,
    #[serde(default = "default_rows")]
    rows: u16,
}

impl Default for RawConfig {
    fn default() -> Self {
        Self {
            program: default_program(),
            args: Vec::new(),
            cwd: None,
            cols: default_cols(),
            rows: default_rows(),
        }
    }
}

fn default_program() -> String {
    DEFAULT_PROGRAM.to_string()
}

fn default_cols() -> u16 {
    DEFAULT_COLS
}

fn default_rows() -> u16 {
    DEFAULT_ROWS
}

/// 一个 PTY 会话。
pub struct Session {
    /// PTY master；`close` 时取出并丢弃以关闭 PTY（并让读线程的 read 返回）。
    master: Option<Box<dyn MasterPty + Send>>,
    /// 写入端（stdin）。
    writer: Box<dyn Write + Send>,
    /// 子进程，用于 kill / 查询退出状态。
    child: Box<dyn Child + Send + Sync>,
    /// 与读线程共享的输出缓冲。
    output: Arc<OutputBuffer>,
    /// 读线程句柄。
    reader: Option<JoinHandle<()>>,
    /// 读线程结束信号（用于带超时的 join）。
    reader_done: Option<Receiver<()>>,
    /// 是否已关闭。
    closed: bool,
}

impl Session {
    /// 打开 PTY 并启动子进程，随后起读线程。
    pub fn spawn(config: SpawnConfig) -> Result<Self> {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(TerminalError::spawn)?;

        let mut command = CommandBuilder::new(&config.program);
        for arg in &config.args {
            command.arg(arg);
        }
        if let Some(cwd) = &config.cwd {
            command.cwd(cwd);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(TerminalError::spawn)?;
        // 释放 slave：读出/写入都走 master。
        drop(pair.slave);

        let writer = pair.master.take_writer().map_err(TerminalError::io)?;
        let reader = pair.master.try_clone_reader().map_err(TerminalError::io)?;

        let output = Arc::new(OutputBuffer::new());
        let (done_tx, done_rx) = mpsc::channel();
        let reader_output = Arc::clone(&output);
        let reader_handle = std::thread::Builder::new()
            .name("onebox-terminal-reader".to_string())
            .spawn(move || {
                read_loop(reader, &reader_output);
                // 线程结束（正常或出错）都要通知 close，避免无谓等待。
                let _ = done_tx.send(());
            })
            .map_err(TerminalError::io)?;

        Ok(Session {
            master: Some(pair.master),
            writer,
            child,
            output,
            reader: Some(reader_handle),
            reader_done: Some(done_rx),
            closed: false,
        })
    }

    /// 向 PTY 写入输入（键盘 / 粘贴），阻塞写完并 flush。
    pub fn write(&mut self, data: &[u8]) -> Result<()> {
        self.writer.write_all(data).map_err(TerminalError::io)?;
        self.writer.flush().map_err(TerminalError::io)?;
        Ok(())
    }

    /// 非阻塞取走已就绪的输出，返回实际写出的字节数（无数据时为 0）。
    pub fn read(&mut self, buf: &mut [u8]) -> usize {
        self.output.drain_into(buf)
    }

    /// 调整 PTY 尺寸，窗口 resize 时调用。
    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<()> {
        if let Some(master) = &self.master {
            master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(TerminalError::io)?;
        }
        Ok(())
    }

    /// 子进程是否已退出；退出时返回其退出码（不可得时为 -1）。
    pub fn status(&mut self) -> Result<(bool, i32)> {
        match self.child.try_wait().map_err(TerminalError::io)? {
            Some(exit) => Ok((true, exit.exit_code() as i32)),
            None => Ok((false, -1)),
        }
    }

    /// 关闭会话：终止子进程、停止读线程、释放 PTY。可重复调用。
    pub fn close(&mut self) {
        if self.closed {
            return;
        }
        self.closed = true;

        // 先唤醒可能正阻塞在「缓冲满」上的读线程，再杀进程、关 PTY。
        self.output.stop();
        let _ = self.child.kill();
        self.master.take();

        // 带超时地等待读线程退出：正常情况下立刻返回；异常时放弃 join，
        // 宁可泄漏一条线程也不阻塞 UI。
        if let Some(done) = self.reader_done.take() {
            match done.recv_timeout(READER_JOIN_TIMEOUT) {
                Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                    if let Some(handle) = self.reader.take() {
                        let _ = handle.join();
                    }
                }
                Err(RecvTimeoutError::Timeout) => {
                    // 放弃 join（detach）。
                    self.reader.take();
                }
            }
        }
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.close();
    }
}

/// 读线程主体：把 PTY 输出推进共享缓冲，直到 EOF 或出错。
fn read_loop(mut reader: Box<dyn Read + Send>, output: &OutputBuffer) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => output.push(&buf[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        }
    }
    output.mark_eof();
}

/// 共享输出缓冲：带上限与背压。
struct OutputBuffer {
    state: Mutex<BufferState>,
    cond: Condvar,
}

struct BufferState {
    data: VecDeque<u8>,
    /// 读线程应停止（关闭时置位）。
    stop: bool,
    /// PTY 已到 EOF。
    eof: bool,
}

impl OutputBuffer {
    fn new() -> Self {
        Self {
            state: Mutex::new(BufferState {
                data: VecDeque::new(),
                stop: false,
                eof: false,
            }),
            cond: Condvar::new(),
        }
    }

    /// 读线程调用：写入数据；缓冲满则等待（背压）。
    fn push(&self, bytes: &[u8]) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.data.len() >= OUTPUT_BUFFER_LIMIT && !state.stop {
            state = self.cond.wait(state).unwrap_or_else(|e| e.into_inner());
        }
        if state.stop {
            return;
        }
        state.data.extend(bytes);
    }

    /// UI 线程调用：取走最多 `buf.len()` 字节，返回实际数量。
    fn drain_into(&self, buf: &mut [u8]) -> usize {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let count = buf.len().min(state.data.len());
        for slot in buf.iter_mut().take(count) {
            *slot = state.data.pop_front().expect("长度已校验");
        }
        if count > 0 {
            // 唤醒可能因缓冲满而等待的读线程。
            self.cond.notify_all();
        }
        count
    }

    /// 读线程退出时标记 EOF。
    fn mark_eof(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.eof = true;
        self.cond.notify_all();
    }

    /// 关闭时置停止标志并唤醒读线程。
    fn stop(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        state.stop = true;
        self.cond.notify_all();
    }

    /// PTY 是否已到 EOF（当前未使用，保留给 status 扩展）。
    #[allow(dead_code)]
    fn is_eof(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .eof
    }
}
