# 20 · 事件契约

> 「一切皆事件」：状态变化只通过事件记录，事件日志是秩序来源。

## 事件字段

| 字段 | 类型 | 由谁生成 | 说明 |
|------|------|----------|------|
| `seq` | i64 | **核** | 全局单调序号，从 1 开始，事件日志的排序主键 |
| `id` | string | **核** | UUID v7，见 `10-ids.md` |
| `event_type` | string | 调用方 | 事件类型，必填且非空白 |
| `source` | string? | 调用方 | 事件来源（模块 ID 或 `"core"`），可选 |
| `ts_millis` | i64 | **核** | 事件发生时间（Unix 毫秒） |
| `payload` | any JSON | 调用方 | 事件负载，可选（缺省 `null`） |

## 追加事件（`onebox_event_append`）

输入 JSON **只允许** `event_type` / `source` / `payload` 三个字段。

**绝对秩序**：调用方若提供 `id`、`seq`、`ts_millis`，核**拒绝**该事件
（返回 `ONEBOX_ERR_INVALID_JSON`），不会静默忽略。核用严格模式解析输入，
任何未知字段都会导致拒绝。

其他规则：

- `event_type` 为空或纯空白 → 拒绝（`ONEBOX_ERR_INVALID_ARGUMENT`）。
- 事件只追加，不修改、不删除。
- `onebox_event_append` 成功时只返回返回码；要拿到核分配的完整事件，
  用 `onebox_event_query` 按条件查回。

## 查询事件（`onebox_event_query`）

过滤 JSON（`filter_json` 可为 NULL 或空串 = 无过滤）字段：

| 字段 | 类型 | 说明 |
|------|------|------|
| `event_type` | string? | 精确匹配事件类型 |
| `source` | string? | 精确匹配事件来源 |
| `since_seq` | i64? | 仅返回 `seq` **严格大于**该值的事件（翻页） |
| `limit` | u32? | 返回条数上限，缺省 100，**必须 ≥ 1**，最大 1000 |

未知字段一律拒绝（`ONEBOX_ERR_INVALID_JSON`）。`limit` 为 `0` 会被拒绝
（`ONEBOX_ERR_INVALID_ARGUMENT`）。`limit` 超过 1000 时按 1000 处理。

结果始终按 `seq` **升序**，输出为信封：

```json
{
  "events": [ { "seq": 1, "id": "...", "event_type": "...", "source": "...", "ts_millis": 0, "payload": null } ],
  "next_since_seq": 1
}
```

- `next_since_seq`：**确实还有更多事件时**等于本页最后一条的 `seq`，可直接作为
  下次请求的 `since_seq`；没有更多数据时为 `null`。
  - 实现说明：核在查询时多取一行（`limit + 1`），据此区分「恰好取满 `limit`
    条」与「还有下一页」；因此恰好取满时 `next_since_seq` 仍为 `null`。

## 持久化表结构

```sql
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE events (
    seq        INTEGER PRIMARY KEY AUTOINCREMENT,
    id         TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    source     TEXT,
    ts_millis  INTEGER NOT NULL,
    payload    TEXT NOT NULL
);

CREATE INDEX idx_events_type   ON events(event_type);
CREATE INDEX idx_events_source ON events(source);
```

- `meta.schema_version` 记录事件日志结构版本；当前为 `1`。
  打开时若版本不匹配，核拒绝启动。
- SQLite 使用 WAL 模式、`synchronous = NORMAL`。
- `payload` 以 JSON 文本存储。
