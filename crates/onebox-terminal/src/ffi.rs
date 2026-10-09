//! FFI 转换辅助：仅在本层与 `lib.rs` 出现 `unsafe`。
//!
//! 业务逻辑在 `session`（禁止 unsafe）；这里只做 C 类型 ↔ Rust 类型的转换。

use std::ffi::{c_char, CStr};

use crate::{onebox_terminal, onebox_terminal_rc};

/// 把可空 C 字符串转成 `Option<String>`。
///
/// # Safety
/// `ptr` 必须是 NULL，或指向以 NUL 结尾的有效 C 字符串。
pub(crate) unsafe fn read_opt_cstr(
    ptr: *const c_char,
    name: &str,
) -> Result<Option<String>, onebox_terminal_rc> {
    if ptr.is_null() {
        return Ok(None);
    }
    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(text) => Ok(Some(text.to_owned())),
        Err(_) => {
            crate::set_last_error(format!("{name} 不是合法的 UTF-8 字符串"));
            Err(onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT)
        }
    }
}

/// 校验句柄非空并返回引用。
///
/// # Safety
/// `handle` 必须是 `onebox_terminal_spawn` 返回、尚未 `onebox_terminal_free`
/// 释放的有效指针。
pub(crate) unsafe fn terminal_ref<'a>(
    handle: *mut onebox_terminal,
) -> Result<&'a onebox_terminal, onebox_terminal_rc> {
    if handle.is_null() {
        crate::set_last_error("handle 不能为 NULL");
        return Err(onebox_terminal_rc::ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT);
    }
    Ok(unsafe { &*handle })
}
