/*
 * IcedOneBox 核最小 C 冒烟测试。
 *
 * 验证第一步目标：
 *   1. 能通过 C ABI 初始化/关闭核；
 *   2. 能生成 ID；
 *   3. SQLite 事件日志能写入并查询；
 *   4. 核心契约「id/seq/时间戳由核生成」在边界上确实生效。
 *
 * 构建方式见同目录 build.ps1（或仓库根 tools/smoke.ps1）。
 */

#include <stdio.h>
#include <string.h>

#include "onebox.h"

static int g_failures = 0;

static void check(int condition, const char *what) {
    if (condition) {
        printf("[ok]   %s\n", what);
    } else {
        printf("[FAIL] %s\n", what);
        g_failures++;
    }
}

int main(void) {
    printf("IcedOneBox C smoke test\n");

    /* 1. 初始化：使用内存数据库，避免污染磁盘。 */
    onebox_core *core = onebox_core_init("{\"db_path\":\":memory:\"}");
    if (core == NULL) {
        printf("[FAIL] onebox_core_init: %s\n", onebox_last_error());
        return 1;
    }
    check(1, "onebox_core_init");

    /* 2. 生成 ID：两次调用应得到不同结果。 */
    char *id1 = onebox_id_generate();
    char *id2 = onebox_id_generate();
    check(id1 != NULL && id2 != NULL && strcmp(id1, id2) != 0,
          "onebox_id_generate 生成不重复的 ID");
    onebox_string_free(id1);
    onebox_string_free(id2);

    /* 3. 追加事件。 */
    onebox_rc rc = onebox_event_append(
        core,
        "{\"event_type\":\"smoke.started\",\"source\":\"c-smoke\",\"payload\":{\"n\":1}}");
    check(rc == ONEBOX_OK, "onebox_event_append");
    if (rc != ONEBOX_OK) {
        printf("       last_error: %s\n", onebox_last_error());
    }

    /* 3b. 调用方提供保留字段 id 必须被拒绝。 */
    rc = onebox_event_append(core, "{\"event_type\":\"bad\",\"id\":\"forged\"}");
    check(rc != ONEBOX_OK, "onebox_event_append 拒绝调用方提供的 id");

    /* 4. 查询事件。 */
    char *json = NULL;
    rc = onebox_event_query(core, "{\"event_type\":\"smoke.started\"}", &json);
    check(rc == ONEBOX_OK && json != NULL, "onebox_event_query");
    if (rc == ONEBOX_OK && json != NULL) {
        printf("       result: %s\n", json);
        check(strstr(json, "smoke.started") != NULL, "查询结果包含事件类型");
        check(strstr(json, "\"seq\":1") != NULL, "事件 seq 从 1 开始");
        onebox_string_free(json);
    } else {
        printf("       last_error: %s\n", onebox_last_error());
    }

    /* 5. 关闭并释放。 */
    check(onebox_core_shutdown(core) == ONEBOX_OK, "onebox_core_shutdown");
    check(onebox_core_free(core) == ONEBOX_OK, "onebox_core_free");

    if (g_failures == 0) {
        printf("全部通过\n");
        return 0;
    }
    printf("失败 %d 项\n", g_failures);
    return 1;
}
