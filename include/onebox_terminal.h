/* IcedOneBox 终端模块 C ABI 头文件 */

#ifndef ONEBOX_TERMINAL_H
#define ONEBOX_TERMINAL_H

/* 此文件由 cbindgen 自动生成，请勿手动编辑。 */

#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>

/**
 * 默认列数。
 */
#define SpawnConfig_DEFAULT_COLS 80

/**
 * 默认行数。
 */
#define SpawnConfig_DEFAULT_ROWS 24

/**
 * 终端模块返回码。数值与核的 `onebox_rc` 对齐（0/1/2/5/99）；
 * 终端没有存储层，故不含 `4`。
 */
typedef enum onebox_terminal_rc {
    /**
     * 成功。
     */
    ONEBOX_TERMINAL_OK = 0,
    /**
     * 参数缺失或非法（空指针、cols/rows 为 0 等）。
     */
    ONEBOX_TERMINAL_ERR_INVALID_ARGUMENT = 1,
    /**
     * 配置 JSON 解析失败或含未知字段。
     */
    ONEBOX_TERMINAL_ERR_INVALID_JSON = 2,
    /**
     * Spawn / IO 等失败统一归到这里；需要区分时再新增返回码。
     */
    ONEBOX_TERMINAL_ERR_INTERNAL = 5,
    /**
     * FFI 边界捕获到 panic。
     */
    ONEBOX_TERMINAL_ERR_PANIC = 99,
} onebox_terminal_rc;

/**
 * 终端会话句柄（对 C 侧不透明）。
 *
 * 内部用 `Mutex` 串行化访问，因此可被多个线程共享调用。
 */
typedef struct onebox_terminal onebox_terminal;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * 启动一个终端会话。
 *
 * `config_json` 可为 NULL 或空串，表示使用默认配置（`powershell.exe`，80x24）。
 * 成功时把句柄写入 `out_handle`（用 `onebox_terminal_free()` 释放）；失败时
 * `*out_handle` 保持 NULL，详情用 `onebox_terminal_last_error()` 获取。
 *
 * # Safety
 * `config_json` 必须是 NULL 或指向以 NUL 结尾的 UTF-8 JSON 字符串；
 * `out_handle` 必须指向可写的 `onebox_terminal *` 变量。
 */
enum onebox_terminal_rc onebox_terminal_spawn(const char *config_json,
                                              struct onebox_terminal **out_handle);

/**
 * 向终端写入输入字节（键盘 / 粘贴），阻塞写完。
 *
 * # Safety
 * `handle` 必须有效；`data` 必须指向 `len` 字节的可读内存（`len` 为 0 时可为 NULL）。
 */
enum onebox_terminal_rc onebox_terminal_write(struct onebox_terminal *handle,
                                              const uint8_t *data,
                                              size_t len);

/**
 * 非阻塞取走已就绪的输出。
 *
 * 成功时 `*out_len` 为实际写出的字节数（无数据时为 0）。调用方负责提供
 * `cap` 字节的可写缓冲。
 *
 * # Safety
 * `handle` 必须有效；`buf` 必须指向 `cap` 字节的可写内存（`cap` 为 0 时可为
 * NULL）；`out_len` 必须指向可写的 `size_t`。
 */
enum onebox_terminal_rc onebox_terminal_read(struct onebox_terminal *handle,
                                             uint8_t *buf,
                                             size_t cap,
                                             size_t *out_len);

/**
 * 调整 PTY 尺寸（列 / 行）。
 *
 * # Safety
 * `handle` 必须有效。
 */
enum onebox_terminal_rc onebox_terminal_resize(struct onebox_terminal *handle,
                                               uint16_t cols,
                                               uint16_t rows);

/**
 * 查询子进程状态。
 *
 * `*out_exited` 为 1 表示已退出，此时 `*out_code` 为退出码（不可得时为 -1）。
 *
 * # Safety
 * `handle` 必须有效；`out_exited` / `out_code` 必须指向可写内存。
 */
enum onebox_terminal_rc onebox_terminal_status(struct onebox_terminal *handle,
                                               int *out_exited,
                                               int32_t *out_code);

/**
 * 关闭会话：终止子进程、停止读线程、释放 PTY。可重复调用；句柄本身仍有效，
 * 需另用 `onebox_terminal_free()` 释放。
 *
 * # Safety
 * `handle` 必须有效。
 */
enum onebox_terminal_rc onebox_terminal_close(struct onebox_terminal *handle);

/**
 * 释放终端句柄。传入 NULL 安全；同一句柄只能释放一次。
 *
 * # Safety
 * `handle` 必须是 `onebox_terminal_spawn` 返回且尚未释放的指针，或 NULL。
 */
void onebox_terminal_free(struct onebox_terminal *handle);

/**
 * 返回当前线程最后一次错误的文本。
 *
 * 指针指向线程局部缓冲，调用方**不要**释放。
 */
const char *onebox_terminal_last_error(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* ONEBOX_TERMINAL_H */
