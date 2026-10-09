using Microsoft.UI.Xaml;
using OneBox.Interop;

namespace OneBox.Shell.WinUI;

/// <summary>最小窗口：一个按钮，验证 WinUI 3 能启动、能加载核 DLL、能显示 ID。</summary>
public sealed partial class MainWindow : Window
{
    public MainWindow()
    {
        InitializeComponent();
    }

    private void InitButton_Click(object sender, RoutedEventArgs e)
    {
        try
        {
            using OneBoxCore core = OneBoxCore.Init("{\"db_path\":\":memory:\"}");
            string id = core.GenerateId();
            OneboxRc rc = core.Shutdown();
            ResultText.Text = $"核已初始化\nID: {id}\nshutdown: {rc}";
        }
        catch (Exception ex)
        {
            ResultText.Text = $"失败: {ex.Message}";
        }
    }
}
