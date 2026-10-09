//! 事件模型与契约（见 `spec/20-events.md`）。

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, Result};

/// 事件追加输入。
///
/// 调用方**只允许**提供 `event_type` / `source` / `payload` 三个字段。
/// 由于启用了 `deny_unknown_fields`，任何额外字段（尤其是 `id` / `seq` /
/// `ts_millis`）都会导致解析失败：`id`、`seq`、时间戳一律由核生成，
/// 以保证事件日志的绝对秩序。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventInput {
    /// 事件类型，必填且非空白。
    pub event_type: String,
    /// 事件来源（模块 ID 或 `"core"`），可选。
    #[serde(default)]
    pub source: Option<String>,
    /// 事件负载，任意 JSON 值，可选（缺省为 `null`）。
    #[serde(default)]
    pub payload: Option<serde_json::Value>,
}

impl EventInput {
    /// 从 JSON 文本解析并做非空校验。
    pub fn from_json(json: &str) -> Result<Self> {
        let input: EventInput =
            serde_json::from_str(json).map_err(|err| CoreError::InvalidJson(err.to_string()))?;
        if input.event_type.trim().is_empty() {
            return Err(CoreError::InvalidArgument(
                "event_type 不能为空".to_string(),
            ));
        }
        Ok(input)
    }
}

/// 已落库的事件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// 全局单调序号，由核分配，从 1 开始。
    pub seq: i64,
    /// 事件 ID，由核分配（UUID v7）。
    pub id: String,
    /// 事件类型。
    pub event_type: String,
    /// 事件来源。
    pub source: Option<String>,
    /// 事件发生时间（Unix 毫秒），由核分配。
    pub ts_millis: i64,
    /// 事件负载，始终是合法 JSON 值（无负载时为 `null`）。
    pub payload: serde_json::Value,
}

/// 事件查询过滤器。
///
/// 未知字段一律拒绝，避免“写了但没生效”的查询条件。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventFilter {
    /// 精确匹配事件类型。
    #[serde(default)]
    pub event_type: Option<String>,
    /// 精确匹配事件来源。
    #[serde(default)]
    pub source: Option<String>,
    /// 仅返回 `seq` 严格大于该值的事件（翻页用）。
    #[serde(default)]
    pub since_seq: Option<i64>,
    /// 返回条数上限，缺省 100，最大 1000。
    #[serde(default)]
    pub limit: Option<u32>,
}

impl EventFilter {
    /// 缺省返回条数。
    pub const DEFAULT_LIMIT: u32 = 100;
    /// 单次返回条数上限。
    pub const MAX_LIMIT: u32 = 1000;

    /// 从 JSON 文本解析。`None` 或空串表示无过滤条件（返回全部，受上限约束）。
    pub fn from_json(json: Option<&str>) -> Result<Self> {
        match json {
            None => Ok(Self::default()),
            Some(text) if text.trim().is_empty() => Ok(Self::default()),
            Some(text) => {
                serde_json::from_str(text).map_err(|err| CoreError::InvalidJson(err.to_string()))
            }
        }
    }

    /// 实际生效的返回条数上限。
    pub fn effective_limit(&self) -> u32 {
        self.limit
            .unwrap_or(Self::DEFAULT_LIMIT)
            .min(Self::MAX_LIMIT)
    }
}

/// 事件查询结果信封。
#[derive(Debug, Serialize)]
pub struct EventQueryResult {
    /// 命中的事件，按 `seq` 升序。
    pub events: Vec<Event>,
    /// 翻页游标：有下一页时等于本页最后一条的 `seq`，可直接作为下次请求的
    /// `since_seq`；没有更多数据时为 `null`。
    pub next_since_seq: Option<i64>,
}

impl EventQueryResult {
    /// 序列化为 JSON 文本。
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|err| CoreError::Internal(err.to_string()))
    }
}
