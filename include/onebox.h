/* IcedOneBox 核 C ABI 头文件 */

#ifndef ONEBOX_H
#define ONEBOX_H

/* 此文件由 cbindgen 自动生成，请勿手动编辑。 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * 统一返回码。`ONEBOX_OK` 为 0，其余为错误。
 */
typedef enum onebox_rc {
    /**
     * 成功。
     */
    ONEBOX_OK = 0,
    /**
     * 参数缺失或非法（如空指针、空 `event_type`）。
     */
    ONEBOX_ERR_INVALID_ARGUMENT = 1,
    /**
     * JSON 解析失败，或结构不符合契约（如提供了保留字段）。
     */
    ONEBOX_ERR_INVALID_JSON = 2,
    /**
     * 查询目标不存在。
     */
    ONEBOX_ERR_NOT_FOUND = 3,
    /**
     * 存储层失败。
     */
    ONEBOX_ERR_STORAGE = 4,
    /**
     * 未归类的内部错误。
     */
    ONEBOX_ERR_INTERNAL = 5,
    /**
     * FFI 边界捕获到 panic。
     */
    ONEBOX_ERR_PANIC = 99,
} onebox_rc;

/**
 * 核句柄。
 *
 * 对 C 侧是不透明类型：只能持有 `onebox_core *`，无法访问内部字段。
 * 句柄内部用 `Mutex` 串行化访问，因此可以被多个模块线程共享调用。
 */
typedef struct onebox_core onebox_core;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * 初始化核。
 *
 * `config_json` 可为 NULL 或空串，表示使用默认配置。成功返回句柄，失败返回
 * NULL，详情用 `onebox_last_error()` 获取。返回的句柄必须用 `onebox_core_free()`
 * 释放。
 *
 * # Safety
 * `config_json` 必须是 NULL 或指向以 NUL 结尾的 UTF-8 JSON 字符串。
 */
struct onebox_core *onebox_core_init(const char *config_json);

/**
 * 关闭核：释放数据库连接。可重复调用。句柄本身仍然有效，需另用
 * `onebox_core_free()` 释放。
 *
 * # Safety
 * `handle` 必须是 `onebox_core_init` 返回且尚未释放的有效指针。
 */
enum onebox_rc onebox_core_shutdown(struct onebox_core *handle);

/**
 * 释放核句柄。传入 NULL 安全；同一句柄只能释放一次。
 *
 * # Safety
 * `handle` 必须是 `onebox_core_init` 返回且尚未释放的指针，或 NULL。
 */
enum onebox_rc onebox_core_free(struct onebox_core *handle);

/**
 * 生成一个新的 ID。返回的字符串必须用 `onebox_string_free()` 释放。
 */
char *onebox_id_generate(void);

/**
 * 追加一条事件。`id`、`seq`、时间戳由核生成；调用方在 `event_json` 中提供
 * 这些字段会被拒绝。
 *
 * # Safety
 * `handle` 必须有效；`event_json` 必须指向以 NUL 结尾的 UTF-8 JSON 字符串。
 */
enum onebox_rc onebox_event_append(struct onebox_core *handle, const char *event_json);

/**
 * 查询事件。成功时把结果 JSON 写入 `out_json`，调用方必须用
 * `onebox_string_free()` 释放该字符串；失败时 `*out_json` 保持为 NULL。
 *
 * `filter_json` 可为 NULL 或空串，表示无过滤条件。
 *
 * # Safety
 * `handle` 必须有效；`filter_json` 必须是 NULL 或有效的 UTF-8 JSON 字符串；
 * `out_json` 必须指向可写的 `char *` 变量。
 */
enum onebox_rc onebox_event_query(struct onebox_core *handle,
                                  const char *filter_json,
                                  char **out_json);

/**
 * 释放由核分配的字符串。
 *
 * 适用于 `onebox_id_generate()` 与 `onebox_event_query()` 返回的指针。
 * 传入 NULL 安全；同一指针只能释放一次。
 *
 * 与其他核函数一致，本函数会清空当前线程的错误文本；若需读取错误详情，请在
 * 调用本函数之前先调用 `onebox_last_error()`。
 *
 * # Safety
 * `value` 必须是核返回且尚未释放的指针，或 NULL。
 */
void onebox_string_free(char *value);

/**
 * 返回当前线程最后一次错误的文本。
 *
 * 指针指向线程局部缓冲，调用方**不要**释放；在下一次调用其他会写入错误状态的
 * 核函数之前一直有效（注意 `onebox_string_free()` 也会清空错误文本）。
 */
const char *onebox_last_error(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* ONEBOX_H */
