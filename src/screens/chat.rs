use crate::metrics::TokenSession;
use crate::stream::StreamEvent;
use crate::types::{ChatMessage, OllamaChatRequest, Session, SessionMetrics};
use std::time::Instant;

/// The current state of the streaming process.
#[derive(Debug, Clone, PartialEq)]
pub enum ChatState {
    Idle,
    Streaming,
    Error(String),
}

/// Actions that the chat screen requests the parent to perform.
#[derive(Debug)]
pub enum Action {
    None,
    SendRequest(OllamaChatRequest, String),
    SaveSession(Session),
    CancelStream,
}

/// State for the active chat screen.
pub struct ChatScreen {
    pub session_id: String,
    pub model: String,
    pub title: String,
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub state: ChatState,
    pub metrics: SessionMetrics,
    pub token_session: Option<TokenSession>,
    pub streaming_content: String,
    pub created_at: String,
    pub updated_at: String,
}

impl ChatScreen {
    pub fn new(session_id: String, model: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            session_id,
            model,
            title: "New Chat".to_string(),
            messages: Vec::new(),
            input: String::new(),
            state: ChatState::Idle,
            metrics: SessionMetrics::default(),
            token_session: None,
            streaming_content: String::new(),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn from_session(session: Session) -> Self {
        Self {
            session_id: session.id,
            model: session.model,
            title: session.title,
            messages: session.messages,
            input: String::new(),
            state: ChatState::Idle,
            metrics: session.metrics,
            token_session: None,
            streaming_content: String::new(),
            created_at: session.created_at,
            updated_at: session.updated_at,
        }
    }

    pub fn update_input(&mut self, input: String) {
        self.input = input;
    }

    pub fn send_message(&mut self) -> Action {
        let content = self.input.trim().to_string();
        if content.is_empty() || self.state == ChatState::Streaming {
            return Action::None;
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: content.clone(),
        });

        // Auto-title from first user message
        if self.messages.len() == 1 {
            self.title = content
                .chars()
                .take(50)
                .collect::<String>()
                .trim()
                .to_string();
        }

        self.input.clear();
        self.state = ChatState::Streaming;
        self.streaming_content.clear();
        self.token_session = Some(TokenSession::new(Instant::now()));

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: self.messages.clone(),
            stream: true,
        };

        Action::SendRequest(request, self.session_id.clone())
    }

    pub fn handle_stream_event(&mut self, event: &StreamEvent) -> Action {
        // Guard clause: ignore events for different sessions
        if event.session_id() != self.session_id {
            return Action::None;
        }

        match event {
            StreamEvent::Chunk { chunk, .. } => {
                if let Some(ref msg) = chunk.message {
                    self.streaming_content.push_str(&msg.content);
                    if let Some(ref mut ts) = self.token_session {
                        ts.record_token(Instant::now());
                    }
                }
                Action::None
            }
            StreamEvent::Completed { metrics, .. } => {
                // Finalize the assistant message
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: self.streaming_content.clone(),
                });
                self.streaming_content.clear();
                self.state = ChatState::Idle;

                // Update session metrics
                self.metrics.prompt_tokens += metrics.prompt_eval_count;
                self.metrics.completion_tokens += metrics.eval_count;
                self.metrics.total_duration_nanos += metrics.total_duration;
                self.metrics.eval_duration_nanos += metrics.eval_duration;

                if let Some(ref ts) = self.token_session {
                    let now = Instant::now();
                    self.metrics.tps = ts.tps(now);
                    if let Some(ttft) = ts.ttft() {
                        self.metrics.ttft_ms = ttft;
                    }
                }
                self.token_session = None;
                self.updated_at = chrono::Utc::now().to_rfc3339();

                Action::SaveSession(self.to_session())
            }
            StreamEvent::ParseError { .. } => Action::None,
            StreamEvent::ConnectionDropped { error, .. } => {
                self.state = ChatState::Error(error.clone());
                if !self.streaming_content.is_empty() {
                    self.messages.push(ChatMessage {
                        role: "assistant".to_string(),
                        content: self.streaming_content.clone(),
                    });
                    self.streaming_content.clear();
                }
                self.token_session = None;
                Action::None
            }
            StreamEvent::Timeout { .. } => {
                self.state = ChatState::Error("Request timed out".to_string());
                self.token_session = None;
                Action::None
            }
        }
    }

    pub fn cancel_stream(&mut self) -> Action {
        if self.state == ChatState::Streaming {
            self.state = ChatState::Idle;
            if !self.streaming_content.is_empty() {
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: self.streaming_content.clone(),
                });
                self.streaming_content.clear();
            }
            self.token_session = None;
            Action::CancelStream
        } else {
            Action::None
        }
    }

    pub fn to_session(&self) -> Session {
        Session {
            id: self.session_id.clone(),
            title: self.title.clone(),
            model: self.model.clone(),
            messages: self.messages.clone(),
            metrics: self.metrics.clone(),
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }
}
