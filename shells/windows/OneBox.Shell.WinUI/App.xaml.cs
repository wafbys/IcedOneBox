using System.Text.Json;
using Microsoft.UI.Xaml;
using OneBox.Interop;

namespace OneBox.Shell.WinUI;

/// <summary>
/// 应用入口，持有应用级共享的核实例。
///
/// 核是进程级单例：<see cref="OnLaunched"/> 初始化一次，**最后一个窗口关闭**时
/// shutdown + free 一次。所有窗口/页面都从 <see cref="SharedCore"/> 取用同一个
/// 实例，避免每个界面各开一个核。
/// </summary>
public partial class App : Application
{
    private readonly List<Window> _windows = new();

    /// <summary>
    /// 应用级共享的核实例。由 <see cref="OnLaunched"/> 初始化；
    /// 在此之前访问会得到 <c>null</c>。
    /// </summary>
    public OneBoxCore Core { get; private set; } = null!;

    /// <summary>便捷访问：当前应用共享的核实例。</summary>
    public static OneBoxCore SharedCore => ((App)Application.Current).Core;

    public App()
    {
        InitializeComponent();
    }

    protected override void OnLaunched(LaunchActivatedEventArgs args)
    {
        Core = OneBoxCore.Init(BuildCoreConfig());
        TrackWindow(new MainWindow());
    }

    /// <summary>
    /// 构造核配置：事件日志落在 <c>%LocalAppData%\IcedOneBox\onebox.db</c>。
    /// SQLite 的 WAL 模式会在同目录产生 <c>-wal</c> / <c>-shm</c> 文件，属正常现象。
    /// </summary>
    private static string BuildCoreConfig()
    {
        string directory = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "IcedOneBox");
        Directory.CreateDirectory(directory);
        string dbPath = Path.Combine(directory, "onebox.db");
        return JsonSerializer.Serialize(new { db_path = dbPath });
    }

    /// <summary>打开一个新的终端窗口。</summary>
    public void OpenTerminal()
    {
        TrackWindow(new TerminalWindow());
    }

    /// <summary>登记一个窗口并激活；窗口关闭时自动注销。</summary>
    private void TrackWindow(Window window)
    {
        _windows.Add(window);
        window.Closed += (_, _) => OnWindowClosed(window);
        window.Activate();
    }

    /// <summary>注销窗口；最后一个窗口关闭时才关闭核。</summary>
    private void OnWindowClosed(Window window)
    {
        _windows.Remove(window);
        if (_windows.Count > 0)
        {
            return;
        }

        Core.Shutdown();
        Core.Dispose();
    }
}
