# IcedOneBox 规范

> **本目录是核行为的唯一权威来源。**
> 代码与规范冲突时，以规范为准；若规范本身有误，先改规范，再改代码。

## 阅读顺序

| 文件 | 内容 | 状态 |
|------|------|------|
| `00-overview.md` | 理念、平台策略、当前阶段 | 生效 |
| `01-contracts.md` | 五条不可动摇的契约 | 生效 |
| `10-ids.md` | ID 契约（UUID v7） | 生效 |
| `20-events.md` | 事件契约、表结构、查询 | 生效 |
| `90-ffi.md` | C ABI 约定：句柄、返回码、内存、线程 | 生效 |

## 关于“现状”与“目标”

- 只有在本目录明确写下的行为才算契约；其余一律视为未实现。
- 第一步已实现的范围：`onebox_core_init` / `onebox_core_shutdown` /
  `onebox_id_generate` / `onebox_event_append` / `onebox_event_query`，
  外加支撑函数 `onebox_last_error` / `onebox_string_free`。
- 消息路由、权限、模块生命周期三组契约目前**只有约定、没有实现**，
  见 `01-contracts.md` 中的对应章节。
