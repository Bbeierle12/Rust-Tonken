use serde::{Deserialize, Serialize};

/// Connection status to the Ollama server.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionStatus {
    Unknown,
    Connected,
    Disconnected,
    Checking,
}

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
    // New aggregate fields
    #[serde(default)]
    pub load_duration_nanos: u64,
    #[serde(default)]
    pub prompt_eval_duration_nanos: u64,
    #[serde(default)]
    pub turn_count: u32,
    #[serde(default)]
    pub total_wall_clock_ms: f64,
    #[serde(default)]
    pub tps_history: Vec<f64>,
    #[serde(default)]
    pub ttft_history: Vec<f64>,
    #[serde(default)]
    pub turn_metrics: Vec<TurnMetrics>,
}

/// Per-turn metrics combining Ollama API metrics + content analysis.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TurnMetrics {
    #[serde(default)]
    pub turn_index: usize,
    // Token / timing
    #[serde(default)]
    pub prompt_tokens: u64,
    #[serde(default)]
    pub completion_tokens: u64,
    #[serde(default)]
    pub total_duration_nanos: u64,
    #[serde(default)]
    pub eval_duration_nanos: u64,
    #[serde(default)]
    pub load_duration_nanos: u64,
    #[serde(default)]
    pub prompt_eval_duration_nanos: u64,
    #[serde(default)]
    pub tps: f64,
    #[serde(default)]
    pub ttft_ms: f64,
    #[serde(default)]
    pub wall_clock_ms: f64,
    // Sentiment
    #[serde(default)]
    pub sentiment_score: f64,
    #[serde(default)]
    pub user_sentiment_score: f64,
    #[serde(default)]
    pub dominant_emotion: Option<String>,
    #[serde(default)]
    pub emotion_counts: Vec<(String, u32)>,
    #[serde(default)]
    pub emotional_range: u32,
    // Linguistic
    #[serde(default)]
    pub reading_level: f64,
    #[serde(default)]
    pub avg_sentence_length: f64,
    #[serde(default)]
    pub avg_word_length: f64,
    #[serde(default)]
    pub type_token_ratio: f64,
    #[serde(default)]
    pub hapax_percentage: f64,
    #[serde(default)]
    pub lexical_density: f64,
    // Conversational
    #[serde(default)]
    pub response_amplification: f64,
    #[serde(default)]
    pub question_density: f64,
    #[serde(default)]
    pub hedging_index: f64,
    #[serde(default)]
    pub code_density: f64,
    #[serde(default)]
    pub list_density: f64,
    #[serde(default)]
    pub topic_similarity_prev: f64,
    #[serde(default)]
    pub topic_similarity_first: f64,
    // Style
    #[serde(default)]
    pub formality_score: f64,
    #[serde(default)]
    pub repetition_index: f64,
    #[serde(default)]
    pub instructional_density: f64,
    #[serde(default)]
    pub certainty_score: f64,
}
