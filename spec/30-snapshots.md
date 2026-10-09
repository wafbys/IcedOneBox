# 30 · 事件快照

> 状态：**生效**（契约已冻结，`onebox_event_snapshot_latest` 已实现）。

## 定位

快照是「一切皆事件」的延伸。视图的状态由事件推演，但事件越积越多，从头重放的
代价会越来越大。快照把某个视图在某一时刻的**物化状态**作为一个事件写回日志，
使重放可以从最近的快照点继续。

三条边界（不可动摇）：

1. **快照是事件**：存进 `events` 表，`event_type = "core.snapshot"`，遵守事件的
   一切规则（只追加；`id` / `seq` / 时间戳由核生成）。
2. **按 reducer 分片**：payload 携带 `reducer_id`，允许多个视图/归约器各存各的
   快照，互不干扰。
3. **核只存和取，不解析 payload**：核不校验、不合并、不裁剪；`state` 对核完全
   不透明，重放（归约）由外衣实现。

> 命名说明：`core.` 前缀表示「**核定义的契约事件**」（event type 由核的规范占用），
> 不表示「核发出的事件」。快照由外衣产生，`source` 填产生它的外衣模块 ID。

## payload 结构

按契约约定，`core.snapshot` 事件的 `payload` 应是一个 JSON 对象。**核不校验以下
任何字段**；正确性由外衣保证。

| 字段 | 类型 | 约定 | 说明 |
|------|------|------|------|
| `reducer_id` | string | 必填、非空 | 视图/归约器标识；同一 id 的多个快照按 `seq` 取最新 |
| `covers_seq` | i64 | 必填 | 该快照覆盖到的事件 `seq`（含）；重放从此之后继续 |
| `state` | any JSON | 必填、值可为 `null` | 物化状态；核原样存储，不解析 |
| `schema_version` | u32 | 可选 | reducer 自己的状态结构版本，供外衣判断新旧快照是否兼容 |
| `note` | string | 可选 | 备注 |

示例：

```json
{
  "reducer_id": "gallery",
  "covers_seq": 1234,
  "schema_version": 3,
  "state": { "...": "..." }
}
```

## 写入

复用 `onebox_event_append(handle, event_json)`：外衣把 `event_type:"core.snapshot"`
连同上面的 payload 传入即可。

- `core.snapshot` 为**保留事件类型**，专用于快照契约。
- 核**不解析、不校验** payload：`reducer_id` / `covers_seq` / `state` 的取值与
  合法性完全由外衣负责。核只按事件规则追加（`event_type` 非空、`id`/`seq`/时间戳
  由核生成）。
- 核**不**决定何时生成快照（无调度、无阈值）。
- 核**不**删除、不覆盖旧快照；事件只追加。

## 读取：`onebox_event_snapshot_latest`

```c
onebox_rc onebox_event_snapshot_latest(onebox_core *handle,
                                       const char *reducer_id,
                                       char **out_json);
```

语义：返回该 `reducer_id` 下 `seq` 最大的 `core.snapshot` 事件。

- 匹配方式：在 `event_type == "core.snapshot"` 的事件中，按 `payload.reducer_id`
  精确匹配，取 `seq` 最大者。核**只读取 `reducer_id` 这一个字段用于匹配**，绝不
  读取或解释 `state`。
- 以下事件一律**跳过**（不报错，继续找下一条）：
  1. `payload` 不是 JSON 对象；
  2. `payload` 缺少 `reducer_id`；
  3. `reducer_id` 不是字符串；
  4. `reducer_id` 与查询值不相等。
- 成功：`ONEBOX_OK`，`*out_json` 写入该事件的完整记录（字段与
  `onebox_event_query` 结果中的 event 一致：`seq/id/event_type/source/ts_millis/payload`），
  由调用方用 `onebox_string_free()` 释放。
- 无快照：`ONEBOX_ERR_NOT_FOUND`，`*out_json = NULL`（本函数首次启用该返回码）。
  若存在 `core.snapshot` 事件但都不匹配该 `reducer_id`，同样返回 `NOT_FOUND`。
- 参数非法（`handle` 或 `reducer_id` 为空）：`ONEBOX_ERR_INVALID_ARGUMENT`。

> 外衣若写过不合契约的快照，核不会报错，但那样的快照永远不会被任何
> `reducer_id` 检索到。外衣自负其责。

## 重放流程（约定）

外衣恢复视图 `reducer_id` 的标准流程：

1. 取最新快照：`onebox_event_snapshot_latest(handle, reducer_id, &json)`。
   - `ONEBOX_ERR_NOT_FOUND` → 从零开始：`state = 初始态`，`from_seq = 0`。
   - 成功 → `state = payload.state`，`from_seq = payload.covers_seq`；
     外衣自行检查 `payload.schema_version` 是否兼容。
2. 拉取后续事件：`onebox_event_query(handle, {"since_seq": from_seq, "limit": ...})`，
   按 `next_since_seq` 翻页，直到 `next_since_seq` 为 `null`。
3. 逐条归约：按 reducer 规则把事件并入 `state`（核不参与）。
   - **必须跳过 `event_type == "core.snapshot"` 的事件**：事件流里混有快照事件，
     快照不是待归约的「业务事件」。这是 reducer 的责任，核不为其过滤。
4. 需要时写回快照：`state` 稳定后，用 `onebox_event_append` 追加新的
   `core.snapshot`，其 `covers_seq` = 已消费的最后一条业务事件的 `seq`。
5. 重复 1–4；旧快照留在日志中，不再被引用。

示意：

```
snapshot_latest(reducer) ──有──▶ state = payload.state, from = payload.covers_seq
           │                          │
        无 └─▶ state = 初始态, from = 0
                                      ▼
      event_query(since_seq = from) ──▶ 逐条归约（跳过 core.snapshot）
                                      ▼
      append(core.snapshot, covers_seq = 已消费的最后 seq)
```

## 核的边界（不做的事）

- 不解析、不校验、不合并 `state`。
- 不校验 payload 的任何字段（含 `covers_seq` 的上下界）。
- 不生成快照、不调度、不设阈值。
- 不裁剪、不删除历史快照。
- 不决定 reducer 的规则。

## 设计说明（已确认）

- **写入路径复用 `onebox_event_append`**，不新增专用写入 API。
- **检索返回整条事件对象**（含 `seq`/`id`/`ts_millis`），外衣自行拆解 payload。
- **核不校验 `covers_seq`**：payload 对核不透明，核不解析、不校验，正确性由外衣保证。
- **不提供 `snapshot_list`**：历史快照列举等真有需求时再加。
- **跳过 `core.snapshot` 靠 reducer 约定**，不给 `onebox_event_query` 增加排除参数。

## 范围外（暂不提供）

- 历史快照列举（如 `onebox_event_snapshot_list`）。
- 快照的自动生成、压缩或清理。

## 与既有契约的关系

- 「一切皆事件」：快照也是事件，遵守 `20-events.md` 的追加与查询规则。
- 「万物有 ID」：快照事件有核分配的 `id` 与 `seq`。
- 契约 2 的措辞已相应修订（`01-contracts.md`）：核提供快照的存储与检索，
  快照的生成是外衣的责任。
