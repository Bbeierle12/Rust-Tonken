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
    pub load_duration_nanos: u64,
    pub prompt_eval_duration_nanos: u64,
    pub turn_count: u32,
    pub total_wall_clock_ms: f64,
    // Per-turn content analysis (from latest turn, or defaults)
    pub sentiment_score: f64,
    pub dominant_emotion: String,
    pub reading_level: f64,
    pub formality_score: f64,
    pub response_amplification: f64,
    pub code_density: f64,
    pub certainty_score: f64,
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
        // Get latest turn metrics for content analysis fields
        let latest_turn = session.metrics.turn_metrics.last();

        let base_row = |role: String, content: String| -> ExportRow {
            ExportRow {
                session_id: session.id.clone(),
                session_title: session.title.clone(),
                model: session.model.clone(),
                role,
                content,
                prompt_tokens: session.metrics.prompt_tokens,
                completion_tokens: session.metrics.completion_tokens,
                total_duration_nanos: session.metrics.total_duration_nanos,
                eval_duration_nanos: session.metrics.eval_duration_nanos,
                tps: session.metrics.tps,
                ttft_ms: session.metrics.ttft_ms,
                load_duration_nanos: session.metrics.load_duration_nanos,
                prompt_eval_duration_nanos: session.metrics.prompt_eval_duration_nanos,
                turn_count: session.metrics.turn_count,
                total_wall_clock_ms: session.metrics.total_wall_clock_ms,
                sentiment_score: latest_turn.map(|t| t.sentiment_score).unwrap_or(0.0),
                dominant_emotion: latest_turn
                    .and_then(|t| t.dominant_emotion.clone())
                    .unwrap_or_default(),
                reading_level: latest_turn.map(|t| t.reading_level).unwrap_or(0.0),
                formality_score: latest_turn.map(|t| t.formality_score).unwrap_or(0.0),
                response_amplification: latest_turn.map(|t| t.response_amplification).unwrap_or(0.0),
                code_density: latest_turn.map(|t| t.code_density).unwrap_or(0.0),
                certainty_score: latest_turn.map(|t| t.certainty_score).unwrap_or(0.0),
                created_at: session.created_at.clone(),
                updated_at: session.updated_at.clone(),
            }
        };

        if session.messages.is_empty() {
            // Still emit one row for sessions with no messages
            let row = base_row(String::new(), String::new());
            wtr.serialize(&row)?;
        }

        for msg in &session.messages {
            let row = base_row(msg.role.clone(), msg.content.clone());
            wtr.serialize(&row)?;
        }
    }

    wtr.flush()?;
    Ok(())
}
