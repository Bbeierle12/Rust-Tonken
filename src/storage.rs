use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::TransactionBehavior;

use crate::types::{ChatMessage, Session, SessionMetrics, TurnMetrics};

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

    // Original schema
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

    // Idempotent ALTER TABLE for new session columns (silently ignore duplicate column errors)
    for stmt in &[
        "ALTER TABLE sessions ADD COLUMN load_duration_nanos INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE sessions ADD COLUMN prompt_eval_duration_nanos INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE sessions ADD COLUMN turn_count INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE sessions ADD COLUMN total_wall_clock_ms REAL NOT NULL DEFAULT 0.0",
    ] {
        let _ = conn.execute(stmt, []);
    }

    // New turn_metrics table
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS turn_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id TEXT NOT NULL,
            turn_index INTEGER NOT NULL,
            -- Token / timing
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_duration_nanos INTEGER NOT NULL DEFAULT 0,
            eval_duration_nanos INTEGER NOT NULL DEFAULT 0,
            load_duration_nanos INTEGER NOT NULL DEFAULT 0,
            prompt_eval_duration_nanos INTEGER NOT NULL DEFAULT 0,
            tps REAL NOT NULL DEFAULT 0.0,
            ttft_ms REAL NOT NULL DEFAULT 0.0,
            wall_clock_ms REAL NOT NULL DEFAULT 0.0,
            -- Sentiment
            sentiment_score REAL NOT NULL DEFAULT 0.0,
            user_sentiment_score REAL NOT NULL DEFAULT 0.0,
            dominant_emotion TEXT,
            emotion_counts_json TEXT NOT NULL DEFAULT '[]',
            emotional_range INTEGER NOT NULL DEFAULT 0,
            -- Linguistic
            reading_level REAL NOT NULL DEFAULT 0.0,
            avg_sentence_length REAL NOT NULL DEFAULT 0.0,
            avg_word_length REAL NOT NULL DEFAULT 0.0,
            type_token_ratio REAL NOT NULL DEFAULT 0.0,
            hapax_percentage REAL NOT NULL DEFAULT 0.0,
            lexical_density REAL NOT NULL DEFAULT 0.0,
            -- Conversational
            response_amplification REAL NOT NULL DEFAULT 0.0,
            question_density REAL NOT NULL DEFAULT 0.0,
            hedging_index REAL NOT NULL DEFAULT 0.0,
            code_density REAL NOT NULL DEFAULT 0.0,
            list_density REAL NOT NULL DEFAULT 0.0,
            topic_similarity_prev REAL NOT NULL DEFAULT 0.0,
            topic_similarity_first REAL NOT NULL DEFAULT 0.0,
            -- Style
            formality_score REAL NOT NULL DEFAULT 0.0,
            repetition_index REAL NOT NULL DEFAULT 0.0,
            instructional_density REAL NOT NULL DEFAULT 0.0,
            certainty_score REAL NOT NULL DEFAULT 0.0,
            FOREIGN KEY (session_id) REFERENCES sessions(id) ON DELETE CASCADE,
            UNIQUE(session_id, turn_index)
        );

        CREATE INDEX IF NOT EXISTS idx_turn_metrics_session_id ON turn_metrics(session_id);",
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
         total_duration_nanos, eval_duration_nanos, tps, ttft_ms,
         load_duration_nanos, prompt_eval_duration_nanos, turn_count, total_wall_clock_ms,
         created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
            session.metrics.load_duration_nanos,
            session.metrics.prompt_eval_duration_nanos,
            session.metrics.turn_count,
            session.metrics.total_wall_clock_ms,
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

/// Save a single turn's metrics (INSERT OR REPLACE).
pub fn save_turn_metrics(
    pool: &Pool<SqliteConnectionManager>,
    session_id: &str,
    turn: &TurnMetrics,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let emotion_json = serde_json::to_string(&turn.emotion_counts).unwrap_or_else(|_| "[]".to_string());

    conn.execute(
        "INSERT OR REPLACE INTO turn_metrics (
            session_id, turn_index,
            prompt_tokens, completion_tokens, total_duration_nanos, eval_duration_nanos,
            load_duration_nanos, prompt_eval_duration_nanos, tps, ttft_ms, wall_clock_ms,
            sentiment_score, user_sentiment_score, dominant_emotion, emotion_counts_json, emotional_range,
            reading_level, avg_sentence_length, avg_word_length, type_token_ratio, hapax_percentage, lexical_density,
            response_amplification, question_density, hedging_index, code_density, list_density,
            topic_similarity_prev, topic_similarity_first,
            formality_score, repetition_index, instructional_density, certainty_score
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11,
            ?12, ?13, ?14, ?15, ?16,
            ?17, ?18, ?19, ?20, ?21, ?22,
            ?23, ?24, ?25, ?26, ?27, ?28, ?29,
            ?30, ?31, ?32, ?33
        )",
        rusqlite::params![
            session_id,
            turn.turn_index,
            turn.prompt_tokens,
            turn.completion_tokens,
            turn.total_duration_nanos,
            turn.eval_duration_nanos,
            turn.load_duration_nanos,
            turn.prompt_eval_duration_nanos,
            turn.tps,
            turn.ttft_ms,
            turn.wall_clock_ms,
            turn.sentiment_score,
            turn.user_sentiment_score,
            turn.dominant_emotion,
            emotion_json,
            turn.emotional_range,
            turn.reading_level,
            turn.avg_sentence_length,
            turn.avg_word_length,
            turn.type_token_ratio,
            turn.hapax_percentage,
            turn.lexical_density,
            turn.response_amplification,
            turn.question_density,
            turn.hedging_index,
            turn.code_density,
            turn.list_density,
            turn.topic_similarity_prev,
            turn.topic_similarity_first,
            turn.formality_score,
            turn.repetition_index,
            turn.instructional_density,
            turn.certainty_score,
        ],
    )?;

    Ok(())
}

/// Load all turn metrics for a session, ordered by turn_index.
pub fn load_turn_metrics(
    pool: &Pool<SqliteConnectionManager>,
    session_id: &str,
) -> Result<Vec<TurnMetrics>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT turn_index,
            prompt_tokens, completion_tokens, total_duration_nanos, eval_duration_nanos,
            load_duration_nanos, prompt_eval_duration_nanos, tps, ttft_ms, wall_clock_ms,
            sentiment_score, user_sentiment_score, dominant_emotion, emotion_counts_json, emotional_range,
            reading_level, avg_sentence_length, avg_word_length, type_token_ratio, hapax_percentage, lexical_density,
            response_amplification, question_density, hedging_index, code_density, list_density,
            topic_similarity_prev, topic_similarity_first,
            formality_score, repetition_index, instructional_density, certainty_score
        FROM turn_metrics WHERE session_id = ?1 ORDER BY turn_index",
    )?;

    let turns = stmt
        .query_map(rusqlite::params![session_id], |row| {
            let emotion_json: String = row.get(13)?;
            let emotion_counts: Vec<(String, u32)> =
                serde_json::from_str(&emotion_json).unwrap_or_default();

            Ok(TurnMetrics {
                turn_index: row.get::<_, i64>(0)? as usize,
                prompt_tokens: row.get::<_, i64>(1)? as u64,
                completion_tokens: row.get::<_, i64>(2)? as u64,
                total_duration_nanos: row.get::<_, i64>(3)? as u64,
                eval_duration_nanos: row.get::<_, i64>(4)? as u64,
                load_duration_nanos: row.get::<_, i64>(5)? as u64,
                prompt_eval_duration_nanos: row.get::<_, i64>(6)? as u64,
                tps: row.get(7)?,
                ttft_ms: row.get(8)?,
                wall_clock_ms: row.get(9)?,
                sentiment_score: row.get(10)?,
                user_sentiment_score: row.get(11)?,
                dominant_emotion: row.get(12)?,
                emotion_counts,
                emotional_range: row.get::<_, i64>(14)? as u32,
                reading_level: row.get(15)?,
                avg_sentence_length: row.get(16)?,
                avg_word_length: row.get(17)?,
                type_token_ratio: row.get(18)?,
                hapax_percentage: row.get(19)?,
                lexical_density: row.get(20)?,
                response_amplification: row.get(21)?,
                question_density: row.get(22)?,
                hedging_index: row.get(23)?,
                code_density: row.get(24)?,
                list_density: row.get(25)?,
                topic_similarity_prev: row.get(26)?,
                topic_similarity_first: row.get(27)?,
                formality_score: row.get(28)?,
                repetition_index: row.get(29)?,
                instructional_density: row.get(30)?,
                certainty_score: row.get(31)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(turns)
}

/// Load a session by ID. Returns None if not found.
pub fn load_session(
    pool: &Pool<SqliteConnectionManager>,
    id: &str,
) -> Result<Option<Session>, Box<dyn std::error::Error + Send + Sync>> {
    // Scope the connection so it's released before we call load_turn_metrics
    let session = {
        let conn = pool.get()?;

        let mut stmt = conn.prepare(
            "SELECT id, title, model, prompt_tokens, completion_tokens,
             total_duration_nanos, eval_duration_nanos, tps, ttft_ms,
             load_duration_nanos, prompt_eval_duration_nanos, turn_count, total_wall_clock_ms,
             created_at, updated_at FROM sessions WHERE id = ?1",
        )?;

        let session = stmt
            .query_row(rusqlite::params![id], |row| {
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
                        load_duration_nanos: row.get::<_, i64>(9)? as u64,
                        prompt_eval_duration_nanos: row.get::<_, i64>(10)? as u64,
                        turn_count: row.get::<_, i64>(11)? as u32,
                        total_wall_clock_ms: row.get(12)?,
                        tps_history: Vec::new(),
                        ttft_history: Vec::new(),
                        turn_metrics: Vec::new(),
                    },
                    created_at: row.get(13)?,
                    updated_at: row.get(14)?,
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
                Some(s)
            }
            None => None,
        }
    }; // conn dropped here

    match session {
        Some(mut s) => {
            // Load turn metrics with a fresh connection
            let turns = load_turn_metrics(pool, id)?;
            s.metrics.tps_history = turns.iter().map(|t| t.tps).collect();
            s.metrics.ttft_history = turns.iter().map(|t| t.ttft_ms).collect();
            s.metrics.turn_metrics = turns;
            Ok(Some(s))
        }
        None => Ok(None),
    }
}

/// List all sessions (without messages or turn_metrics, for sidebar/list display).
pub fn list_sessions(
    pool: &Pool<SqliteConnectionManager>,
) -> Result<Vec<Session>, Box<dyn std::error::Error + Send + Sync>> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare(
        "SELECT id, title, model, prompt_tokens, completion_tokens,
         total_duration_nanos, eval_duration_nanos, tps, ttft_ms,
         load_duration_nanos, prompt_eval_duration_nanos, turn_count, total_wall_clock_ms,
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
                    load_duration_nanos: row.get::<_, i64>(9)? as u64,
                    prompt_eval_duration_nanos: row.get::<_, i64>(10)? as u64,
                    turn_count: row.get::<_, i64>(11)? as u32,
                    total_wall_clock_ms: row.get(12)?,
                    tps_history: Vec::new(),
                    ttft_history: Vec::new(),
                    turn_metrics: Vec::new(),
                },
                created_at: row.get(13)?,
                updated_at: row.get(14)?,
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
