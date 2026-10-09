using System.Text.Json;

namespace OneBox.Shell.WinUI;

/// <summary>
/// 事件列表项的展示模型。只读快照：刷新时整体重建，因此不需要属性变更通知。
/// </summary>
public sealed class EventViewModel
{
    /// <summary>payload 预览的最大字符数（超过则截断为单行）。</summary>
    private const int PayloadPreviewMaxLength = 120;

    /// <summary>全局单调序号，由核分配。</summary>
    public required long Seq { get; init; }

    /// <summary>事件类型。</summary>
    public required string EventType { get; init; }

    /// <summary>事件来源；核中为可选，这里缺省显示为 <c>core</c> 以外的占位。</summary>
    public required string Source { get; init; }

    /// <summary>事件时间（本地时区，已格式化）。</summary>
    public required string Timestamp { get; init; }

    /// <summary>payload 的单行预览（已截断）。</summary>
    public required string PayloadPreview { get; init; }

    /// <summary>序号展示文本，如 <c>#12</c>。</summary>
    public string SeqLabel => $"#{Seq}";

    /// <summary>来源 + payload 的合并展示行。</summary>
    public string DetailLine => $"source: {Source}    payload: {PayloadPreview}";

    /// <summary>从 <c>onebox_event_query</c> 结果中的单条事件 JSON 构造。</summary>
    public static EventViewModel FromJson(JsonElement element)
    {
        long seq = element.GetProperty("seq").GetInt64();
        string eventType = element.GetProperty("event_type").GetString() ?? string.Empty;

        string? source = element.TryGetProperty("source", out JsonElement sourceElement)
            && sourceElement.ValueKind == JsonValueKind.String
                ? sourceElement.GetString()
                : null;

        long tsMillis = element.GetProperty("ts_millis").GetInt64();

        JsonElement payload = element.TryGetProperty("payload", out JsonElement payloadElement)
            ? payloadElement
            : default;

        return new EventViewModel
        {
            Seq = seq,
            EventType = eventType,
            Source = source ?? "(无)",
            Timestamp = DateTimeOffset.FromUnixTimeMilliseconds(tsMillis)
                .ToLocalTime()
                .ToString("yyyy-MM-dd HH:mm:ss"),
            PayloadPreview = FormatPayload(payload),
        };
    }

    private static string FormatPayload(JsonElement payload)
    {
        if (payload.ValueKind is JsonValueKind.Undefined or JsonValueKind.Null)
        {
            return "null";
        }

        string text = payload.GetRawText().ReplaceLineEndings(" ").Trim();
        return text.Length <= PayloadPreviewMaxLength
            ? text
            : text[..PayloadPreviewMaxLength] + "…";
    }
}
