//! SQLite 事件日志存储。
//!
//! 表结构见 `spec/20-events.md`。核只做追加与按条件读取，不做修改或删除，
//! 保证事件日志的追加写秩序。

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{CoreError, Result};
use crate::event::{Event, EventFilter};

/// 当前事件日志 schema 版本。结构变更时必须递增，并在 `prepare` 中处理迁移。
const SCHEMA_VERSION: i64 = 1;

/// 事件日志存储。
pub struct EventStore {
    conn: Connection,
}

impl EventStore {
    /// 打开（必要时创建）事件日志。`path == ":memory:"` 时使用内存数据库。
    pub fn open(path: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::prepare(&conn)?;
        Ok(Self { conn })
    }

    /// 建表并校验 schema 版本。
    fn prepare(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            r#"
            PRAGMA journal_mode = WAL;
            PRAGMA synchronous = NORMAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS events (
                seq        INTEGER PRIMARY KEY AUTOINCREMENT,
                id         TEXT NOT NULL UNIQUE,
                event_type TEXT NOT NULL,
                source     TEXT,
                ts_millis  INTEGER NOT NULL,
                payload    TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_events_type   ON events(event_type);
            CREATE INDEX IF NOT EXISTS idx_events_source ON events(source);
            "#,
        )?;

        let stored: Option<String> = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'schema_version'",
                [],
                |row| row.get(0),
            )
            .optional()?;

        match stored {
            None => {
                conn.execute(
                    "INSERT INTO meta(key, value) VALUES('schema_version', ?1)",
                    params![SCHEMA_VERSION.to_string()],
                )?;
            }
            Some(text) => {
                let found: i64 = text.parse().unwrap_or(-1);
                if found != SCHEMA_VERSION {
                    return Err(CoreError::Storage(format!(
                        "事件日志 schema 版本不匹配：期望 {SCHEMA_VERSION}，实际 {found}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// 追加一条事件，返回分配到的 `seq`。
    pub fn append(&self, event: &Event) -> Result<i64> {
        let payload = serde_json::to_string(&event.payload)
            .map_err(|err| CoreError::Internal(format!("序列化事件负载失败: {err}")))?;
        self.conn.execute(
            "INSERT INTO events(id, event_type, source, ts_millis, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.id,
                event.event_type,
                event.source,
                event.ts_millis,
                payload
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// 按过滤条件读取事件，按 `seq` 升序。
    ///
    /// 返回 `(events, has_more)`：`events` 最多 `limit` 条；`has_more` 表示
    /// 是否还存在更多事件。内部会多取一行来判断，因此能区分「恰好取满 limit
    /// 条」与「还有下一页」。
    pub fn query(&self, filter: &EventFilter) -> Result<(Vec<Event>, bool)> {
        let limit = filter.effective_limit();
        let mut where_clauses: Vec<&str> = Vec::new();
        let mut binds: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(event_type) = &filter.event_type {
            where_clauses.push("event_type = ?");
            binds.push(Box::new(event_type.clone()));
        }
        if let Some(source) = &filter.source {
            where_clauses.push("source = ?");
            binds.push(Box::new(source.clone()));
        }
        if let Some(since_seq) = filter.since_seq {
            where_clauses.push("seq > ?");
            binds.push(Box::new(since_seq));
        }
        // 多取一行：仅用于判断是否还有下一页，返回前会截断到 limit。
        binds.push(Box::new(i64::from(limit) + 1));

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };
        let sql = format!(
            "SELECT seq, id, event_type, source, ts_millis, payload \
             FROM events{where_sql} ORDER BY seq ASC LIMIT ?"
        );

        let mut statement = self.conn.prepare(&sql)?;
        let rows = statement.query_map(
            rusqlite::params_from_iter(binds.iter().map(|value| &**value)),
            Self::row_to_event,
        )?;

        let mut events = Vec::new();
        for row in rows {
            events.push(row?);
        }

        let has_more = events.len() as u32 > limit;
        if has_more {
            events.truncate(limit as usize);
        }
        Ok((events, has_more))
    }

    /// 返回指定 reducer 的最新快照事件。
    ///
    /// 仅在 `event_type == "core.snapshot"` 的事件中按 `payload.reducer_id` 精确
    /// 匹配，取 `seq` 最大者。以下事件一律**跳过**（不报错）：
    /// 1. `payload` 不是 JSON 对象；
    /// 2. `payload` 缺少 `reducer_id`；
    /// 3. `reducer_id` 不是字符串；
    /// 4. `reducer_id` 与查询值不相等。
    pub fn latest_snapshot(&self, reducer_id: &str) -> Result<Option<Event>> {
        let mut statement = self.conn.prepare(
            "SELECT seq, id, event_type, source, ts_millis, payload \
             FROM events WHERE event_type = 'core.snapshot' ORDER BY seq DESC",
        )?;
        let rows = statement.query_map([], Self::row_to_event)?;
        for row in rows {
            let event = row?;
            if reducer_id_of(&event.payload) == Some(reducer_id) {
                return Ok(Some(event));
            }
        }
        Ok(None)
    }

    /// 把一行结果映射为 [`Event`]。
    fn row_to_event(row: &Row<'_>) -> rusqlite::Result<Event> {
        let payload_text: String = row.get(5)?;
        let payload = serde_json::from_str(&payload_text).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
        })?;
        Ok(Event {
            seq: row.get(0)?,
            id: row.get(1)?,
            event_type: row.get(2)?,
            source: row.get(3)?,
            ts_millis: row.get(4)?,
            payload,
        })
    }
}

/// 从快照 payload 中读取 `reducer_id`。
///
/// 仅当 payload 是 JSON 对象且 `reducer_id` 是字符串时返回其值；其余返回 `None`，
/// 调用方据此跳过该事件（不报错）。
fn reducer_id_of(payload: &serde_json::Value) -> Option<&str> {
    payload
        .as_object()
        .and_then(|object| object.get("reducer_id"))
        .and_then(|value| value.as_str())
}
