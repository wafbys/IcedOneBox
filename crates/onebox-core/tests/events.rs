//! 核的集成测试：验证「一切皆事件」与「万物有 ID」的最小契约。

use onebox_core::{Core, CoreConfig, CoreError};

/// 构建一个使用内存数据库的核实例。
fn memory_core() -> Core {
    Core::init(CoreConfig {
        db_path: Some(":memory:".to_string()),
    })
    .expect("初始化内存核失败")
}

#[test]
fn append_then_query_roundtrip() {
    let mut core = memory_core();
    let event = core
        .append_event(r#"{"event_type":"test.created","source":"unit","payload":{"n":1}}"#)
        .expect("追加事件失败");

    assert_eq!(event.seq, 1, "第一条事件 seq 应为 1");
    assert_eq!(event.event_type, "test.created");
    assert_eq!(event.source.as_deref(), Some("unit"));
    assert!(!event.id.is_empty(), "事件 ID 不应为空");
    assert!(event.ts_millis > 0, "时间戳应为正数");

    let json = core
        .query_events(Some(r#"{"event_type":"test.created"}"#))
        .expect("查询事件失败");
    assert!(json.contains("\"seq\":1"));
    assert!(json.contains("test.created"));
    assert!(json.contains("unit"));
}

#[test]
fn seq_is_monotonic_and_ids_are_unique() {
    let mut core = memory_core();
    let first = core.append_event(r#"{"event_type":"a"}"#).unwrap();
    let second = core.append_event(r#"{"event_type":"a"}"#).unwrap();

    assert_eq!(first.seq, 1);
    assert_eq!(second.seq, 2);
    assert_ne!(first.id, second.id, "两条事件的 ID 不应相同");
}

#[test]
fn caller_supplied_reserved_fields_are_rejected() {
    let mut core = memory_core();

    let err = core
        .append_event(r#"{"event_type":"bad","id":"forged"}"#)
        .expect_err("提供 id 必须被拒绝");
    assert!(matches!(err, CoreError::InvalidJson(_)), "实际错误: {err:?}");

    let err = core
        .append_event(r#"{"event_type":"bad","seq":7}"#)
        .expect_err("提供 seq 必须被拒绝");
    assert!(matches!(err, CoreError::InvalidJson(_)), "实际错误: {err:?}");

    let err = core
        .append_event(r#"{"event_type":"bad","ts_millis":1}"#)
        .expect_err("提供 ts_millis 必须被拒绝");
    assert!(matches!(err, CoreError::InvalidJson(_)), "实际错误: {err:?}");
}

#[test]
fn empty_event_type_is_rejected() {
    let mut core = memory_core();
    let err = core
        .append_event(r#"{"event_type":"  "}"#)
        .expect_err("空 event_type 必须被拒绝");
    assert!(matches!(err, CoreError::InvalidArgument(_)), "实际错误: {err:?}");
}

#[test]
fn query_respects_filters_limit_and_paging() {
    let mut core = memory_core();
    for index in 0..5 {
        core.append_event(&format!(
            r#"{{"event_type":"t","source":"s{index}","payload":{index}}}"#
        ))
        .unwrap();
    }

    let filtered = core
        .query_events(Some(r#"{"event_type":"t","source":"s3"}"#))
        .expect("查询失败");
    assert!(filtered.contains("\"source\":\"s3\""));
    assert!(!filtered.contains("\"source\":\"s2\""));

    // 第一页：limit=2，应返回 seq 1、2，并给出翻页游标 2。
    let page_one = core
        .query_events(Some(r#"{"event_type":"t","limit":2}"#))
        .expect("查询失败");
    assert!(page_one.contains("\"seq\":1"));
    assert!(page_one.contains("\"seq\":2"));
    assert!(page_one.contains("\"next_since_seq\":2"));

    // 第二页：since_seq=2，应从 seq 3 开始。
    let page_two = core
        .query_events(Some(r#"{"event_type":"t","limit":2,"since_seq":2}"#))
        .expect("查询失败");
    assert!(page_two.contains("\"seq\":3"));
    assert!(!page_two.contains("\"seq\":1"));
}

#[test]
fn unknown_filter_field_is_rejected() {
    let core = memory_core();
    let err = core
        .query_events(Some(r#"{"order":"desc"}"#))
        .expect_err("未知过滤字段必须被拒绝");
    assert!(matches!(err, CoreError::InvalidJson(_)), "实际错误: {err:?}");
}

#[test]
fn shutdown_makes_core_unusable() {
    let mut core = memory_core();
    core.shutdown();
    let err = core
        .append_event(r#"{"event_type":"a"}"#)
        .expect_err("关闭后不可用");
    assert!(matches!(err, CoreError::Internal(_)), "实际错误: {err:?}");
}
