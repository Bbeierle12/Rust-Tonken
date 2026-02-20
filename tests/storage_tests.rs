use ollama_scope::storage::{
    delete_session, init_pool, list_sessions, load_session, run_migrations, save_session,
};
use ollama_scope::types::{ChatMessage, Session, SessionMetrics};
use std::sync::Arc;
use tempfile::NamedTempFile;

fn temp_pool() -> (
    r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>,
    NamedTempFile,
) {
    let tmp = NamedTempFile::new().unwrap();
    let pool = init_pool(tmp.path().to_str().unwrap()).unwrap();
    run_migrations(&pool).unwrap();
    (pool, tmp)
}

fn sample_session(id: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "Test Session".to_string(),
        model: "llama3".to_string(),
        messages: vec![
            ChatMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Hi there!".to_string(),
            },
        ],
        metrics: SessionMetrics {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_duration_nanos: 1_000_000_000,
            eval_duration_nanos: 500_000_000,
            tps: 25.0,
            ttft_ms: 150.0,
        },
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:01:00Z".to_string(),
    }
}

#[test]
fn test_save_then_load() {
    let (pool, _tmp) = temp_pool();
    let session = sample_session("s1");
    save_session(&pool, &session).unwrap();

    let loaded = load_session(&pool, "s1").unwrap().unwrap();
    assert_eq!(loaded.id, "s1");
    assert_eq!(loaded.title, "Test Session");
    assert_eq!(loaded.messages.len(), 2);
    assert_eq!(loaded.messages[0].role, "user");
    assert_eq!(loaded.messages[0].content, "Hello");
    assert_eq!(loaded.messages[1].content, "Hi there!");
    assert_eq!(loaded.metrics.tps, 25.0);
}

#[test]
fn test_list_sessions() {
    let (pool, _tmp) = temp_pool();
    save_session(&pool, &sample_session("s1")).unwrap();
    save_session(&pool, &sample_session("s2")).unwrap();

    let sessions = list_sessions(&pool).unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_delete_session() {
    let (pool, _tmp) = temp_pool();
    save_session(&pool, &sample_session("s1")).unwrap();
    delete_session(&pool, "s1").unwrap();

    let loaded = load_session(&pool, "s1").unwrap();
    assert!(loaded.is_none());
}

#[test]
fn test_fk_cascade_delete() {
    let (pool, _tmp) = temp_pool();
    save_session(&pool, &sample_session("s1")).unwrap();
    delete_session(&pool, "s1").unwrap();

    // Messages should be gone due to CASCADE
    let conn = pool.get().unwrap();
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM messages WHERE session_id = ?1",
            rusqlite::params!["s1"],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn test_empty_db_returns_empty_list() {
    let (pool, _tmp) = temp_pool();
    let sessions = list_sessions(&pool).unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn test_load_nonexistent_returns_none() {
    let (pool, _tmp) = temp_pool();
    let result = load_session(&pool, "nonexistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_duplicate_session_id_replaces() {
    let (pool, _tmp) = temp_pool();
    let mut session = sample_session("s1");
    save_session(&pool, &session).unwrap();

    session.title = "Updated Title".to_string();
    session.messages = vec![ChatMessage {
        role: "user".to_string(),
        content: "New message".to_string(),
    }];
    save_session(&pool, &session).unwrap();

    let loaded = load_session(&pool, "s1").unwrap().unwrap();
    assert_eq!(loaded.title, "Updated Title");
    assert_eq!(loaded.messages.len(), 1);
    assert_eq!(loaded.messages[0].content, "New message");
}

#[test]
fn test_concurrent_writes() {
    let (pool, _tmp) = temp_pool();
    let pool = Arc::new(pool);
    let mut handles = vec![];

    for i in 0..10 {
        let pool = Arc::clone(&pool);
        handles.push(std::thread::spawn(move || {
            let session = sample_session(&format!("concurrent-{i}"));
            save_session(&pool, &session).unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let sessions = list_sessions(&pool).unwrap();
    assert_eq!(sessions.len(), 10);
}

#[test]
fn test_pragma_verification() {
    let (pool, _tmp) = temp_pool();
    let conn = pool.get().unwrap();

    // Verify WAL mode
    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(journal_mode.to_lowercase(), "wal");

    // Verify foreign keys are on
    let fk: i64 = conn
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(fk, 1);

    // Verify busy timeout
    let timeout: i64 = conn
        .query_row("PRAGMA busy_timeout", [], |row| row.get(0))
        .unwrap();
    assert_eq!(timeout, 5000);
}

#[test]
fn test_session_metrics_persistence() {
    let (pool, _tmp) = temp_pool();
    let session = sample_session("s1");
    save_session(&pool, &session).unwrap();

    let loaded = load_session(&pool, "s1").unwrap().unwrap();
    assert_eq!(loaded.metrics.prompt_tokens, 10);
    assert_eq!(loaded.metrics.completion_tokens, 20);
    assert_eq!(loaded.metrics.total_duration_nanos, 1_000_000_000);
    assert_eq!(loaded.metrics.eval_duration_nanos, 500_000_000);
    assert!((loaded.metrics.ttft_ms - 150.0).abs() < f64::EPSILON);
}
