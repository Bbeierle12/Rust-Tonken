use crate::types::OllamaChatChunk;

/// Final metrics reported in the last chunk of an Ollama stream (done=true).
#[derive(Debug, Clone)]
pub struct FinalStreamMetrics {
    pub total_duration: u64,
    pub load_duration: u64,
    pub prompt_eval_count: u64,
    pub prompt_eval_duration: u64,
    pub eval_count: u64,
    pub eval_duration: u64,
}

/// Events emitted by the NDJSON streaming parser.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// A successfully parsed chunk from the stream.
    Chunk {
        session_id: String,
        chunk: OllamaChatChunk,
    },
    /// The stream has completed with final metrics.
    Completed {
        session_id: String,
        metrics: FinalStreamMetrics,
    },
    /// A line could not be parsed as valid JSON.
    ParseError {
        session_id: String,
        error: String,
    },
    /// The TCP connection was dropped mid-stream.
    ConnectionDropped {
        session_id: String,
        error: String,
    },
    /// A per-chunk or overall timeout was exceeded.
    Timeout {
        session_id: String,
    },
}

impl StreamEvent {
    /// Returns the session_id associated with this event.
    pub fn session_id(&self) -> &str {
        match self {
            StreamEvent::Chunk { session_id, .. } => session_id,
            StreamEvent::Completed { session_id, .. } => session_id,
            StreamEvent::ParseError { session_id, .. } => session_id,
            StreamEvent::ConnectionDropped { session_id, .. } => session_id,
            StreamEvent::Timeout { session_id } => session_id,
        }
    }
}
