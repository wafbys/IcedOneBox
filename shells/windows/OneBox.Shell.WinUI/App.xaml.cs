using Microsoft.UI.Xaml;
using OneBox.Interop;

namespace OneBox.Shell.WinUI;

/// <summary>
/// 应用入口，持有应用级共享的核实例。
///
/// 核是进程级单例：<see cref="OnLaunched"/> 初始化一次，窗口关闭（当前即应用退出）
/// 时 shutdown + free 一次。所有窗口/页面都从 <see cref="SharedCore"/> 取用同一个
/// 实例，避免每个界面各开一个核。
/// </summary>
public partial class App : Application
{
    private Window? _window;

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
        // 内存库：先跑通界面链路，暂不落盘（持久化留到外衣稳定之后）。
        Core = OneBoxCore.Init("{\"db_path\":\":memory:\"}");

        _window = new MainWindow();
        _window.Closed += OnWindowClosed;
        _window.Activate();
    }

    /// <summary>
    /// 窗口关闭 = 当前应用退出（单窗口）：关闭数据库连接并释放句柄。
    /// 多窗口时不应在此关闭核，届时需改到真正的进程退出时机。
    /// </summary>
    private void OnWindowClosed(object sender, WindowEventArgs args)
    {
        Core.Shutdown();
        Core.Dispose();
    }
}
