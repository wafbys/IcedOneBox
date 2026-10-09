/*
 * IcedOneBox 终端模块最小 C 冒烟测试。
 *
 * 验证终端 C ABI：
 *   1. spawn 一个 cmd 会话；
 *   2. 读回 echo 的输出；
 *   3. resize；
 *   4. close / free。
 *
 * 注意：Windows 上终端走 ConPTY，建立后 ConPTY 会发 ESC[6n（DSR）查询光标位置，
 * 消费者必须应答 CPR（ESC[row;colR），否则不会继续输出。这里手工应答，模拟真实
 * 终端（xterm.js）的行为；生产外衣由 xterm.js 自动完成。
 *
 * 构建方式见同目录 build.ps1（或仓库根 tools/smoke.ps1）。
 */

#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <windows.h>

#include "onebox_terminal.h"

static int g_failures = 0;

static void check(int condition, const char *what) {
    if (condition) {
        printf("[ok]   %s\n", what);
    } else {
        printf("[FAIL] %s\n", what);
        g_failures++;
    }
}

/* 在字节流中查找子串（不做 NUL 终止假设）。 */
static int contains_bytes(const unsigned char *haystack, size_t hay_len,
                          const char *needle) {
    size_t needle_len = strlen(needle);
    size_t i;
    if (needle_len == 0 || hay_len < needle_len) {
        return 0;
    }
    for (i = 0; i + needle_len <= hay_len; i++) {
        if (memcmp(haystack + i, needle, needle_len) == 0) {
            return 1;
        }
    }
    return 0;
}

/* 判断一段输出里是否含 ConPTY 的 ESC[6n。 */
static int has_dsr(const unsigned char *data, size_t len) {
    static const unsigned char dsr[4] = {0x1b, '[', '6', 'n'};
    size_t i;
    for (i = 0; i + 4 <= len; i++) {
        if (memcmp(data + i, dsr, 4) == 0) {
            return 1;
        }
    }
    return 0;
}

int main(void) {
    static unsigned char collected[65536];
    size_t total = 0;
    DWORD deadline;
    onebox_terminal *terminal = NULL;
    onebox_terminal_rc rc;

    printf("IcedOneBox terminal C smoke test\n");

    rc = onebox_terminal_spawn(
        "{\"program\":\"cmd.exe\",\"args\":[\"/c\",\"echo terminal-c-hello\"],"
        "\"cols\":80,\"rows\":24}",
        &terminal);
    if (rc != ONEBOX_TERMINAL_OK || terminal == NULL) {
        printf("[FAIL] onebox_terminal_spawn: %s\n", onebox_terminal_last_error());
        return 1;
    }
    check(1, "onebox_terminal_spawn");

    deadline = GetTickCount() + 10000;
    for (;;) {
        unsigned char buffer[4096];
        size_t got = 0;
        int exited = 0;
        int32_t code = -1;

        rc = onebox_terminal_read(terminal, buffer, sizeof(buffer), &got);
        if (rc != ONEBOX_TERMINAL_OK) {
            printf("[FAIL] onebox_terminal_read: %s\n", onebox_terminal_last_error());
            g_failures++;
            break;
        }
        if (got > 0) {
            size_t room = sizeof(collected) - total;
            size_t copy = got < room ? got : room;
            memcpy(collected + total, buffer, copy);
            total += copy;
            if (has_dsr(buffer, got)) {
                const char *cpr = "\x1b[1;1R";
                onebox_terminal_write(terminal, (const uint8_t *)cpr, strlen(cpr));
            }
        }

        onebox_terminal_status(terminal, &exited, &code);
        if (exited && got == 0) {
            break;
        }
        if (GetTickCount() > deadline) {
            break;
        }
        Sleep(20);
    }

    check(contains_bytes(collected, total, "terminal-c-hello"),
          "onebox_terminal_read 回环");
    check(onebox_terminal_resize(terminal, 100, 30) == ONEBOX_TERMINAL_OK,
          "onebox_terminal_resize");
    check(onebox_terminal_close(terminal) == ONEBOX_TERMINAL_OK,
          "onebox_terminal_close");
    check(onebox_terminal_free(terminal) == ONEBOX_TERMINAL_OK,
          "onebox_terminal_free");

    if (g_failures == 0) {
        printf("全部通过\n");
        return 0;
    }
    printf("失败 %d 项\n", g_failures);
    return 1;
}
