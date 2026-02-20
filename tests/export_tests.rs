use ollama_scope::export::{export_sessions, ExportRow};
use ollama_scope::types::{ChatMessage, Session, SessionMetrics};
use proptest::prelude::*;

fn sample_session(id: &str, messages: Vec<ChatMessage>) -> Session {
    Session {
        id: id.to_string(),
        title: "Test".to_string(),
        model: "llama3".to_string(),
        messages,
        metrics: SessionMetrics::default(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:01:00Z".to_string(),
    }
}

fn roundtrip(sessions: &[Session]) -> Vec<ExportRow> {
    let mut buf = Vec::new();
    export_sessions(sessions, &mut buf).unwrap();
    let mut reader = csv::Reader::from_reader(buf.as_slice());
    reader
        .deserialize()
        .collect::<Result<Vec<ExportRow>, _>>()
        .unwrap()
}

#[test]
fn test_roundtrip_equality() {
    let sessions = vec![sample_session(
        "s1",
        vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Hi!".to_string(),
            },
        ],
    )];

    let rows = roundtrip(&sessions);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].session_id, "s1");
    assert_eq!(rows[0].role, "user");
    assert_eq!(rows[0].content, "Hello");
    assert_eq!(rows[1].role, "assistant");
    assert_eq!(rows[1].content, "Hi!");
}

#[test]
fn test_content_with_commas() {
    let sessions = vec![sample_session(
        "s1",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello, world, how are you?".to_string(),
        }],
    )];

    let rows = roundtrip(&sessions);
    assert_eq!(rows[0].content, "Hello, world, how are you?");
}

#[test]
fn test_content_with_newlines() {
    let sessions = vec![sample_session(
        "s1",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Line 1\nLine 2\nLine 3".to_string(),
        }],
    )];

    let rows = roundtrip(&sessions);
    assert_eq!(rows[0].content, "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_content_with_emoji() {
    let sessions = vec![sample_session(
        "s1",
        vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello 🎉🚀 world!".to_string(),
        }],
    )];

    let rows = roundtrip(&sessions);
    assert_eq!(rows[0].content, "Hello 🎉🚀 world!");
}

#[test]
fn test_empty_messages_still_exports() {
    let sessions = vec![sample_session("s1", vec![])];
    let rows = roundtrip(&sessions);
    assert_eq!(rows.len(), 1, "Empty session should still produce a row");
    assert_eq!(rows[0].role, "");
    assert_eq!(rows[0].content, "");
}

#[test]
fn test_content_with_double_quotes() {
    let sessions = vec![sample_session(
        "s1",
        vec![ChatMessage {
            role: "user".to_string(),
            content: r#"He said "hello" to me"#.to_string(),
        }],
    )];

    let rows = roundtrip(&sessions);
    assert_eq!(rows[0].content, r#"He said "hello" to me"#);
}

#[test]
fn test_csv_output_snapshot() {
    let sessions = vec![sample_session(
        "s1",
        vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Hello!".to_string(),
            },
        ],
    )];

    let mut buf = Vec::new();
    export_sessions(&sessions, &mut buf).unwrap();
    let output = String::from_utf8(buf).unwrap();
    insta::assert_snapshot!(output);
}

proptest! {
    #[test]
    fn prop_unicode_roundtrip(content in "\\PC{1,100}") {
        let sessions = vec![sample_session(
            "s1",
            vec![ChatMessage {
                role: "user".to_string(),
                content: content.clone(),
            }],
        )];

        let rows = roundtrip(&sessions);
        prop_assert_eq!(&rows[0].content, &content);
    }
}
