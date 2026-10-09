//! IcedOneBox 核的 C ABI 层。
//!
//! 本 crate 是核中**唯一**允许使用 `unsafe` 的地方，职责仅有三项：
//! 1. 在 C 类型与 Rust 类型之间转换；
//! 2. 用 `catch_unwind` 拦截 panic，绝不让 panic 穿过 FFI 边界；
//! 3. 把 [`CoreError`] 映射为 [`onebox_rc`] 返回码，并写入 `onebox_last_error()`。
//!
//! 业务逻辑一律在 `onebox-core` 中实现；本层不加任何业务逻辑。

#![allow(non_camel_case_types)]

use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Mutex;

// 注意：本 crate 的不透明结构体也叫 `onebox_core`，会遮蔽同名依赖 crate。
// 这里用前导 `::` 明确指向外部 crate（onebox-core），避免解析歧义。
use ::onebox_core::{generate_id, Core, CoreConfig, CoreError};

/// 核句柄。
///
/// 对 C 侧是不透明类型：只能持有 `onebox_core *`，无法访问内部字段。
/// 句柄内部用 `Mutex` 串行化访问，因此可以被多个模块线程共享调用。
pub struct onebox_core {
    inner: Mutex<Core>,
}

/// 统一返回码。`ONEBOX_OK` 为 0，其余为错误。
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum onebox_rc {
    /// 成功。
    ONEBOX_OK = 0,
    /// 参数缺失或非法（如空指针、空 `event_type`）。
    ONEBOX_ERR_INVALID_ARGUMENT = 1,
    /// JSON 解析失败，或结构不符合契约（如提供了保留字段）。
    ONEBOX_ERR_INVALID_JSON = 2,
    /// 查询目标不存在。
    ONEBOX_ERR_NOT_FOUND = 3,
    /// 存储层失败。
    ONEBOX_ERR_STORAGE = 4,
    /// 未归类的内部错误。
    ONEBOX_ERR_INTERNAL = 5,
    /// FFI 边界捕获到 panic。
    ONEBOX_ERR_PANIC = 99,
}

thread_local! {
    /// 每个线程各自的“最后一次错误”文本。
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

/// 记录错误文本到当前线程。
fn set_last_error(message: impl Into<String>) {
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

/// 把核错误映射为返回码。
fn rc_of(err: &CoreError) -> onebox_rc {
    match err {
        CoreError::InvalidArgument(_) => onebox_rc::ONEBOX_ERR_INVALID_ARGUMENT,
        CoreError::InvalidJson(_) => onebox_rc::ONEBOX_ERR_INVALID_JSON,
        CoreError::NotFound(_) => onebox_rc::ONEBOX_ERR_NOT_FOUND,
        CoreError::Storage(_) => onebox_rc::ONEBOX_ERR_STORAGE,
        CoreError::Internal(_) => onebox_rc::ONEBOX_ERR_INTERNAL,
    }
}

/// 记录错误文本并返回对应返回码。
fn record(err: &CoreError) -> onebox_rc {
    set_last_error(err.to_string());
    rc_of(err)
}

/// 处理 panic：记录并返回 panic 返回码。
fn on_panic() -> onebox_rc {
    set_last_error("FFI 边界捕获到 panic");
    onebox_rc::ONEBOX_ERR_PANIC
}

/// 把可空 C 字符串转成 `Option<String>`。
///
/// # Safety
/// `ptr` 必须是 NULL，或指向以 NUL 结尾的有效 C 字符串。
unsafe fn read_opt_cstr(ptr: *const c_char, name: &str) -> Result<Option<String>, onebox_rc> {
    if ptr.is_null() {
        return Ok(None);
    }
    match CStr::from_ptr(ptr).to_str() {
        Ok(text) => Ok(Some(text.to_owned())),
        Err(_) => {
            set_last_error(format!("{name} 不是合法的 UTF-8 字符串"));
            Err(onebox_rc::ONEBOX_ERR_INVALID_ARGUMENT)
        }
    }
}

/// 把必填 C 字符串转成 `String`。
///
/// # Safety
/// `ptr` 必须指向以 NUL 结尾的有效 C 字符串。
unsafe fn read_cstr(ptr: *const c_char, name: &str) -> Result<String, onebox_rc> {
    match read_opt_cstr(ptr, name)? {
        Some(text) => Ok(text),
        None => {
            set_last_error(format!("{name} 不能为 NULL"));
            Err(onebox_rc::ONEBOX_ERR_INVALID_ARGUMENT)
        }
    }
}

/// 校验句柄非空并返回引用。
///
/// # Safety
/// `handle` 必须是 `onebox_core_init` 返回、尚未 `onebox_core_free` 释放的有效
/// 指针；返回的引用仅在句柄被释放之前有效，调用方必须保证此期间句柄不被释放，
/// 也不与其他线程对该句柄的调用产生竞态（句柄内部以 `Mutex` 串行化）。
unsafe fn core_ref<'a>(handle: *mut onebox_core) -> Result<&'a onebox_core, onebox_rc> {
    if handle.is_null() {
        set_last_error("handle 不能为 NULL");
        return Err(onebox_rc::ONEBOX_ERR_INVALID_ARGUMENT);
    }
    Ok(&*handle)
}

/// 把 Rust 字符串转成核分配的 C 字符串指针（由 `onebox_string_free` 释放）。
fn into_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(c_string) => c_string.into_raw(),
        Err(_) => {
            set_last_error("字符串包含 NUL，无法转换为 C 字符串");
            std::ptr::null_mut()
        }
    }
}

/// 初始化核。
///
/// `config_json` 可为 NULL 或空串，表示使用默认配置。成功返回句柄，失败返回
/// NULL，详情用 `onebox_last_error()` 获取。返回的句柄必须用 `onebox_core_free()`
/// 释放。
///
/// # Safety
/// `config_json` 必须是 NULL 或指向以 NUL 结尾的 UTF-8 JSON 字符串。
#[no_mangle]
pub unsafe extern "C" fn onebox_core_init(config_json: *const c_char) -> *mut onebox_core {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let json = unsafe { read_opt_cstr(config_json, "config_json") }?;
        let config = CoreConfig::from_json(json.as_deref()).map_err(|err| record(&err))?;
        let core = Core::init(config).map_err(|err| record(&err))?;
        Ok::<*mut onebox_core, onebox_rc>(Box::into_raw(Box::new(onebox_core {
            inner: Mutex::new(core),
        })))
    }));

    match outcome {
        Ok(Ok(handle)) => handle,
        Ok(Err(_)) => std::ptr::null_mut(),
        Err(_) => {
            on_panic();
            std::ptr::null_mut()
        }
    }
}

/// 关闭核：释放数据库连接。可重复调用。句柄本身仍然有效，需另用
/// `onebox_core_free()` 释放。
///
/// # Safety
/// `handle` 必须是 `onebox_core_init` 返回且尚未释放的有效指针。
#[no_mangle]
pub unsafe extern "C" fn onebox_core_shutdown(handle: *mut onebox_core) -> onebox_rc {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_rc {
        let core = match unsafe { core_ref(handle) } {
            Ok(core) => core,
            Err(code) => return code,
        };
        let mut guard = core.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.shutdown();
        onebox_rc::ONEBOX_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 释放核句柄。传入 NULL 安全；同一句柄只能释放一次。
///
/// # Safety
/// `handle` 必须是 `onebox_core_init` 返回且尚未释放的指针，或 NULL。
#[no_mangle]
pub unsafe extern "C" fn onebox_core_free(handle: *mut onebox_core) -> onebox_rc {
    clear_last_error();
    if handle.is_null() {
        return onebox_rc::ONEBOX_OK;
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        // 回收句柄内存。即使尚未 shutdown，drop 也会关闭数据库连接。
        drop(unsafe { Box::from_raw(handle) });
        onebox_rc::ONEBOX_OK
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 生成一个新的 ID。返回的字符串必须用 `onebox_string_free()` 释放。
#[no_mangle]
pub extern "C" fn onebox_id_generate() -> *mut c_char {
    clear_last_error();
    match catch_unwind(AssertUnwindSafe(|| into_c_string(generate_id()))) {
        Ok(pointer) => pointer,
        Err(_) => {
            on_panic();
            std::ptr::null_mut()
        }
    }
}

/// 追加一条事件。`id`、`seq`、时间戳由核生成；调用方在 `event_json` 中提供
/// 这些字段会被拒绝。
///
/// # Safety
/// `handle` 必须有效；`event_json` 必须指向以 NUL 结尾的 UTF-8 JSON 字符串。
#[no_mangle]
pub unsafe extern "C" fn onebox_event_append(
    handle: *mut onebox_core,
    event_json: *const c_char,
) -> onebox_rc {
    clear_last_error();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_rc {
        let core = match unsafe { core_ref(handle) } {
            Ok(core) => core,
            Err(code) => return code,
        };
        let json = match unsafe { read_cstr(event_json, "event_json") } {
            Ok(json) => json,
            Err(code) => return code,
        };
        let mut guard = core.inner.lock().unwrap_or_else(|err| err.into_inner());
        match guard.append_event(&json) {
            Ok(_) => onebox_rc::ONEBOX_OK,
            Err(err) => record(&err),
        }
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 查询事件。成功时把结果 JSON 写入 `out_json`，调用方必须用
/// `onebox_string_free()` 释放该字符串；失败时 `*out_json` 保持为 NULL。
///
/// `filter_json` 可为 NULL 或空串，表示无过滤条件。
///
/// # Safety
/// `handle` 必须有效；`filter_json` 必须是 NULL 或有效的 UTF-8 JSON 字符串；
/// `out_json` 必须指向可写的 `char *` 变量。
#[no_mangle]
pub unsafe extern "C" fn onebox_event_query(
    handle: *mut onebox_core,
    filter_json: *const c_char,
    out_json: *mut *mut c_char,
) -> onebox_rc {
    clear_last_error();
    if out_json.is_null() {
        set_last_error("out_json 不能为 NULL");
        return onebox_rc::ONEBOX_ERR_INVALID_ARGUMENT;
    }
    unsafe { *out_json = std::ptr::null_mut() };

    let outcome = catch_unwind(AssertUnwindSafe(|| -> onebox_rc {
        let core = match unsafe { core_ref(handle) } {
            Ok(core) => core,
            Err(code) => return code,
        };
        let filter = match unsafe { read_opt_cstr(filter_json, "filter_json") } {
            Ok(filter) => filter,
            Err(code) => return code,
        };
        let guard = core.inner.lock().unwrap_or_else(|err| err.into_inner());
        match guard.query_events(filter.as_deref()) {
            Ok(json) => {
                let pointer = into_c_string(json);
                if pointer.is_null() {
                    return onebox_rc::ONEBOX_ERR_INTERNAL;
                }
                unsafe { *out_json = pointer };
                onebox_rc::ONEBOX_OK
            }
            Err(err) => record(&err),
        }
    }));
    outcome.unwrap_or_else(|_| on_panic())
}

/// 释放由核分配的字符串。
///
/// 适用于 `onebox_id_generate()` 与 `onebox_event_query()` 返回的指针。
/// 传入 NULL 安全；同一指针只能释放一次。
///
/// 与其他核函数一致，本函数会清空当前线程的错误文本；若需读取错误详情，请在
/// 调用本函数之前先调用 `onebox_last_error()`。
///
/// # Safety
/// `value` 必须是核返回且尚未释放的指针，或 NULL。
#[no_mangle]
pub unsafe extern "C" fn onebox_string_free(value: *mut c_char) {
    clear_last_error();
    if value.is_null() {
        return;
    }
    drop(unsafe { CString::from_raw(value) });
}

/// 返回当前线程最后一次错误的文本。
///
/// 指针指向线程局部缓冲，调用方**不要**释放；在下一次调用其他会写入错误状态的
/// 核函数之前一直有效（注意 `onebox_string_free()` 也会清空错误文本）。
#[no_mangle]
pub extern "C" fn onebox_last_error() -> *const c_char {
    match catch_unwind(AssertUnwindSafe(|| {
        LAST_ERROR.with(|cell| cell.borrow().as_ptr())
    })) {
        Ok(pointer) => pointer,
        Err(_) => std::ptr::null(),
    }
}
