//! 终端会话冒烟：直接驱动 `Session`（不经 FFI）。
//!
//! 验证最小闭环：spawn → 读输出 → resize → close 能干净退出、不挂起。

use std::time::{Duration, Instant};

use onebox_terminal::session::{Session, SpawnConfig};

/// ConPTY 启动时会发 `ESC[6n`（查询光标位置）；真实终端（xterm.js）会自动回
/// 一个 CPR（`ESC[row;colR`）。自动化测试里没有终端，这里做最小应答，
/// 否则 ConPTY 不会继续输出。这也正是外衣里 xterm.js 的职责。
fn respond_to_dsr(session: &mut Session, chunk: &[u8]) {
    if chunk.windows(4).any(|w| w == b"\x1b[6n") {
        let _ = session.write(b"\x1b[1;1R");
    }
}

/// 读循环：在超时内尽量收集输出，并等待子进程退出。
fn drain_until_exit(session: &mut Session, total_timeout: Duration) -> Vec<u8> {
    let deadline = Instant::now() + total_timeout;
    let mut collected = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let mut read_any = false;
        loop {
            let n = session.read(&mut buf);
            if n == 0 {
                break;
            }
            collected.extend_from_slice(&buf[..n]);
            respond_to_dsr(session, &buf[..n]);
            read_any = true;
        }

        let (exited, _code) = session.status().expect("status 调用失败");
        // 子进程已退出、且本轮没有再读到数据，即认为输出收敛。
        if exited && !read_any {
            break;
        }
        if Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    collected
}

#[test]
fn echo_round_trip() {
    let config = SpawnConfig {
        program: "cmd.exe".to_string(),
        args: vec!["/c".to_string(), "echo hello-onebox".to_string()],
        cwd: None,
        cols: 80,
        rows: 24,
    };
    let mut session = Session::spawn(config).expect("spawn 失败");

    let output = drain_until_exit(&mut session, Duration::from_secs(10));
    session.close();

    let text = String::from_utf8_lossy(&output);
    assert!(
        text.contains("hello-onebox"),
        "输出应包含 hello-onebox，实际: {text:?}"
    );
}

#[test]
fn resize_after_spawn() {
    let config = SpawnConfig {
        program: "cmd.exe".to_string(),
        args: Vec::new(),
        cwd: None,
        cols: 80,
        rows: 24,
    };
    let mut session = Session::spawn(config).expect("spawn 失败");

    session.resize(120, 40).expect("resize 失败");
    session.resize(100, 30).expect("第二次 resize 失败");

    // 写入一条命令证明写路径可用，然后关闭。
    session.write(b"echo ok\r\n").expect("write 失败");
    let output = drain_until_exit(&mut session, Duration::from_secs(10));
    session.close();

    let text = String::from_utf8_lossy(&output);
    assert!(text.contains("ok"), "写路径输出应包含 ok，实际: {text:?}");
}

#[test]
fn config_defaults_and_unknown_field() {
    // 空配置 → 默认值。
    let config = SpawnConfig::from_json(None).expect("默认配置应成功");
    assert_eq!(config.program, "powershell.exe");
    assert_eq!(config.cols, 80);
    assert_eq!(config.rows, 24);

    // 未知字段 → 拒绝。
    assert!(SpawnConfig::from_json(Some(r#"{"nope":1}"#)).is_err());
}
