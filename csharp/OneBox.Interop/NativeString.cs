using System.Runtime.InteropServices;

namespace OneBox.Interop;

/// <summary>核分配的 UTF-8 字符串的读取与释放。</summary>
internal static class NativeString
{
    /// <summary>
    /// 读取核分配的 UTF-8 字符串并释放其原生内存。
    ///
    /// 这是 <c>Marshal.PtrToStringUTF8</c> + <c>onebox_string_free</c> 的配对封装：
    /// 读出的托管字符串与原生内存解耦，随后立即释放原生内存，避免泄漏。
    /// 零指针返回 <c>null</c>（不调用释放）。
    /// </summary>
    internal static string? ReadAndFree(IntPtr pointer)
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
}
