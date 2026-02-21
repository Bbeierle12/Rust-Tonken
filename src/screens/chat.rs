use crate::metrics::TokenSession;
use crate::stream::StreamEvent;
use crate::types::{ChatMessage, OllamaChatRequest, Session, SessionMetrics, TurnMetrics};
use std::collections::{HashSet, VecDeque};
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
    SaveSessionAndAnalyze {
        session: Session,
        turn_index: usize,
        user_text: String,
        assistant_text: String,
        first_assistant_text: String,
        previous_assistant_text: Option<String>,
    },
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
    // Sparkline / streaming UI fields
    pub tps_samples: VecDeque<f64>,
    pub blink_visible: bool,
    pub chunk_count: u64,
    pub stream_start: Option<Instant>,
    // Content analysis state
    pub content_analysis_pending: bool,
    pub metrics_collapsed: HashSet<String>,
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
            tps_samples: VecDeque::new(),
            blink_visible: true,
            chunk_count: 0,
            stream_start: None,
            content_analysis_pending: false,
            metrics_collapsed: HashSet::new(),
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
            tps_samples: VecDeque::new(),
            blink_visible: true,
            chunk_count: 0,
            stream_start: None,
            content_analysis_pending: false,
            metrics_collapsed: HashSet::new(),
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
        self.tps_samples.clear();
        self.chunk_count = 0;
        self.stream_start = Some(Instant::now());
        self.blink_visible = true;

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
                let assistant_text = self.streaming_content.clone();

                // Finalize the assistant message
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: assistant_text.clone(),
                });
                self.streaming_content.clear();
                self.state = ChatState::Idle;
                self.blink_visible = true;

                // Update session metrics — including previously discarded fields
                self.metrics.prompt_tokens += metrics.prompt_eval_count;
                self.metrics.completion_tokens += metrics.eval_count;
                self.metrics.total_duration_nanos += metrics.total_duration;
                self.metrics.eval_duration_nanos += metrics.eval_duration;
                self.metrics.load_duration_nanos += metrics.load_duration;
                self.metrics.prompt_eval_duration_nanos += metrics.prompt_eval_duration;

                // Compute wall clock time
                let wall_clock_ms = self
                    .stream_start
                    .map(|s| s.elapsed().as_secs_f64() * 1000.0)
                    .unwrap_or(0.0);
                self.metrics.total_wall_clock_ms += wall_clock_ms;

                if let Some(ref ts) = self.token_session {
                    let now = Instant::now();
                    self.metrics.tps = ts.tps(now);
                    if let Some(ttft) = ts.ttft() {
                        self.metrics.ttft_ms = ttft;
                    }
                }

                // Track per-turn histories
                self.metrics.turn_count += 1;
                self.metrics.tps_history.push(self.metrics.tps);
                self.metrics.ttft_history.push(self.metrics.ttft_ms);

                // Build partial TurnMetrics with token/timing data
                let turn_index = self.metrics.turn_count as usize - 1;
                let turn = TurnMetrics {
                    turn_index,
                    prompt_tokens: metrics.prompt_eval_count,
                    completion_tokens: metrics.eval_count,
                    total_duration_nanos: metrics.total_duration,
                    eval_duration_nanos: metrics.eval_duration,
                    load_duration_nanos: metrics.load_duration,
                    prompt_eval_duration_nanos: metrics.prompt_eval_duration,
                    tps: self.metrics.tps,
                    ttft_ms: self.metrics.ttft_ms,
                    wall_clock_ms,
                    ..TurnMetrics::default()
                };
                self.metrics.turn_metrics.push(turn);

                self.token_session = None;
                self.updated_at = chrono::Utc::now().to_rfc3339();
                self.content_analysis_pending = true;

                // Gather text for content analysis
                let user_text = self
                    .messages
                    .iter()
                    .rev()
                    .find(|m| m.role == "user")
                    .map(|m| m.content.clone())
                    .unwrap_or_default();

                let first_assistant_text = self
                    .messages
                    .iter()
                    .find(|m| m.role == "assistant")
                    .map(|m| m.content.clone())
                    .unwrap_or_default();

                // Previous assistant text (the one before the current one)
                let assistant_msgs: Vec<&ChatMessage> = self
                    .messages
                    .iter()
                    .filter(|m| m.role == "assistant")
                    .collect();
                let previous_assistant_text = if assistant_msgs.len() >= 2 {
                    Some(assistant_msgs[assistant_msgs.len() - 2].content.clone())
                } else {
                    None
                };

                Action::SaveSessionAndAnalyze {
                    session: self.to_session(),
                    turn_index,
                    user_text,
                    assistant_text,
                    first_assistant_text,
                    previous_assistant_text,
                }
            }
            StreamEvent::ParseError { .. } => Action::None,
            StreamEvent::ConnectionDropped { error, .. } => {
                self.state = ChatState::Error(error.clone());
                self.blink_visible = true;
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
                self.blink_visible = true;
                self.token_session = None;
                Action::None
            }
        }
    }

    pub fn cancel_stream(&mut self) -> Action {
        if self.state == ChatState::Streaming {
            self.state = ChatState::Idle;
            self.blink_visible = true;
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

    /// Record a TPS sample, keeping a rolling window of 60 values.
    pub fn record_tps_sample(&mut self, tps: f64) {
        self.tps_samples.push_back(tps);
        if self.tps_samples.len() > 60 {
            self.tps_samples.pop_front();
        }
    }

    /// Toggle the cursor blink state.
    pub fn toggle_blink(&mut self) {
        self.blink_visible = !self.blink_visible;
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
