namespace OneBox.Interop;

/// <summary>与 C 侧 <c>onebox_rc</c> 对应。0 为成功，其余为错误。</summary>
public enum OneboxRc
{
    /// <summary>成功。</summary>
    Ok = 0,

    /// <summary>参数缺失或非法（空指针、空 event_type 等）。</summary>
    InvalidArgument = 1,

    /// <summary>JSON 解析失败，或结构不符合契约（如提供了保留字段）。</summary>
    InvalidJson = 2,

    /// <summary>查询目标不存在（如快照不存在）。</summary>
    NotFound = 3,

    /// <summary>存储层失败。</summary>
    Storage = 4,

    /// <summary>未归类的内部错误。</summary>
    Internal = 5,

    /// <summary>FFI 边界捕获到 panic。</summary>
    Panic = 99,
}
