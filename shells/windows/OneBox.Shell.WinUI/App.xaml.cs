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
        // 内存库：先跑通界面链路，暂不落盘（持久化留到外衣稳定之后）。
        Core = OneBoxCore.Init("{\"db_path\":\":memory:\"}");
        TrackWindow(new MainWindow());
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
