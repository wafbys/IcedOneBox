// IcedOneBox 核最小 C# 冒烟测试。
//
// 使用 OneBox.Interop 类库（FFI 声明与句柄生命周期都封在那里），验证：
// C ABI 在 .NET 里可调用，行为与 C 侧一致。

using OneBox.Interop;

namespace IcedOneBox.Smoke;

internal static class Program
{
    private static int _failures;

    private static int Main()
    {
        Console.WriteLine("IcedOneBox C# smoke test");

        // 1. 初始化：内存数据库，避免污染磁盘。
        OneBoxCore core;
        try
        {
            core = OneBoxCore.Init("{\"db_path\":\":memory:\"}");
        }
        catch (Exception ex)
        {
            Console.WriteLine($"[FAIL] onebox_core_init: {ex.Message}");
            return 1;
        }

        using (core)
        {
            Check(true, "onebox_core_init");

            // 2. 生成 ID：两次调用应得到不同结果。
            string id1 = core.GenerateId();
            string id2 = core.GenerateId();
            Check(!string.IsNullOrEmpty(id1) && id1 != id2, "onebox_id_generate 生成不重复的 ID");
            Console.WriteLine($"       id = {id1}");

            // 3. 追加事件。
            OneboxRc rc = core.AppendEvent(
                "{\"event_type\":\"smoke.started\",\"source\":\"csharp-smoke\",\"payload\":{\"n\":1}}");
            Check(rc == OneboxRc.Ok, "onebox_event_append");
            if (rc != OneboxRc.Ok)
            {
                Console.WriteLine($"       last_error: {OneBoxCore.LastError}");
            }

            // 3b. 调用方提供保留字段 id 必须被拒绝（返回 InvalidJson，与 C 侧一致）。
            rc = core.AppendEvent("{\"event_type\":\"bad\",\"id\":\"forged\"}");
            Check(rc == OneboxRc.InvalidJson, $"onebox_event_append 拒绝调用方提供的 id（返回 {(int)rc}）");

            // 4. 查询事件。
            rc = core.QueryEvents("{\"event_type\":\"smoke.started\"}", out string? json);
            Check(rc == OneboxRc.Ok && json is not null, "onebox_event_query");
            if (json is not null)
            {
                Console.WriteLine($"       result: {json}");
                Check(json.Contains("smoke.started"), "查询结果包含事件类型");
                Check(json.Contains("\"seq\":1"), "事件 seq 从 1 开始");
            }
            else
            {
                Console.WriteLine($"       last_error: {OneBoxCore.LastError}");
            }

            // 5. 快照链路：写一条 core.snapshot -> 查最新快照 -> 校验 payload。
            rc = core.AppendEvent(
                "{\"event_type\":\"core.snapshot\",\"source\":\"csharp-smoke\",\"payload\":{\"reducer_id\":\"demo\",\"covers_seq\":1,\"state\":{\"count\":7}}}");
            Check(rc == OneboxRc.Ok, "onebox_event_append 写入快照事件");

            rc = core.SnapshotLatest("demo", out string? snapshot);
            Check(rc == OneboxRc.Ok && snapshot is not null, "onebox_event_snapshot_latest");
            if (snapshot is not null)
            {
                Console.WriteLine($"       snapshot: {snapshot}");
                Check(snapshot.Contains("\"reducer_id\":\"demo\""), "快照含 reducer_id");
                Check(snapshot.Contains("\"count\":7"), "快照 state 正确");
            }

            // 5b. 无快照的 reducer 必须返回 NOT_FOUND。
            rc = core.SnapshotLatest("nope", out string? missing);
            Check(rc == OneboxRc.NotFound && missing is null, $"无快照返回 NOT_FOUND（返回 {(int)rc}）");

            // 6. 关闭。
            Check(core.Shutdown() == OneboxRc.Ok, "onebox_core_shutdown");
        }
        Check(true, "onebox_core_free（Dispose 释放句柄）");

        if (_failures == 0)
        {
            Console.WriteLine("全部通过");
            return 0;
        }
        Console.WriteLine($"失败 {_failures} 项");
        return 1;
    }

    private static void Check(bool condition, string what)
    {
        if (condition)
        {
            Console.WriteLine($"[ok]   {what}");
        }
        else
        {
            Console.WriteLine($"[FAIL] {what}");
            _failures++;
        }
    }
}
