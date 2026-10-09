using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;

namespace OneBox.Interop;

/// <summary>
/// 终端会话的托管包装：负责生命周期（spawn / close / free）与 PTY 读写。
///
/// 句柄通过 <see cref="SafeHandle"/> 管理：<see cref="Dispose"/>（或 <c>using</c>）
/// 时调用 <c>onebox_terminal_free</c> 释放原生句柄；即使未显式 <see cref="Close"/>，
/// 释放时也会终止子进程并关闭 PTY。
///
/// 输出是裸字节流：<see cref="Read"/> 使用调用方提供的缓冲，避免每次分配。
/// </summary>
public sealed class TerminalSession : SafeHandleZeroOrMinusOneIsInvalid
{
    private TerminalSession()
        : base(ownsHandle: true)
    {
    }

    /// <summary>
    /// 启动一个终端会话。<paramref name="configJson"/> 为 <c>null</c> 或空串时
    /// 使用默认配置（<c>powershell.exe</c>，80x24）。失败时抛出
    /// <see cref="InvalidOperationException"/>。
    /// </summary>
    public static TerminalSession Spawn(string? configJson = null)
    {
        OneboxRc rc = NativeTerminal.onebox_terminal_spawn(configJson, out IntPtr handle);
        if (rc != OneboxRc.Ok || handle == IntPtr.Zero)
        {
            throw new InvalidOperationException($"onebox_terminal_spawn 失败（{rc}）: {LastError}");
        }

        TerminalSession session = new();
        session.SetHandle(handle);
        return session;
    }

    /// <summary>当前线程最后一次错误的文本。</summary>
    public static string LastError =>
        Marshal.PtrToStringUTF8(NativeTerminal.onebox_terminal_last_error()) ?? string.Empty;

    /// <summary>向 PTY 写入输入字节（键盘 / 粘贴）。</summary>
    public void Write(byte[] data)
    {
        ThrowIfDisposed();
        if (data.Length == 0)
        {
            return;
        }
        OneboxRc rc = NativeTerminal.onebox_terminal_write(handle, data, (nuint)data.Length);
        if (rc != OneboxRc.Ok)
        {
            throw new IOException($"onebox_terminal_write 失败（{rc}）: {LastError}");
        }
    }

    /// <summary>
    /// 非阻塞取走已就绪的输出，返回实际字节数（无数据时为 0）。
    /// <paramref name="buffer"/> 由调用方复用，避免每帧分配。
    /// </summary>
    public int Read(byte[] buffer)
    {
        ThrowIfDisposed();
        if (buffer.Length == 0)
        {
            return 0;
        }
        OneboxRc rc = NativeTerminal.onebox_terminal_read(
            handle, buffer, (nuint)buffer.Length, out nuint outLen);
        if (rc != OneboxRc.Ok)
        {
            throw new IOException($"onebox_terminal_read 失败（{rc}）: {LastError}");
        }
        return (int)outLen;
    }

    /// <summary>调整 PTY 尺寸（列 / 行）。</summary>
    public void Resize(ushort cols, ushort rows)
    {
        ThrowIfDisposed();
        OneboxRc rc = NativeTerminal.onebox_terminal_resize(handle, cols, rows);
        if (rc != OneboxRc.Ok)
        {
            throw new IOException($"onebox_terminal_resize 失败（{rc}）: {LastError}");
        }
    }

    /// <summary>查询子进程是否已退出；退出时 <paramref name="exitCode"/> 为退出码。</summary>
    public bool TryGetExit(out int exitCode)
    {
        ThrowIfDisposed();
        OneboxRc rc = NativeTerminal.onebox_terminal_status(handle, out int exited, out int code);
        if (rc != OneboxRc.Ok)
        {
            throw new IOException($"onebox_terminal_status 失败（{rc}）: {LastError}");
        }
        exitCode = code;
        return exited != 0;
    }

    /// <summary>
    /// 关闭会话：终止子进程、关闭 PTY。可重复调用；句柄本身仍有效，直到
    /// <see cref="System.Runtime.InteropServices.SafeHandle.Dispose"/> 释放。
    /// </summary>
    public void Shutdown()
    {
        if (IsClosed || IsInvalid)
        {
            return;
        }
        NativeTerminal.onebox_terminal_close(handle);
    }

    protected override bool ReleaseHandle()
    {
        NativeTerminal.onebox_terminal_free(handle);
        return true;
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(IsClosed, this);
    }
}
