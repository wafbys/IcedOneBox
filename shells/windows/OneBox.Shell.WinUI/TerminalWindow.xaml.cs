using System.Text;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.Web.WebView2.Core;
using OneBox.Interop;

namespace OneBox.Shell.WinUI;

/// <summary>
/// 终端窗口：一个 WebView2 跑 xterm.js，底下接一个 PTY（默认 PowerShell）。
///
/// 输出用 <see cref="DispatcherQueueTimer"/> 每 30ms 从核外终端模块拉一次，
/// 经 base64 通过 WebView2 消息推给 xterm.js；键盘输入反向走同一条通道。
/// 终端生命周期归本窗口，核实例由 <see cref="App"/> 统一持有。
/// </summary>
public sealed partial class TerminalWindow : Window
{
    /// <summary>本地资源虚拟主机名（映射到输出目录的 Assets/terminal）。</summary>
    private const string VirtualHost = "onebox.terminal";

    /// <summary>每个 tick 最多交给前端的字节数，避免持续输出时饿死 UI。</summary>
    private const int PumpBudgetPerTick = 256 * 1024;

    private readonly byte[] _readBuffer = new byte[8192];

    private TerminalSession? _session;
    private DispatcherQueueTimer? _timer;
    private string? _spawnError;
    private bool _webInitialized;

    public TerminalWindow()
    {
        InitializeComponent();

        try
        {
            _session = TerminalSession.Spawn(null);
        }
        catch (Exception ex)
        {
            _spawnError = ex.Message;
        }

        _timer = DispatcherQueue.GetForCurrentThread().CreateTimer();
        _timer.Interval = TimeSpan.FromMilliseconds(30);
        _timer.Tick += OnTimerTick;

        TerminalWebView.Loaded += OnWebViewLoaded;
        Closed += OnClosed;

        AppWindow.Resize(new Windows.Graphics.SizeInt32(1000, 620));
    }

    private async void OnWebViewLoaded(object sender, RoutedEventArgs e)
    {
        if (_webInitialized)
        {
            return;
        }
        _webInitialized = true;

        try
        {
            // 非打包应用必须显式指定用户数据目录，否则 WebView2 可能因无权写入
            // exe 所在目录而启动失败。
            string userDataFolder = Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
                "IcedOneBox",
                "WebView2");
            Directory.CreateDirectory(userDataFolder);

            CoreWebView2Environment environment =
                await CoreWebView2Environment.CreateWithOptionsAsync(null, userDataFolder, null);
            await TerminalWebView.EnsureCoreWebView2Async(environment);

            CoreWebView2 core = TerminalWebView.CoreWebView2;
            string assetsFolder = Path.Combine(AppContext.BaseDirectory, "WebAssets", "terminal");
            core.SetVirtualHostNameToFolderMapping(
                VirtualHost, assetsFolder, CoreWebView2HostResourceAccessKind.Allow);
            core.WebMessageReceived += OnWebMessage;
            core.Navigate($"https://{VirtualHost}/index.html");
        }
        catch (Exception ex)
        {
            Title = $"终端初始化失败: {ex.Message}";
        }
    }

    private void OnWebMessage(CoreWebView2 sender, CoreWebView2WebMessageReceivedEventArgs args)
    {
        try
        {
            using JsonDocument document = JsonDocument.Parse(args.WebMessageAsJson);
            JsonElement root = document.RootElement;
            string? type = root.TryGetProperty("type", out JsonElement typeElement)
                ? typeElement.GetString()
                : null;

            switch (type)
            {
                case "ready":
                    _timer?.Start();
                    break;

                case "input" when _session is not null
                    && root.TryGetProperty("data", out JsonElement dataElement)
                    && dataElement.GetString() is string base64Input:
                    _session.Write(Convert.FromBase64String(base64Input));
                    break;

                case "resize" when _session is not null
                    && root.TryGetProperty("cols", out JsonElement colsElement)
                    && root.TryGetProperty("rows", out JsonElement rowsElement):
                    int cols = colsElement.GetInt32();
                    int rows = rowsElement.GetInt32();
                    if (cols is > 0 and <= ushort.MaxValue && rows is > 0 and <= ushort.MaxValue)
                    {
                        _session.Resize((ushort)cols, (ushort)rows);
                    }
                    break;
            }
        }
        catch (Exception)
        {
            // WebView2 内容视为不可信输入：非法消息直接丢弃。
        }
    }

    private void OnTimerTick(DispatcherQueueTimer sender, object args)
    {
        CoreWebView2? core = TerminalWebView.CoreWebView2;
        if (core is null)
        {
            return;
        }

        if (_session is null)
        {
            _timer?.Stop();
            if (_spawnError is not null)
            {
                PostOutput(core, Encoding.UTF8.GetBytes($"\r\n[启动失败] {_spawnError}\r\n"));
            }
            return;
        }

        int budget = PumpBudgetPerTick;
        while (budget > 0)
        {
            int count = _session.Read(_readBuffer);
            if (count == 0)
            {
                break;
            }
            budget -= count;
            PostOutput(core, _readBuffer, count);
        }

        if (_session.TryGetExit(out int exitCode))
        {
            _timer?.Stop();
            core.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "exit", code = exitCode }));
        }
    }

    private static void PostOutput(CoreWebView2 core, byte[] data, int length)
    {
        string base64 = Convert.ToBase64String(data, 0, length);
        core.PostWebMessageAsJson(JsonSerializer.Serialize(new { type = "output", data = base64 }));
    }

    private static void PostOutput(CoreWebView2 core, byte[] data) => PostOutput(core, data, data.Length);

    private void OnClosed(object sender, WindowEventArgs args)
    {
        _timer?.Stop();
        _timer = null;

        _session?.Shutdown();
        _session?.Dispose();
        _session = null;

        try
        {
            TerminalWebView.Close();
        }
        catch (Exception)
        {
            // 关闭顺序无碍；忽略。
        }
    }
}
