using System.Runtime.InteropServices;

namespace OneBox.Interop;

/// <summary>
/// 终端模块的 C ABI 声明（独立 DLL <c>onebox_terminal</c>）。
///
/// 与核分开：终端不是核的一部分，头文件是 <c>onebox_terminal.h</c>。
/// 返回码复用 <see cref="OneboxRc"/>（<c>onebox_terminal_rc</c> 的数值与核刻意对齐）。
/// 仅供本程序集内部使用；对外请用 <see cref="TerminalSession"/>。
/// </summary>
internal static partial class NativeTerminal
{
    private const string Library = "onebox_terminal";

    [LibraryImport(Library, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial OneboxRc onebox_terminal_spawn(string? configJson, out IntPtr outHandle);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_write(IntPtr handle, byte[] data, nuint len);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_read(IntPtr handle, byte[] buffer, nuint cap, out nuint outLen);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_resize(IntPtr handle, ushort cols, ushort rows);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_status(IntPtr handle, out int outExited, out int outCode);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_close(IntPtr handle);

    [LibraryImport(Library)]
    internal static partial OneboxRc onebox_terminal_free(IntPtr handle);

    /// <summary>返回线程局部指针，归库所有，<b>不要</b>释放。</summary>
    [LibraryImport(Library)]
    internal static partial IntPtr onebox_terminal_last_error();
}
