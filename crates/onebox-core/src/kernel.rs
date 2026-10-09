//! 核实例（Kernel）与初始化配置。

use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::error::{CoreError, Result};
use crate::event::{Event, EventFilter, EventInput, EventQueryResult};
use crate::id::generate_id;
use crate::store::EventStore;

/// 核初始化配置。
///
/// 启用 `deny_unknown_fields`：出现未知字段即拒绝，避免“悄悄接受但没生效”
/// 的配置。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CoreConfig {
    /// 事件日志数据库路径。
    ///
    /// 缺省为当前工作目录下的 `onebox.db`；`:memory:` 表示使用内存数据库
    /// （测试用，进程退出即丢失）。
    #[serde(default)]
    pub db_path: Option<String>,
}

impl CoreConfig {
    /// 缺省数据库文件名。
    pub const DEFAULT_DB_PATH: &'static str = "onebox.db";

    /// 从 JSON 文本解析配置。`None` 或空串表示全部使用默认值。
    pub fn from_json(json: Option<&str>) -> Result<Self> {
        match json {
            None => Ok(Self::default()),
            Some(text) if text.trim().is_empty() => Ok(Self::default()),
            Some(text) => {
                serde_json::from_str(text).map_err(|err| CoreError::InvalidJson(err.to_string()))
            }
        }
    }

    /// 解析后的数据库路径。
    pub fn db_path(&self) -> &str {
        self.db_path.as_deref().unwrap_or(Self::DEFAULT_DB_PATH)
    }
}

/// 核实例。核的全部可变状态都在这里。
pub struct Core {
    store: Option<EventStore>,
}

impl Core {
    /// 初始化核：打开事件日志并校验 schema 版本。
    pub fn init(config: CoreConfig) -> Result<Self> {
        let store = EventStore::open(config.db_path())?;
        Ok(Self { store: Some(store) })
    }

    /// 关闭核：释放数据库连接。可重复调用。
    pub fn shutdown(&mut self) {
        self.store = None;
    }

    /// 生成一个新的 ID。
    pub fn generate_id(&self) -> String {
        generate_id()
    }

    /// 追加一条事件。
    ///
    /// 契约「一切皆事件」+「万物有 ID」：`id`、`seq`、`ts_millis` 一律由核
    /// 生成；调用方若在 JSON 中提供这些字段会被拒绝（见 [`EventInput`]）。
    pub fn append_event(&mut self, input_json: &str) -> Result<Event> {
        let input = EventInput::from_json(input_json)?;
        let event = Event {
            seq: 0, // 占位；落库后由存储层回填真实序号。
            id: generate_id(),
            event_type: input.event_type,
            source: input.source,
            ts_millis: now_millis()?,
            payload: input.payload.unwrap_or(serde_json::Value::Null),
        };
        let seq = self.store_mut()?.append(&event)?;
        Ok(Event { seq, ..event })
    }

    /// 查询事件，返回 JSON 文本。
    pub fn query_events(&self, filter_json: Option<&str>) -> Result<String> {
        let filter = EventFilter::from_json(filter_json)?;
        let (events, has_more) = self.store()?.query(&filter)?;
        // 仅当确实还有更多事件时才给出翻页游标；恰好取满 limit 条时为 null。
        let next_since_seq = if has_more {
            events.last().map(|event| event.seq)
        } else {
            None
        };
        EventQueryResult {
            events,
            next_since_seq,
        }
        .to_json()
    }

    fn store(&self) -> Result<&EventStore> {
        self.store
            .as_ref()
            .ok_or_else(|| CoreError::Internal("核已关闭".to_string()))
    }

    fn store_mut(&mut self) -> Result<&mut EventStore> {
        self.store
            .as_mut()
            .ok_or_else(|| CoreError::Internal("核已关闭".to_string()))
    }
}

/// 当前 Unix 毫秒时间戳。
fn now_millis() -> Result<i64> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| CoreError::Internal(format!("系统时间早于 Unix 纪元: {err}")))?;
    Ok(duration.as_millis() as i64)
}
