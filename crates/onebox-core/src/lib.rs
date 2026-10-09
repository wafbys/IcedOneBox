//! IcedOneBox 核。
//!
//! 稳定的核 + 可替换的外衣。本 crate 是核的**纯逻辑**部分：
//! - 不依赖任何 GUI、平台特定代码或外衣；
//! - 通过 `#![forbid(unsafe_code)]` 禁止 unsafe（unsafe 只允许出现在 FFI 层）；
//! - 对外只暴露 Rust API，C ABI 在 `onebox-ffi` 中提供。
//!
//! 行为规范位于仓库 `spec/` 目录，是核行为的唯一权威来源。

#![forbid(unsafe_code)]

mod error;
mod event;
mod id;
mod kernel;
mod store;

pub use error::{CoreError, Result};
pub use event::{Event, EventFilter, EventInput, EventQueryResult};
pub use id::generate_id;
pub use kernel::{Core, CoreConfig};
