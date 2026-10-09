# 90 · C ABI 约定

## 句柄

- 核句柄对 C 侧是不透明类型 `onebox_core`，只能持有 `onebox_core *`。
- `onebox_core_init()` 成功返回非空句柄，失败返回 `NULL`（详情看
  `onebox_last_error()`）。
- `onebox_core_shutdown()` 释放资源（关闭数据库），句柄本身仍有效。
- `onebox_core_free()` 释放句柄内存。传入 NULL 安全；同一句柄只能释放一次。
- 典型顺序：`init` → 使用 → `shutdown` → `free`。

## 返回码

除返回指针的函数外，所有函数返回 `onebox_rc`：

| 值 | 名称 | 含义 |
|----|------|------|
| 0 | `ONEBOX_OK` | 成功 |
| 1 | `ONEBOX_ERR_INVALID_ARGUMENT` | 参数缺失或非法（空指针、空 `event_type` 等） |
| 2 | `ONEBOX_ERR_INVALID_JSON` | JSON 解析失败，或结构不符合契约 |
| 3 | `ONEBOX_ERR_NOT_FOUND` | 查询目标不存在 |
| 4 | `ONEBOX_ERR_STORAGE` | 存储层失败 |
| 5 | `ONEBOX_ERR_INTERNAL` | 未归类的内部错误 |
| 99 | `ONEBOX_ERR_PANIC` | FFI 边界捕获到 panic |

## 错误详情

`onebox_last_error()` 返回当前线程最后一次错误的文本：

- 返回 `const char *`，**不要释放**。
- 在下一次调用任何核函数或本函数之前有效。
- 每个线程各自维护，互不影响。

## 内存所有权

| 场景 | 规则 |
|------|------|
| 核返回的字符串（`onebox_id_generate`、`onebox_event_query` 的 `out_json`） | 由核分配，调用方用 `onebox_string_free()` 释放 |
| `onebox_last_error()` 的返回值 | 线程局部缓冲，**不要**释放 |
| 输入字符串（`config_json`、`event_json`、`filter_json`） | 由调用方持有，核只读不改 |

`onebox_string_free(NULL)` 安全；同一指针只能释放一次。

## 并发

- 句柄内部用 `Mutex` 串行化访问，因此**同一个句柄可以被多个线程共享调用**。
- `onebox_last_error()` 是线程局部的。
- 核保持同步，不引入异步运行时。

## panic 与 NUL

- 所有导出的 C 函数都用 `catch_unwind` 包裹，panic 不会穿过 FFI 边界；
  一旦发生，函数返回 `ONEBOX_ERR_PANIC`（或 `NULL`）并记录错误文本。
- 输入必须是合法 UTF-8、以 NUL 结尾的 C 字符串；否则返回
  `ONEBOX_ERR_INVALID_ARGUMENT`。
- 返回的字符串内部不含 NUL。

## 头文件

`include/onebox.h` 由 cbindgen 在构建时自动生成，请勿手动编辑。

## 命名

所有导出函数加 `onebox_` 前缀。所有跨边界数据用 C 兼容类型；
复杂数据统一用 JSON 字符串。
