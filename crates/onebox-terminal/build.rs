//! 构建脚本：调用 cbindgen，从本 crate 生成 `include/onebox_terminal.h`。

use std::env;
use std::path::PathBuf;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("缺少环境变量 CARGO_MANIFEST_DIR"));

    // crates/onebox-terminal -> 仓库根
    let repo_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .expect("无法从 crate 目录定位仓库根目录");

    let config_path = manifest_dir.join("cbindgen.toml");
    let header_path = repo_root.join("include").join("onebox_terminal.h");

    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed={}", config_path.display());

    let config = cbindgen::Config::from_file(&config_path).expect("解析 cbindgen.toml 失败");

    let bindings = cbindgen::Builder::new()
        .with_crate(&manifest_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen 生成 C 绑定失败");

    if let Some(parent) = header_path.parent() {
        std::fs::create_dir_all(parent).expect("创建 include 目录失败");
    }
    // 注意：write_to_file 返回 false 仅表示“文件内容未变化”，属正常情况，不能当失败。
    bindings.write_to_file(&header_path);
}
