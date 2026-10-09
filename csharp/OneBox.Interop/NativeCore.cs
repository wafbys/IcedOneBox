using System.Runtime.InteropServices;

namespace OneBox.Interop;

/// <summary>
/// 核的 C ABI 声明（全仓库唯一一份）。用源生成的 <c>[LibraryImport]</c>，
/// 字符串统一 UTF-8 编组。
///
/// 仅供本程序集内部使用；对外请用 <see cref="OneBoxCore"/>。
/// </summary>
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
