//! ID 生成。
//!
//! 契约「万物有 ID」：任何实体、事件、消息都有唯一标识，ID 一旦生成永不改变。
//!
//! 这里使用 UUID v7：全局唯一且按时间有序，适合作为事件日志的排序依据。

/// 生成一个新的 ID（UUID v7，小写连字符形式，36 字符）。
pub fn generate_id() -> String {
    uuid::Uuid::now_v7().to_string()
}
