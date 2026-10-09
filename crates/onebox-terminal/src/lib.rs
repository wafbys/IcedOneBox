//! IcedOneBox 终端模块的 C ABI 层。
//!
//! 与核的 `onebox-ffi` 一样是「薄」边界层：只做类型转换、panic 拦截、错误
//! 映射，不写业务逻辑。业务逻辑在 [`session`] 中，且禁止 unsafe。
//!
//! 终端是**独立模块**（不是核的一部分）：PTY 管理不属于核的五条契约。
//! 返回码用自有的 [`onebox_terminal_rc`]，数值与核的 `onebox_rc` 刻意对齐，
//! 以便外衣侧复用同一个枚举。
//!
//! ## 对「跨边界统一用 JSON」的例外
//!
//! PTY 的输入输出是**裸字节流**（含 VT 转义、可能被截断的多字节 UTF-8），
//! 塞进 JSON 字符串既慢又易错，因此 `read` / `write` 用裸缓冲；只有 `spawn`
//! 的配置走 JSON。此例外已记入 `spec/90-ffi.md` 的「字节流例外」。
//!
//! ## 前置条件（Windows / ConPTY）
//!
//! 本模块只负责 PTY，不做终端仿真。ConPTY 建立后会发出 `ESC[6n`（DSR，查询
//! 光标位置），**消费者必须应答 CPR（`ESC[row;colR`）**，否则子进程输出不会
//! 继续。xterm.js 等真实终端会自动应答；非终端消费者（脚本、测试）需自行应答。

#![allow(non_camel_case_types)]

mod error;
mod ffi;

pub mod session;

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

pub use error::TerminalError;
use ffi::{read_opt_cstr, terminal_ref};
use session::{Session, SpawnConfig};

/// 终端会话句柄（对 C 侧不透明）。
///
/// 内部用 `Mutex` 串行化访问，因此可被多个线程共享调用。
pub struct onebox_terminal {
    inner: Mutex<Session>,
}

/// 终端模块返回码。数值与核的 `onebox_rc` 对齐（0/1/2/5/99）；
/// 终端没有存储层，故不含 `4`。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum onebox_terminal_rc {
    /// 成功。
    ONEBOX_TERMINAL_OK = 0,
    /// 参数缺失或非法（空指针、cols/rows 为 0 等）。
    ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT = 1,
    /// 配置 JSON 解析失败或含未知字段。
    ONEBOX_TERMINAL_ERR_INVALID_JSON = 2,
    /// Spawn / IO 等失败统一归到这里；需要区分时再新增返回码。
    ONEBOX_TERMINAL_ERR_INTERNAL = 5,
    /// FFI 边界捕获到 panic。
    ONEBOX_TERMINAL_ERR_PANIC = 99,
}

thread_local! {
    /// 每个线程各自的“最后一次错误”文本。
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// 记录错误文本到当前线程。
pub(crate) fn set_last_error(message: impl Into<String>) {
    let message = message.into();
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = CString::new(message)
            .unwrap_or_else(|_| CString::new("error message contained NUL").unwrap());
    });
}

/// 清空当前线程的错误文本。
fn clear_last_error() {
    set_last_error("");
}

/// 把终端错误映射为返回码。
fn rc_of(err: &TerminalError) -> onebox_terminal_rc {
    match err {
        TerminalError::InvalidArgument(_) => onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT,
        TerminalError::InvalidJson(_) => onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_JSON,
        TerminalError::Spawn(_) | TerminalError::Io(_) | TerminalError::Internal(_) => {
            onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INTERNAL
        }
    }
}

/// 记录错误文本并返回对应返回码。
fn record(err: &TerminalError) -> onebox_terminal_rc {
    set_last_error(err.to_string());
    rc_of(err)
}

/// 处理 panic：记录并返回 panic 返回码。
fn on_panic() -> onebox_terminal_rc {
    set_last_error("FFI 边界捕获到 panic");
    onebox_terminal_rc::ONEBOX_TERMINAL_ERR_PANIC
}

/// 启动一个终端会话。
///
/// `config_json` 可为 NULL 或空串，表示使用默认配置（`powershell.exe`，80x24）。
/// 成功时把句柄写入 `out_handle`（用 `onebox_terminal_free()` 释放）；失败时
/// `*out_handle` 保持 NULL，详情用 `onebox_terminal_last_error()` 获取。
///
/// # Safety
/// `config_json` 必须是 NULL 或指向以 NUL 结尾的 UTF-8 JSON 字符串；
/// `out_handle` 必须指向可写的 `onebox_terminal *` 变量。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_spawn(
    config_json: *const c_char,
    out_handle: *mut *mut onebox_terminal,
) -> onebox_terminal_rc {
    clear_last_error();
    if out_handle.is_null() {
        set_last_error("out_handle 不能为 NULL");
        return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
    }
    unsafe { *out_handle = std::ptr::null_mut() };

    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let json = match unsafe { read_opt_cstr(config_json, "config_json") } {
            Ok(value) => value,
            Err(code) => return code,
        };
        let config = match SpawnConfig::from_json(json.as_deref()) {
            Ok(config) => config,
            Err(err) => return record(&err),
        };
        let session = match Session::spawn(config) {
            Ok(session) => session,
            Err(err) => return record(&err),
        };
        let handle = Box::into_raw(Box::new(onebox_terminal {
            inner: Mutex::new(session),
        }));
        unsafe { *out_handle = handle };
        onebox_terminal_rc::ONEBOX_TERMINAL_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 向终端写入输入字节（键盘 / 粘贴），阻塞写完。
///
/// # Safety
/// `handle` 必须有效；`data` 必须指向 `len` 字节的可读内存（`len` 为 0 时可为 NULL）。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_write(
    handle: *mut onebox_terminal,
    data: *const u8,
    len: usize,
) -> onebox_terminal_rc {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let terminal = match unsafe { terminal_ref(handle) } {
            Ok(terminal) => terminal,
            Err(code) => return code,
        };
        if data.is_null() && len != 0 {
            set_last_error("data 不能为 NULL（len > 0）");
            return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
        }
        let slice = if len == 0 {
            &[][..]
        } else {
            unsafe { std::slice::from_raw_parts(data, len) }
        };
        let mut guard = terminal.inner.lock().unwrap_or_else(|e| e.into_inner());
        match guard.write(slice) {
            Ok(()) => onebox_terminal_rc::ONEBOX_TERMINAL_OK,
            Err(err) => record(&err),
        }
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 非阻塞取走已就绪的输出。
///
/// 成功时 `*out_len` 为实际写出的字节数（无数据时为 0）。调用方负责提供
/// `cap` 字节的可写缓冲。
///
/// # Safety
/// `handle` 必须有效；`buf` 必须指向 `cap` 字节的可写内存（`cap` 为 0 时可为
/// NULL）；`out_len` 必须指向可写的 `size_t`。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_read(
    handle: *mut onebox_terminal,
    buf: *mut u8,
    cap: usize,
    out_len: *mut usize,
) -> onebox_terminal_rc {
    clear_last_error();
    if out_len.is_null() {
        set_last_error("out_len 不能为 NULL");
        return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
    }
    unsafe { *out_len = 0 };

    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let terminal = match unsafe { terminal_ref(handle) } {
            Ok(terminal) => terminal,
            Err(code) => return code,
        };
        if buf.is_null() && cap != 0 {
            set_last_error("buf 不能为 NULL（cap > 0）");
            return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
        }
        let slice = if cap == 0 {
            &mut [][..]
        } else {
            unsafe { std::slice::from_raw_parts_mut(buf, cap) }
        };
        let mut guard = terminal.inner.lock().unwrap_or_else(|e| e.into_inner());
        let count = guard.read(slice);
        unsafe { *out_len = count };
        onebox_terminal_rc::ONEBOX_TERMINAL_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 调整 PTY 尺寸（列 / 行）。
///
/// # Safety
/// `handle` 必须有效。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_resize(
    handle: *mut onebox_terminal,
    cols: u16,
    rows: u16,
) -> onebox_terminal_rc {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let terminal = match unsafe { terminal_ref(handle) } {
            Ok(terminal) => terminal,
            Err(code) => return code,
        };
        if cols == 0 || rows == 0 {
            set_last_error("cols 与 rows 必须 >= 1");
            return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
        }
        let mut guard = terminal.inner.lock().unwrap_or_else(|e| e.into_inner());
        match guard.resize(cols, rows) {
            Ok(()) => onebox_terminal_rc::ONEBOX_TERMINAL_OK,
            Err(err) => record(&err),
        }
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 查询子进程状态。
///
/// `*out_exited` 为 1 表示已退出，此时 `*out_code` 为退出码（不可得时为 -1）。
///
/// # Safety
/// `handle` 必须有效；`out_exited` / `out_code` 必须指向可写内存。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_status(
    handle: *mut onebox_terminal,
    out_exited: *mut c_int,
    out_code: *mut i32,
) -> onebox_terminal_rc {
    clear_last_error();
    if out_exited.is_null() || out_code.is_null() {
        set_last_error("out_exited / out_code 不能为 NULL");
        return onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT;
    }
    unsafe {
        *out_exited = 0;
        *out_code = -1;
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let terminal = match unsafe { terminal_ref(handle) } {
            Ok(terminal) => terminal,
            Err(code) => return code,
        };
        let mut guard = terminal.inner.lock().unwrap_or_else(|e| e.into_inner());
        match guard.status() {
            Ok((exited, code)) => {
                unsafe {
                    *out_exited = if exited { 1 } else { 0 };
                    *out_code = code;
                }
                onebox_terminal_rc::ONEBOX_TERMINAL_OK
            }
            Err(err) => record(&err),
        }
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 关闭会话：终止子进程、停止读线程、释放 PTY。可重复调用；句柄本身仍有效，
/// 需另用 `onebox_terminal_free()` 释放。
///
/// # Safety
/// `handle` 必须有效。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_close(handle: *mut onebox_terminal) -> onebox_terminal_rc {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_terminal_rc {
        let terminal = match unsafe { terminal_ref(handle) } {
            Ok(terminal) => terminal,
            Err(code) => return code,
        };
        let mut guard = terminal.inner.lock().unwrap_or_else(|e| e.into_inner());
        guard.close();
        onebox_terminal_rc::ONEBOX_TERMINAL_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 释放终端句柄。传入 NULL 安全；同一句柄只能释放一次。
///
/// 与 `onebox_core_free` 一致，本函数返回返回码（正常路径为
/// `ONEBOX_TERMINAL_OK`）。drop 会关闭会话（`Session::drop` → `close`）。
///
/// # Safety
/// `handle` 必须是 `onebox_terminal_spawn` 返回且尚未释放的指针，或 NULL。
#[no_mangle]
pub unsafe extern "C" fn onebox_terminal_free(
    handle: *mut onebox_terminal,
) -> onebox_terminal_rc {
    clear_last_error();
    if handle.is_null() {
        return onebox_terminal_rc::ONEBOX_TERMINAL_OK;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        drop(unsafe { Box::from_raw(handle) });
        onebox_terminal_rc::ONEBOX_TERMINAL_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 返回当前线程最后一次错误的文本。
///
/// 指针指向线程局部缓冲，调用方**不要**释放。
#[no_mangle]
pub extern "C" fn onebox_terminal_last_error() -> *const c_char {
    match catch_unwind(AssertUnwindSafe(|| {
        LAST_ERROR.with(|cell| cell.borrow().as_ptr())
    })) {
        Ok(pointer) => pointer,
        Err(_) => std::ptr::null(),
    }
}
