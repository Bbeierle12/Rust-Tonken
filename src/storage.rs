use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::TransactionBehavior;

use crate::types::{ChatMessage, Session, SessionMetrics};

/// Initialize a connection pool with WAL mode, busy timeout, and NORMAL synchronous.
pub fn init_pool(db_path: &str) -> Result<Pool<SqliteConnectionManager>, Box<dyn std::error::Error + Send + Sync>> {
    let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA busy_timeout = 5000;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;
        Ok(())
    });

    let pool = Pool::builder().max_size(4).build(manager)?;
    Ok(pool)
}

/// Run schema migrations to create tables if they don't exist.
pub fn run_migrations(pool: &Pool<SqliteConnectionManager>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            model TEXT NOT NULL,
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_duration_nanos INTEGER NOT NULL DEFAULT 0,
            eval_duration_nanos INTEGER NOT NULL DEFAULT 0,
            tps REAL NOT NULL DEFAULT 0.0,
            ttft_ms REAL NOT NULL DEFAULT 0.0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS token_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            metric_name TEXT NOT NULL,
            metric_value REAL NOT NULL,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_messages_session_id ON messages(session_id);
        CREATE INDEX IF NOT EXISTS idx_token_metrics_session_id ON token_metrics(session_id);",
    )?;
    Ok(())
}

/// Save a session (INSERT OR REPLACE) within an IMMEDIATE transaction.
pub fn save_session(
    pool: &Pool<SqliteConnectionManager>,
    session: &Session,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = pool.get()?;
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;

    tx.execute(
        "INSERT OR REPLACE INTO sessions (id, title, model, prompt_tokens, completion_tokens,
         total_duration_nanos, eval_duration_nanos, tps, ttft_ms, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        rusqlite::params![
            session.id,
            session.title,
            session.model,
            session.metrics.prompt_tokens,
            session.metrics.completion_tokens,
            session.metrics.total_duration_nanos,
            session.metrics.eval_duration_nanos,
            session.metrics.tps,
            session.metrics.ttft_ms,
            session.created_at,
            session.updated_at,
        ],
    )?;

    // Delete existing messages for this session, then re-insert
    tx.execute(
        "DELETE FROM messages WHERE session_id = ?1",
        rusqlite::params![session.id],
    )?;

    for msg in &session.messages {
        tx.execute(
            "INSERT INTO messages (session_id, role, content) VALUES (?1, ?2, ?3)",
            rusqlite::params![session.id, msg.role, msg.content],
        )?;
    }

    tx.commit()?;
    Ok(())
}

/// Load a session by ID. Returns None if not found.
pub fn load_session(
    pool: &Pool<SqliteConnectionManager>,
    id: &str,
) -> Result<Option<Session>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;

    let mut stmt = conn.prepare(
        "SELECT id, title, model, prompt_tokens, completion_tokens,
         total_duration_nanos, eval_duration_nanos, tps, ttft_ms,
         created_at, updated_at FROM sessions WHERE id = ?1",
    )?;

    let session = stmt
        .query_row(rusqlite::params![id], |row| {
            Ok(Session {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                messages: Vec::new(), // filled below
                metrics: SessionMetrics {
                    prompt_tokens: row.get::<_, i64>(3)? as u64,
                    completion_tokens: row.get::<_, i64>(4)? as u64,
                    total_duration_nanos: row.get::<_, i64>(5)? as u64,
                    eval_duration_nanos: row.get::<_, i64>(6)? as u64,
                    tps: row.get(7)?,
                    ttft_ms: row.get(8)?,
                },
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })
        .optional()?;

    match session {
        Some(mut s) => {
            let mut msg_stmt =
                conn.prepare("SELECT role, content FROM messages WHERE session_id = ?1 ORDER BY id")?;
            let messages = msg_stmt
                .query_map(rusqlite::params![id], |row| {
                    Ok(ChatMessage {
                        role: row.get(0)?,
                        content: row.get(1)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            s.messages = messages;
            Ok(Some(s))
        }
        None => Ok(None),
    }
}

/// List all sessions (without messages, for sidebar display).
pub fn list_sessions(
    pool: &Pool<SqliteConnectionManager>,
) -> Result<Vec<Session>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, model, prompt_tokens, completion_tokens,
         total_duration_nanos, eval_duration_nanos, tps, ttft_ms,
         created_at, updated_at FROM sessions ORDER BY updated_at DESC",
    )?;

    let sessions = stmt
        .query_map([], |row| {
            Ok(Session {
                id: row.get(0)?,
                title: row.get(1)?,
                model: row.get(2)?,
                messages: Vec::new(),
                metrics: SessionMetrics {
                    prompt_tokens: row.get::<_, i64>(3)? as u64,
                    completion_tokens: row.get::<_, i64>(4)? as u64,
                    total_duration_nanos: row.get::<_, i64>(5)? as u64,
                    eval_duration_nanos: row.get::<_, i64>(6)? as u64,
                    tps: row.get(7)?,
                    ttft_ms: row.get(8)?,
                },
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(sessions)
}

/// Delete a session and its messages (FK cascade).
pub fn delete_session(
    pool: &Pool<SqliteConnectionManager>,
    id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    conn.execute("DELETE FROM sessions WHERE id = ?1", rusqlite::params![id])?;
    Ok(())
}

/// Helper trait to make query_row return Option
use rusqlite::OptionalExtension;
