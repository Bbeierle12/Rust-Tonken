use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::types::Session;

/// A single row in the CSV export, representing one message with its session context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExportRow {
    pub session_id: String,
    pub session_title: String,
    pub model: String,
    pub role: String,
    pub content: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_duration_nanos: u64,
    pub eval_duration_nanos: u64,
    pub tps: f64,
    pub ttft_ms: f64,
    pub created_at: String,
    pub updated_at: String,
}

/// Export sessions as CSV to any writer.
///
/// Each message becomes one row, with session-level metadata repeated.
/// Uses the `csv` crate's built-in quoting — no manual quoting needed.
pub fn export_sessions(
    sessions: &[Session],
    writer: impl Write,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut wtr = csv::Writer::from_writer(writer);

    for session in sessions {
        if session.messages.is_empty() {
            // Still emit one row for sessions with no messages
            let row = ExportRow {
                session_id: session.id.clone(),
                session_title: session.title.clone(),
                model: session.model.clone(),
                role: String::new(),
                content: String::new(),
                prompt_tokens: session.metrics.prompt_tokens,
                completion_tokens: session.metrics.completion_tokens,
                total_duration_nanos: session.metrics.total_duration_nanos,
                eval_duration_nanos: session.metrics.eval_duration_nanos,
                tps: session.metrics.tps,
                ttft_ms: session.metrics.ttft_ms,
                created_at: session.created_at.clone(),
                updated_at: session.updated_at.clone(),
            };
            wtr.serialize(&row)?;
        }

        for msg in &session.messages {
            let row = ExportRow {
                session_id: session.id.clone(),
                session_title: session.title.clone(),
                model: session.model.clone(),
                role: msg.role.clone(),
                content: msg.content.clone(),
                prompt_tokens: session.metrics.prompt_tokens,
                completion_tokens: session.metrics.completion_tokens,
                total_duration_nanos: session.metrics.total_duration_nanos,
                eval_duration_nanos: session.metrics.eval_duration_nanos,
                tps: session.metrics.tps,
                ttft_ms: session.metrics.ttft_ms,
                created_at: session.created_at.clone(),
                updated_at: session.updated_at.clone(),
            };
            wtr.serialize(&row)?;
        }
    }

    wtr.flush()?;
    Ok(())
}
