use serde::{Deserialize, Serialize};

/// A single chunk from the Ollama streaming chat API (NDJSON line).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatChunk {
    pub model: String,
    pub created_at: String,
    #[serde(default)]
    pub message: Option<ChunkMessage>,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub load_duration: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u64>,
    #[serde(default)]
    pub prompt_eval_duration: Option<u64>,
    #[serde(default)]
    pub eval_count: Option<u64>,
    #[serde(default)]
    pub eval_duration: Option<u64>,
}

/// The message fragment within a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
}

/// Request body sent to Ollama's /api/chat endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub stream: bool,
}

/// A single chat message (user or assistant).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// A complete chat session with its messages and accumulated metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub metrics: SessionMetrics,
    pub created_at: String,
    pub updated_at: String,
}

/// Accumulated metrics for an entire session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_duration_nanos: u64,
    pub eval_duration_nanos: u64,
    pub tps: f64,
    pub ttft_ms: f64,
}
