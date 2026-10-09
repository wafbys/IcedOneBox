# 00 · 概述

> 「为什么做、要成为什么」以及设计品味，见仓库根的 [`VISION.md`](../VISION.md)。
> 本文件只描述核的定位、平台策略与当前阶段。

## 项目定位

IcedOneBox 是一个个人自用的 **One Box**：统一的数字工作台入口。

核心结构是「稳定的核 + 可替换的外衣」：

- **核（Core）**：极小的、与技术无关的规范（Spec）及其 Rust 实现。
  核不关心核外世界，只关心五条契约。核是无聊的：它只做五件事，做完就没事。
- **外衣（Shell）**：GUI、终端、AI 代理、数据模块等，各自独立，
  通过 C ABI 调用核。

## 平台策略

| 平台 | 地位 | 组合 |
|------|------|------|
| Windows | 主场 | Rust 核 + WinUI 3 外衣 |
| macOS | 客场 | Rust 核 + SwiftUI 外衣 |
| Linux | 客场 | Rust 核 + GTK 外衣 |

核本身跨平台；外衣与平台绑定。核不知道上面穿的是什么，只知道有模块注册了、
有消息来了、有事件要记。

## 分层

```
外衣 / 模块（WinUI、终端、AI、数据……）
        │  通过 C ABI
        ▼
   onebox-ffi        ← 唯一允许 unsafe；只做类型转换 / panic 拦截 / 错误映射
        │  Rust 调用
        ▼
   onebox-core       ← 纯逻辑；#![forbid(unsafe_code)]；无平台依赖
        │
        ▼
   SQLite（rusqlite bundled，静态编译）
```

## 当前阶段

开发核。目标：一个跨平台 Rust 库，暴露 C ABI，包含五条契约的最小实现。

第一步只实现五个 API + 两个支撑函数，并用一个 C 冒烟测试验证三条链路：
编译产物、C ABI 可调用、事件日志可写可查。

## 技术约束

- 语言：Rust；核编译为原生库（cdylib + staticlib），无运行时依赖。
- 头文件：cbindgen 从 `onebox-ffi` 生成 `include/onebox.h`。
- 持久化：SQLite（rusqlite，`bundled` 静态编译）。事件日志是秩序来源。
- 序列化：serde + serde_json；跨边界数据统一用 JSON。
- 异步：核保持同步，暂不引入 tokio。
- 安全：`onebox-core` 内 `#![forbid(unsafe_code)]`，unsafe 集中在 `onebox-ffi`。
- 命名：导出函数一律加 `onebox_` 前缀；标识符英文，注释与文档中文。
