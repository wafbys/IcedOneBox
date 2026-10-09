// IcedOneBox 核最小 C# 冒烟测试。
//
// 验证：C ABI 在 .NET 里可调用，行为与 C 侧一致；核分配的字符串被正确释放。
//
// 要点：
// - 用 [LibraryImport]（源生成 P/Invoke）+ StringMarshalling.Utf8，不用 [DllImport]。
// - 读回字符串用 Marshal.PtrToStringUTF8（核返回 UTF-8），不用 PtrToStringAnsi。
// - 核返回的字符串一律用 onebox_string_free 释放（内存泄漏高危区）。

using System.Runtime.InteropServices;

namespace IcedOneBox.Smoke;

/// <summary>核的 P/Invoke 声明。</summary>
internal static partial class NativeCore
{
    private const string Library = "onebox";

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr onebox_core_init(string? configJson);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_core_shutdown(IntPtr handle);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_core_free(IntPtr handle);

    [LibraryImport(Library)]
    internal static partial IntPtr onebox_id_generate();

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial OneboxRc onebox_event_append(IntPtr handle, string eventJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial OneboxRc onebox_event_query(IntPtr handle, string? filterJson, out IntPtr outJson);

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial OneboxRc onebox_event_snapshot_latest(IntPtr handle, string reducerId, out IntPtr outJson);

    [LibraryImport(Library)]
    internal static partial void onebox_string_free(IntPtr value);

    /// <summary>返回线程局部指针，归核所有，<b>不要</b>释放。</summary>
    [LibraryImport(Library)]
    internal static partial IntPtr onebox_last_error();
}

/// <summary>与 C 侧 <c>onebox_rc</c> 对应。</summary>
internal enum OneboxRc
{
    Ok = 0,
    InvalidArgument = 1,
    InvalidJson = 2,
    NotFound = 3,
    Storage = 4,
    Internal = 5,
    Panic = 99,
}

internal static class Program
{
    private static int _failures;

    private static int Main()
    {
        Console.WriteLine("IcedOneBox C# smoke test");

        // 1. 初始化：内存数据库，避免污染磁盘。
        IntPtr core = NativeCore.onebox_core_init("{\"db_path\":\":memory:\"}");
        if (core == IntPtr.Zero)
        {
            Console.WriteLine($"[FAIL] onebox_core_init: {LastError()}");
            return 1;
        }
        Check(true, "onebox_core_init");

        // 2. 生成 ID：两次调用应得到不同结果。
        string? id1 = ReadNativeString(NativeCore.onebox_id_generate());
        string? id2 = ReadNativeString(NativeCore.onebox_id_generate());
        Check(id1 is not null && id2 is not null && id1 != id2, "onebox_id_generate 生成不重复的 ID");
        Console.WriteLine($"       id = {id1}");

        // 3. 追加事件。
        OneboxRc rc = NativeCore.onebox_event_append(
            core,
            "{\"event_type\":\"smoke.started\",\"source\":\"csharp-smoke\",\"payload\":{\"n\":1}}");
        Check(rc == OneboxRc.Ok, "onebox_event_append");
        if (rc != OneboxRc.Ok)
        {
            Console.WriteLine($"       last_error: {LastError()}");
        }

        // 3b. 调用方提供保留字段 id 必须被拒绝（具体返回 InvalidJson，与 C 侧一致）。
        rc = NativeCore.onebox_event_append(core, "{\"event_type\":\"bad\",\"id\":\"forged\"}");
        Check(rc == OneboxRc.InvalidJson, $"onebox_event_append 拒绝调用方提供的 id（返回 {(int)rc}）");

        // 4. 查询事件。
        rc = NativeCore.onebox_event_query(
            core, "{\"event_type\":\"smoke.started\"}", out IntPtr jsonPtr);
        Check(rc == OneboxRc.Ok && jsonPtr != IntPtr.Zero, "onebox_event_query");
        if (rc == OneboxRc.Ok && jsonPtr != IntPtr.Zero)
        {
            string? json = ReadNativeString(jsonPtr);
            Console.WriteLine($"       result: {json}");
            Check(json is not null && json.Contains("smoke.started"), "查询结果包含事件类型");
            Check(json is not null && json.Contains("\"seq\":1"), "事件 seq 从 1 开始");
        }
        else
        {
            Console.WriteLine($"       last_error: {LastError()}");
        }

        // 4b. 快照链路：写一条 core.snapshot -> 查最新快照 -> 校验 payload。
        rc = NativeCore.onebox_event_append(
            core,
            "{\"event_type\":\"core.snapshot\",\"source\":\"csharp-smoke\",\"payload\":{\"reducer_id\":\"demo\",\"covers_seq\":1,\"state\":{\"count\":7}}}");
        Check(rc == OneboxRc.Ok, "onebox_event_append 写入快照事件");

        rc = NativeCore.onebox_event_snapshot_latest(core, "demo", out IntPtr snapPtr);
        Check(rc == OneboxRc.Ok && snapPtr != IntPtr.Zero, "onebox_event_snapshot_latest");
        if (rc == OneboxRc.Ok && snapPtr != IntPtr.Zero)
        {
            string? snapshot = ReadNativeString(snapPtr);
            Console.WriteLine($"       snapshot: {snapshot}");
            Check(snapshot is not null && snapshot.Contains("\"reducer_id\":\"demo\""), "快照含 reducer_id");
            Check(snapshot is not null && snapshot.Contains("\"count\":7"), "快照 state 正确");
        }
        else
        {
            Console.WriteLine($"       last_error: {LastError()}");
        }

        // 4c. 无快照的 reducer 必须返回 NOT_FOUND。
        rc = NativeCore.onebox_event_snapshot_latest(core, "nope", out IntPtr missingPtr);
        Check(rc == OneboxRc.NotFound && missingPtr == IntPtr.Zero,
            $"无快照返回 NOT_FOUND（返回 {(int)rc}）");

        // 5. 关闭并释放。
        Check(NativeCore.onebox_core_shutdown(core) == OneboxRc.Ok, "onebox_core_shutdown");
        Check(NativeCore.onebox_core_free(core) == OneboxRc.Ok, "onebox_core_free");

        if (_failures == 0)
        {
            Console.WriteLine("全部通过");
            return 0;
        }
        Console.WriteLine($"失败 {_failures} 项");
        return 1;
    }

    /// <summary>
    /// 读取核分配的 UTF-8 字符串，并释放其原生内存。
    /// 零指针返回 <c>null</c>。
    /// </summary>
    private static string? ReadNativeString(IntPtr pointer)
    {
        if (pointer == IntPtr.Zero)
        {
            return null;
        }
        try
        {
            return Marshal.PtrToStringUTF8(pointer);
        }
        finally
        {
            NativeCore.onebox_string_free(pointer);
        }
    }

    /// <summary>读取 onebox_last_error()；该指针归核所有（线程局部），不释放。</summary>
    private static string LastError()
    {
        return Marshal.PtrToStringUTF8(NativeCore.onebox_last_error()) ?? string.Empty;
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
