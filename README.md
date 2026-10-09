# IcedOneBox

统一的数字工作台入口。**稳定的核 + 可替换的外衣。**

- **核**：极小的、与技术无关的规范（`spec/`）及其 Rust 实现。核只做五件事。
- **外衣**：GUI、终端、AI 代理、数据模块等，各自独立，通过 C ABI 调用核。

## 目录结构

```
IcedOneBox/
├── spec/                      # 规范文档：核行为的唯一权威来源（中文）
├── crates/
│   ├── onebox-core/           # 核：纯逻辑，forbid(unsafe_code)，无平台依赖
│   ├── onebox-ffi/            # 核的 C ABI 层（唯一允许核 unsafe），cdylib + staticlib
│   └── onebox-terminal/       # 终端模块（独立于核）：PTY 会话，自己的 C ABI
├── include/                   # cbindgen 生成：onebox.h、onebox_terminal.h
├── csharp/OneBox.Interop/     # C# 绑定层：FFI 声明 + 句柄生命周期（核 + 终端）
├── examples/c-smoke/          # 最小 C 调用程序（核）
├── examples/c-smoke-terminal/ # 最小 C 调用程序（终端）
├── examples/csharp-smoke/     # 最小 C# 调用程序（引用 OneBox.Interop）
├── shells/windows/OneBox.Shell.WinUI/  # 非打包 WinUI 3 外衣（事件列表 + 终端窗口）
├── tools/                     # 构建/验证脚本
├── Directory.Build.targets    # C# 共用构建片段（复制 onebox.dll / onebox_terminal.dll）
├── cbindgen.toml
└── Cargo.toml                 # workspace
```

## 构建

前置：Rust 工具链（stable），Windows 上还需要 C 编译器
（VS Build Tools 的 `cl.exe`，或 MinGW 的 `gcc`）。

```powershell
# 构建核与终端（生成 onebox.dll / onebox_terminal.dll 与两个头文件）
cargo build

# Rust 单元/集成测试（核）
cargo test

# 终端模块的集成测试
cargo test -p onebox-terminal

# 端到端冒烟：构建核与终端 -> 生成头文件 -> 编译并运行 C 测试
./tools/smoke.ps1

# C# 冒烟（.NET）：构建核 -> 运行控制台程序
./tools/csharp-smoke.ps1

# WinUI 3 外衣（非打包）：构建并启动
dotnet run --project shells/windows/OneBox.Shell.WinUI
```

## 当前状态

**核**（`onebox-core` / `onebox-ffi`）已实现：

- `onebox_core_init` / `onebox_core_shutdown`
- `onebox_id_generate`
- `onebox_event_append` / `onebox_event_query`
- `onebox_event_snapshot_latest`
- 支撑函数：`onebox_last_error` / `onebox_string_free`

持久化：SQLite 事件日志（`rusqlite` bundled，静态编译，无运行时依赖）。

**终端模块**（`onebox-terminal`，独立于核）：spawn / write / read / resize /
status / close / free；Windows 上走 ConPTY。

**WinUI 3 外衣**：事件列表与写入、终端窗口（WebView2 + xterm.js）；
核为进程级单例（最后一个窗口关闭才 shutdown）。

未实现（仅有契约约定）：消息路由、权限、模块生命周期。终端模块目前绕过核
（有意暂缓，见 `spec/99-open-questions.md`）。

## 契约摘要

1. **万物有 ID**：任何实体、事件、消息都有唯一标识，ID 一旦生成永不改变。
2. **一切皆事件**：状态变化只通过事件记录，事件日志是秩序来源。
3. **消息必经核**：模块不直接通信；核只路由，不解析、不判断、不修改。
4. **权限先声明**：模块注册时声明权限，核只做允许或拒绝。
5. **模块有生命周期**：注册、启动、心跳、崩溃、重启、注销，核统一管理。

详见 `spec/`。
