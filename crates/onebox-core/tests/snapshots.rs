//! 快照契约的集成测试（见 `spec/30-snapshots.md`）。

use onebox_core::{Core, CoreConfig, CoreError};

fn memory_core() -> Core {
    Core::init(CoreConfig {
        db_path: Some(":memory:".to_string()),
    })
    .expect("初始化内存核失败")
}

/// 写入一条合法的 `core.snapshot` 事件。
fn append_snapshot(core: &mut Core, reducer_id: &str, covers_seq: i64, state: serde_json::Value) {
    let payload = serde_json::json!({
        "reducer_id": reducer_id,
        "covers_seq": covers_seq,
        "state": state,
    });
    let event = serde_json::json!({
        "event_type": "core.snapshot",
        "source": "test",
        "payload": payload,
    });
    core.append_event(&event.to_string()).expect("写入快照失败");
}

#[test]
fn snapshot_latest_returns_the_newest_for_a_reducer() {
    let mut core = memory_core();
    append_snapshot(&mut core, "gallery", 1, serde_json::json!({ "v": 1 }));
    append_snapshot(&mut core, "gallery", 2, serde_json::json!({ "v": 2 }));

    let json = core.snapshot_latest("gallery").expect("应有快照");
    assert!(json.contains("\"reducer_id\":\"gallery\""), "{json}");
    assert!(json.contains("\"covers_seq\":2"), "应取最新一条: {json}");
    assert!(json.contains("\"v\":2"), "{json}");
    assert!(!json.contains("\"v\":1"), "不应返回旧快照: {json}");
}

#[test]
fn snapshot_latest_isolates_reducers() {
    let mut core = memory_core();
    append_snapshot(&mut core, "a", 1, serde_json::json!({ "who": "A" }));
    append_snapshot(&mut core, "b", 2, serde_json::json!({ "who": "B" }));

    let a = core.snapshot_latest("a").expect("a 应有快照");
    assert!(a.contains("\"who\":\"A\""), "{a}");
    assert!(!a.contains("\"who\":\"B\""), "不应串到别的 reducer: {a}");

    let b = core.snapshot_latest("b").expect("b 应有快照");
    assert!(b.contains("\"who\":\"B\""), "{b}");
}

#[test]
fn snapshot_latest_without_snapshot_is_not_found() {
    let core = memory_core();
    let err = core.snapshot_latest("missing").expect_err("应无快照");
    assert!(matches!(err, CoreError::NotFound(_)), "实际错误: {err:?}");
}

#[test]
fn snapshot_latest_skips_events_that_violate_the_contract() {
    let mut core = memory_core();

    // 1. payload 不是 JSON 对象（合法 JSON，但是字符串）
    core.append_event(r#"{"event_type":"core.snapshot","payload":"not an object"}"#)
        .unwrap();
    // 2. payload 是对象，但缺 reducer_id
    core.append_event(r#"{"event_type":"core.snapshot","payload":{"covers_seq":1}}"#)
        .unwrap();
    // 3. reducer_id 不是字符串
    core.append_event(
        r#"{"event_type":"core.snapshot","payload":{"reducer_id":123,"covers_seq":1}}"#,
    )
    .unwrap();
    // 4. 字段齐全但 event_type 不是 core.snapshot（不参与匹配）
    core.append_event(r#"{"event_type":"other","payload":{"reducer_id":"r","covers_seq":1}}"#)
        .unwrap();

    let err = core.snapshot_latest("r").expect_err("以上都应被跳过");
    assert!(matches!(err, CoreError::NotFound(_)), "实际错误: {err:?}");

    // 追加一条合法快照后，应能查到它（证明是跳过而非报错）。
    append_snapshot(&mut core, "r", 5, serde_json::json!({ "ok": true }));
    let json = core.snapshot_latest("r").expect("应能查到合法快照");
    assert!(json.contains("\"ok\":true"), "{json}");
}

#[test]
fn snapshot_latest_rejects_empty_reducer_id() {
    let core = memory_core();
    let err = core
        .snapshot_latest("   ")
        .expect_err("空 reducer_id 应被拒绝");
    assert!(
        matches!(err, CoreError::InvalidArgument(_)),
        "实际错误: {err:?}"
    );
}
