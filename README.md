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
│   └── onebox-ffi/            # 唯一 unsafe 层：C ABI，产出 cdylib + staticlib
├── include/                   # cbindgen 生成的 onebox.h（构建时生成）
├── csharp/OneBox.Interop/     # C# 绑定层：唯一的 FFI 声明 + 句柄生命周期
├── examples/c-smoke/          # 最小 C 调用程序
├── examples/csharp-smoke/     # 最小 C# 调用程序（引用 OneBox.Interop）
├── shells/windows/OneBox.Shell.WinUI/  # 非打包 WinUI 3 外衣（最小窗口）
├── tools/                     # 构建/验证脚本
├── Directory.Build.targets    # C# 共用构建片段（复制 onebox.dll）
├── cbindgen.toml
└── Cargo.toml                 # workspace
```

## 构建

前置：Rust 工具链（stable），Windows 上还需要 C 编译器
（VS Build Tools 的 `cl.exe`，或 MinGW 的 `gcc`）。

```powershell
# 构建核（生成 target/debug/onebox.dll、onebox.lib 与 include/onebox.h）
cargo build

# Rust 单元/集成测试
cargo test

# 端到端冒烟：构建核 -> 生成头文件 -> 编译并运行 C 测试
./tools/smoke.ps1

# C# 冒烟（.NET）：构建核 -> 运行控制台程序
./tools/csharp-smoke.ps1

# WinUI 3 外衣（非打包）：构建并启动
dotnet run --project shells/windows/OneBox.Shell.WinUI
```

## 当前状态

开发核。已实现：

- `onebox_core_init` / `onebox_core_shutdown`
- `onebox_id_generate`
- `onebox_event_append` / `onebox_event_query`
- `onebox_event_snapshot_latest`
- 支撑函数：`onebox_last_error` / `onebox_string_free`

持久化：SQLite 事件日志（`rusqlite` bundled，静态编译，无运行时依赖）。

未实现（仅有契约约定）：消息路由、权限、模块生命周期。

## 契约摘要

1. **万物有 ID**：任何实体、事件、消息都有唯一标识，ID 一旦生成永不改变。
2. **一切皆事件**：状态变化只通过事件记录，事件日志是秩序来源。
3. **消息必经核**：模块不直接通信；核只路由，不解析、不判断、不修改。
4. **权限先声明**：模块注册时声明权限，核只做允许或拒绝。
5. **模块有生命周期**：注册、启动、心跳、崩溃、重启、注销，核统一管理。

详见 `spec/`。
