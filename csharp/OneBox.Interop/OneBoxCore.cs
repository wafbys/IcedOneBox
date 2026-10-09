using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace OneBox.Interop;

/// <summary>
/// 核句柄的托管包装：负责生命周期（init / shutdown / free）与返回字符串的内存。
///
/// 句柄通过 <see cref="System.Runtime.InteropServices.SafeHandle"/> 管理：
/// <see cref="Dispose"/>（或 <c>using</c>）时调用 <c>onebox_core_free</c> 释放原生句柄。
/// 即使未显式 <see cref="Shutdown"/>，释放时也会关闭数据库连接。
/// </summary>
public sealed class OneBoxCore : SafeHandleZeroOrMinusOneIsInvalid
{
    private OneBoxCore()
        : base(ownsHandle: true)
    {
    }

    /// <summary>
    /// 初始化核。失败时抛出 <see cref="InvalidOperationException"/>（消息为核的
    /// 错误详情）。返回的实例应用 <c>using</c> 或手动 <see cref="Dispose"/>。
    /// </summary>
    public static OneBoxCore Init(string? configJson)
    {
        IntPtr handle = NativeCore.onebox_core_init(configJson);
        if (handle == IntPtr.Zero)
        {
            throw new InvalidOperationException($"onebox_core_init 失败: {LastError}");
        }

        OneBoxCore core = new();
        core.SetHandle(handle);
        return core;
    }

    /// <summary>当前线程最后一次错误的文本（线程局部，无需释放）。</summary>
    public static string LastError =>
        Marshal.PtrToStringUTF8(NativeCore.onebox_last_error()) ?? string.Empty;

    /// <summary>生成一个新的 ID（UUID v7）。</summary>
    public string GenerateId()
    {
        ThrowIfDisposed();
        return NativeString.ReadAndFree(NativeCore.onebox_id_generate())
            ?? throw new InvalidOperationException($"onebox_id_generate 返回空: {LastError}");
    }

    /// <summary>追加一条事件。`id` / `seq` / 时间戳由核生成。</summary>
    public OneboxRc AppendEvent(string eventJson)
    {
        ThrowIfDisposed();
        return NativeCore.onebox_event_append(handle, eventJson);
    }

    /// <summary>
    /// 查询事件。成功时 <paramref name="resultJson"/> 为结果 JSON；否则为 <c>null</c>。
    /// </summary>
    public OneboxRc QueryEvents(string? filterJson, out string? resultJson)
    {
        ThrowIfDisposed();
        OneboxRc rc = NativeCore.onebox_event_query(handle, filterJson, out IntPtr pointer);
        resultJson = rc == OneboxRc.Ok ? NativeString.ReadAndFree(pointer) : null;
        return rc;
    }

    /// <summary>
    /// 查询指定 reducer 的最新快照。成功时 <paramref name="resultJson"/> 为该事件的
    /// 完整 JSON；无快照返回 <see cref="OneboxRc.NotFound"/>。
    /// </summary>
    public OneboxRc SnapshotLatest(string reducerId, out string? resultJson)
    {
        ThrowIfDisposed();
        OneboxRc rc = NativeCore.onebox_event_snapshot_latest(handle, reducerId, out IntPtr pointer);
        resultJson = rc == OneboxRc.Ok ? NativeString.ReadAndFree(pointer) : null;
        return rc;
    }

    /// <summary>
    /// 关闭核（释放数据库连接）。可重复调用；句柄本身仍有效，直到 <see cref="Dispose"/>。
    /// </summary>
    public OneboxRc Shutdown()
    {
        ThrowIfDisposed();
        return NativeCore.onebox_core_shutdown(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeCore.onebox_core_free(handle);
        return true;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(IsClosed, this);
    }
}
