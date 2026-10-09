# AI 协作指南 — IcedOneBox

> 目标：高效交付高质量代码。规则要实用，不是为约束而约束。

## 项目背景

个人自用的统一数字工作台入口。**稳定的核 + 可替换的外衣。**

- 核：与技术无关的规范（`spec/`）+ Rust 实现。核不关心核外世界。
- 外衣：GUI / 终端 / AI / 数据模块，各自独立，通过 C ABI 调核。
- Windows 是主场（核 + WinUI 3），macOS / Linux 是客场（核 + SwiftUI / GTK）。

| 路径 | 角色 |
|------|------|
| `VISION.md` | 愿景与设计品味（为什么做、要成为什么）；**非行为规范** |
| `spec/` | **核行为的唯一权威来源**。代码与规范冲突时以规范为准 |
| `crates/onebox-core` | 核的纯逻辑：`#![forbid(unsafe_code)]`，无平台/GUI 依赖 |
| `crates/onebox-ffi` | 核**唯一允许 unsafe** 的 C ABI 层：类型转换 / panic 拦截 / 错误映射 |
| `crates/onebox-terminal` | 终端模块（独立于核）：PTY 会话管理，自己的 C ABI；unsafe 仅限其 FFI 层 |
| `include/onebox.h` | cbindgen 生成，勿手改 |
| `include/onebox_terminal.h` | cbindgen 生成，勿手改 |
| `examples/c-smoke` | 最小 C 调用程序（FFI 链路验证） |
| `examples/csharp-smoke` | 最小 C# 外衣（.NET，FFI 链路验证） |
| `csharp/OneBox.Interop` | C# 绑定层：唯一 FFI 声明 + SafeHandle 生命周期 |
| `shells/windows/OneBox.Shell.WinUI` | 非打包 WinUI 3 外衣（事件列表 + 终端窗口） |

## 五条契约（不可动摇）

1. 万物有 ID；2. 一切皆事件；3. 消息必经核；4. 权限先声明；5. 模块有生命周期。
详见 `spec/01-contracts.md`。

**核不解析消息内容，不判断对错，不修改消息；不关心模块内部实现。**
任何「也许以后有用」的功能都先不加。

## 构建与验证

```powershell
cargo build           # 生成 dll/lib 与 include/onebox.h、include/onebox_terminal.h
cargo test            # core 的集成测试
cargo test -p onebox-terminal  # 终端模块的集成测试
cargo clippy --all-targets
./tools/smoke.ps1     # 构建 + 生成头文件 + 编译运行 C 测试
./tools/csharp-smoke.ps1  # 构建 + 运行 .NET 控制台冒烟
dotnet run --project shells/windows/OneBox.Shell.WinUI  # 非打包 WinUI 3 外衣
```

改核行为时：`cargo test` 绿 + `./tools/smoke.ps1` 通过。

## 硬性规则

- 规范先行：改行为先改 `spec/`，再改代码；代码注释中文，标识符英文。
- `onebox-core` 内**禁止** unsafe；核的 unsafe 集中在 `onebox-ffi`。
- `onebox-terminal` 的 unsafe 仅限其 FFI 层（`src/lib.rs` / `src/ffi.rs`），
  `src/session.rs` 保持 `#![forbid(unsafe_code)]`。
- 所有导出 C 函数加 `onebox_` 前缀，用 `catch_unwind` 包裹，错误走返回码 +
  `onebox_last_error()`。
- 事件只追加，不修改、不删除。`id` / `seq` / 时间戳由核生成，调用方提供即拒绝。
- 跨边界数据统一用 JSON；复杂结构不用裸指针传递。**例外**：终端 PTY 的输入输出
  是裸字节流，用裸缓冲（见 `spec/90-ffi.md` 的「字节流例外」）。
- 保持核极小。新增 API 前先确认它属于五条契约的哪一条。
- WinUI 外衣的核生命周期跟随进程：启动时 init 一次，**最后一个窗口关闭**时才
  shutdown + free。多窗口（如终端窗口）共享同一个核实例，不各开一个核。
- Commit message 用中文。

## 交互铁律（不要让我猜、不要让我记）

1. **不要让我猜**：信息不足就查、就问。凡推测必须标注
   `已实测：…` / `未实测，推断：…`，不得把猜测当结论。
2. **不要让我记**：需要记住的常量与约定，都要写进 `spec/` 或本文件，可查。
3. **安全动作要顺**：构建、测试、读文件直接做，不必反复请示。
4. **危险动作要像发射核弹**：删除、`git` 改写历史、覆盖未提交改动、
   改对外行为等不可逆操作，动手前说清「改什么 → 影响 → 能否撤销」并等确认。
5. **不夹带**：超出指令的顺手改动必须单独说明，禁止静默混入。

## 当前进度

已实现：`core_init` / `core_shutdown` / `id_generate` / `event_append` /
`event_query` / `event_snapshot_latest` + `last_error` / `string_free`。

终端模块（独立于核，`crates/onebox-terminal`）：spawn / write / read / resize /
status / close / free，Windows 上走 ConPTY；WinUI 外衣有终端窗口（WebView2 +
xterm.js，30ms 轮询拉取输出）。

未实现（仅有契约）：消息路由、权限、模块生命周期。
按契约逐条补全，每步保证能编译、能运行、能验证。
