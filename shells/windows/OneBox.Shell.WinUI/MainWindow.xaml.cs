using System.Collections.ObjectModel;
using System.Text.Json;
using System.Text.Json.Nodes;
using Microsoft.UI.Xaml;
using OneBox.Interop;

namespace OneBox.Shell.WinUI;

/// <summary>
/// 主窗口：上半部分追加事件，下半部分展示事件列表。
/// 核实例来自 <see cref="App.SharedCore"/>（应用级单例），本窗口只使用不负责生命周期。
/// </summary>
public sealed partial class MainWindow : Window
{
    /// <summary>本外衣写入事件时使用的 source。</summary>
    private const string Source = "shell.winui";

    /// <summary>列表最多展示的事件条数。</summary>
    private const int MaxDisplayed = 50;

    /// <summary>
    /// 查询上限。核只支持按 <c>seq</c> 升序 + 前向翻页，没有「取最新」接口；
    /// 这里一次取到上限再截末尾 <see cref="MaxDisplayed"/> 条，保证刚追加的事件可见。
    /// 事件超过 1000 条时需要真正的反向分页（后续再补）。
    /// </summary>
    private const int QueryLimit = 1000;

    private readonly ObservableCollection<EventViewModel> _events = new();

    public MainWindow()
    {
        InitializeComponent();
        EventList.ItemsSource = _events;
        RootGrid.Loaded += OnRootGridLoaded;
    }

    /// <summary>窗口加载时拉取一次事件。</summary>
    private void OnRootGridLoaded(object sender, RoutedEventArgs e)
    {
        RefreshEvents();
    }

    private void RefreshButton_Click(object sender, RoutedEventArgs e)
    {
        RefreshEvents();
    }

    private void AppendButton_Click(object sender, RoutedEventArgs e)
    {
        string eventType = EventTypeBox.Text.Trim();
        if (eventType.Length == 0)
        {
            SetStatus("事件类型不能为空");
            return;
        }

        JsonObject evt = new()
        {
            ["event_type"] = eventType,
            ["source"] = Source,
        };

        string payloadText = PayloadBox.Text.Trim();
        if (payloadText.Length > 0)
        {
            if (!TryParsePayload(payloadText, out JsonNode? payload, out string? error))
            {
                SetStatus($"payload 不是合法 JSON：{error}");
                return;
            }
            evt["payload"] = payload;
        }

        OneboxRc rc = App.SharedCore.AppendEvent(evt.ToJsonString());
        if (rc != OneboxRc.Ok)
        {
            SetStatus($"追加失败（{rc}）：{OneBoxCore.LastError}");
            return;
        }

        SetStatus($"已追加 {eventType}");
        RefreshEvents();
    }

    /// <summary>查询最近的事件并重建列表。</summary>
    private void RefreshEvents()
    {
        OneboxRc rc = App.SharedCore.QueryEvents($"{{\"limit\":{QueryLimit}}}", out string? json);
        if (rc != OneboxRc.Ok || json is null)
        {
            SetStatus($"查询失败（{rc}）：{OneBoxCore.LastError}");
            return;
        }

        _events.Clear();
        using (JsonDocument doc = JsonDocument.Parse(json))
        {
            if (doc.RootElement.TryGetProperty("events", out JsonElement events))
            {
                List<EventViewModel> all = new();
                foreach (JsonElement item in events.EnumerateArray())
                {
                    all.Add(EventViewModel.FromJson(item));
                }
                foreach (EventViewModel view in all.TakeLast(MaxDisplayed))
                {
                    _events.Add(view);
                }
            }
        }

        bool empty = _events.Count == 0;
        EmptyHint.Visibility = empty ? Visibility.Visible : Visibility.Collapsed;
        EventList.Visibility = empty ? Visibility.Collapsed : Visibility.Visible;
        if (empty)
        {
            SetStatus("暂无事件");
        }
    }

    private static bool TryParsePayload(string text, out JsonNode? payload, out string? error)
    {
        try
        {
            payload = JsonNode.Parse(text);
            error = null;
            return true;
        }
        catch (JsonException ex)
        {
            payload = null;
            error = ex.Message;
            return false;
        }
    }

    private void SetStatus(string message)
    {
        StatusText.Text = message;
    }
}
