use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::message::Message;
use crate::screens::analysis::AnalysisScreen;
use crate::screens::chat::{self, ChatScreen};
use crate::screens::export::{self, ExportScreen};
use crate::screens::history::HistoryScreen;
use crate::screens::loading::LoadingScreen;
use crate::screens::session_list::{self, SessionListScreen};
use crate::screens::settings::SettingsScreen;
use crate::storage;
use crate::types::{ConnectionStatus, Session};

/// The screens of the application.
pub enum Screen {
    Loading,
    SessionList,
    Chat,
    NewChat,
    Export,
    Settings,
    History,
    Analysis,
}

/// Root application state.
pub struct App {
    pub screen: Screen,
    pub session_list: SessionListScreen,
    pub chat: Option<ChatScreen>,
    pub export: Option<ExportScreen>,
    pub settings: SettingsScreen,
    pub base_url: String,
    pub selected_model: String,
    pub pool: Option<Pool<SqliteConnectionManager>>,
    pub error: Option<String>,
    // New fields
    pub loading: LoadingScreen,
    pub history: Option<HistoryScreen>,
    pub analysis_screen: Option<AnalysisScreen>,
    pub connection_status: ConnectionStatus,
}

impl App {
    pub fn new() -> Self {
        Self {
            screen: Screen::SessionList,
            session_list: SessionListScreen::new(),
            chat: None,
            export: None,
            settings: SettingsScreen::new(
                "http://localhost:11434".to_string(),
                "llama3".to_string(),
            ),
            base_url: "http://localhost:11434".to_string(),
            selected_model: "llama3".to_string(),
            pool: None,
            error: None,
            loading: LoadingScreen::new(),
            history: None,
            analysis_screen: None,
            connection_status: ConnectionStatus::Unknown,
        }
    }

    /// Initialize the database pool and run migrations.
    pub fn init_db(&mut self, db_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = storage::init_pool(db_path)?;
        storage::run_migrations(&pool)?;
        self.pool = Some(pool);
        Ok(())
    }

    /// Helper: join all messages from a session into a single string for analysis.
    pub fn session_text(&self, id: &str) -> Option<String> {
        self.session_list
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| {
                s.messages
                    .iter()
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
    }

    /// Helper: is the chat currently streaming?
    pub fn is_streaming(&self) -> bool {
        self.chat
            .as_ref()
            .map(|c| c.state == chat::ChatState::Streaming)
            .unwrap_or(false)
    }

    /// Process a message and return an optional follow-up action description.
    /// Returns a description of what task the parent should spawn, if any.
    pub fn update(&mut self, message: Message) -> UpdateAction {
        match message {
            // ── Navigation ──────────────────────────────────────
            Message::NavigateToSessionList => {
                self.screen = Screen::SessionList;
                self.session_list.loading = true;
                UpdateAction::ListSessions
            }
            Message::NavigateToChat(session_id) => {
                self.screen = Screen::Chat;
                UpdateAction::LoadSession(session_id)
            }
            Message::NavigateToNewChat => {
                let id = uuid::Uuid::new_v4().to_string();
                self.chat = Some(ChatScreen::new(id, self.selected_model.clone()));
                self.screen = Screen::NewChat;
                UpdateAction::None
            }
            Message::NavigateToExport => {
                self.screen = Screen::Export;
                let mut export_screen = ExportScreen::new(self.session_list.sessions.clone());
                export_screen.generate_preview();
                self.export = Some(export_screen);
                UpdateAction::None
            }
            Message::NavigateToSettings => {
                self.screen = Screen::Settings;
                self.settings.loading_models = true;
                UpdateAction::FetchModels(self.base_url.clone())
            }
            Message::NavigateToHistory => {
                self.history = Some(HistoryScreen::new(self.session_list.sessions.clone()));
                self.screen = Screen::History;
                UpdateAction::None
            }
            Message::NavigateToAnalysis => {
                self.analysis_screen = Some(AnalysisScreen::new());
                self.screen = Screen::Analysis;
                UpdateAction::None
            }

            // ── Chat screen messages ────────────────────────────
            Message::ChatInputChanged(input) => {
                match self.chat {
                    Some(ref mut chat) => {
                        chat.update_input(input);
                        UpdateAction::None
                    }
                    None => UpdateAction::None,
                }
            }
            Message::SendMessage => match self.chat {
                Some(ref mut chat) => match chat.send_message() {
                    chat::Action::SendRequest(request, session_id) => {
                        UpdateAction::StartStream(request, session_id)
                    }
                    _ => UpdateAction::None,
                },
                None => UpdateAction::None,
            },
            Message::StreamEventReceived(event) => {
                match self.chat {
                    Some(ref mut chat) if event.session_id() == chat.session_id => {
                        match chat.handle_stream_event(&event) {
                            chat::Action::SaveSession(session) => {
                                UpdateAction::SaveSession(session)
                            }
                            chat::Action::SaveSessionAndAnalyze {
                                session,
                                turn_index,
                                user_text,
                                assistant_text,
                                first_assistant_text,
                                previous_assistant_text,
                            } => UpdateAction::SaveSessionAndAnalyze {
                                session,
                                turn_index,
                                user_text,
                                assistant_text,
                                first_assistant_text,
                                previous_assistant_text,
                            },
                            _ => UpdateAction::None,
                        }
                    }
                    _ => UpdateAction::None,
                }
            }
            Message::CancelStream => match self.chat {
                Some(ref mut chat) => match chat.cancel_stream() {
                    chat::Action::CancelStream => UpdateAction::AbortStream,
                    _ => UpdateAction::None,
                },
                None => UpdateAction::None,
            },
            Message::ToggleBlink => {
                if let Some(ref mut chat) = self.chat {
                    chat.toggle_blink();
                }
                UpdateAction::None
            }

            // ── Session list messages ───────────────────────────
            Message::SessionSelected(id) => {
                match self.session_list.select_session(&id) {
                    session_list::Action::LoadSession(id) => {
                        self.screen = Screen::Chat;
                        UpdateAction::LoadSession(id)
                    }
                    _ => UpdateAction::None,
                }
            }
            Message::DeleteSession(id) => {
                self.session_list.delete_session(&id);
                if let Some(ref mut history) = self.history {
                    history.remove_session(&id);
                }
                UpdateAction::DeleteSession(id)
            }
            Message::RefreshSessions => {
                self.session_list.refresh();
                UpdateAction::ListSessions
            }

            // ── Export messages ──────────────────────────────────
            Message::ExportRequested => match self.export {
                Some(ref mut export_screen) => match export_screen.start_export() {
                    export::Action::ExportToFile(sessions) => {
                        UpdateAction::ExportSessions(sessions)
                    }
                    _ => UpdateAction::None,
                },
                None => UpdateAction::None,
            },
            Message::ExportCompleted(result) => {
                if let Some(ref mut export_screen) = self.export {
                    export_screen.export_completed(result);
                }
                UpdateAction::None
            }
            Message::ExportToggleSession(id) => {
                if let Some(ref mut export_screen) = self.export {
                    export_screen.toggle_session(&id);
                    export_screen.generate_preview();
                }
                UpdateAction::None
            }
            Message::ExportSelectAll => {
                if let Some(ref mut export_screen) = self.export {
                    export_screen.select_all();
                    export_screen.generate_preview();
                }
                UpdateAction::None
            }
            Message::ExportDeselectAll => {
                if let Some(ref mut export_screen) = self.export {
                    export_screen.deselect_all();
                    export_screen.generate_preview();
                }
                UpdateAction::None
            }

            // ── Settings messages ───────────────────────────────
            Message::BaseUrlChanged(url) => {
                self.settings.update_base_url(url.clone());
                self.base_url = url;
                UpdateAction::None
            }
            Message::ModelSelected(model) => {
                self.settings.select_model(model.clone());
                self.selected_model = model;
                UpdateAction::None
            }

            // ── DB result messages ──────────────────────────────
            Message::DbSessionLoaded(session_id, result) => {
                match (&self.screen, &self.chat) {
                    (Screen::Chat, None) | (Screen::Chat, Some(_)) => {
                        match result {
                            Ok(Some(session)) => {
                                if session.id == session_id {
                                    self.chat = Some(ChatScreen::from_session(session));
                                }
                                UpdateAction::None
                            }
                            Ok(None) => {
                                self.error = Some(format!("Session {session_id} not found"));
                                self.screen = Screen::SessionList;
                                UpdateAction::None
                            }
                            Err(e) => {
                                self.error = Some(e);
                                UpdateAction::None
                            }
                        }
                    }
                    _ => UpdateAction::None,
                }
            }
            Message::DbSessionSaved(session_id, result) => {
                if let Err(e) = result {
                    self.error = Some(format!("Failed to save session {session_id}: {e}"));
                }
                UpdateAction::None
            }
            Message::DbSessionsListed(result) => {
                match result {
                    Ok(sessions) => {
                        self.session_list.set_sessions(sessions);
                    }
                    Err(e) => {
                        self.error = Some(format!("Failed to list sessions: {e}"));
                        self.session_list.set_sessions(Vec::new());
                    }
                }
                UpdateAction::None
            }
            Message::DbSessionDeleted(session_id, result) => {
                if let Err(e) = result {
                    self.error = Some(format!("Failed to delete session {session_id}: {e}"));
                }
                UpdateAction::None
            }

            // ── Model management ────────────────────────────────
            Message::ModelsLoaded(result) => {
                match result {
                    Ok(models) => self.settings.set_models(models),
                    Err(e) => self.error = Some(format!("Failed to load models: {e}")),
                }
                UpdateAction::None
            }

            // ── Analysis ────────────────────────────────────────
            Message::SimilarityComputed(_session_id, _score) => {
                UpdateAction::None
            }
            Message::AnalysisSelectLeft(id) => {
                if let Some(ref mut analysis) = self.analysis_screen {
                    analysis.select_left(id);
                    if analysis.is_ready() {
                        let left_id = analysis.left_session_id.as_ref().unwrap().clone();
                        let right_id = analysis.right_session_id.as_ref().unwrap().clone();
                        if let (Some(left_text), Some(right_text)) =
                            (self.session_text(&left_id), self.session_text(&right_id))
                        {
                            return UpdateAction::ComputeAnalysis { left_text, right_text };
                        }
                    }
                }
                UpdateAction::None
            }
            Message::AnalysisSelectRight(id) => {
                if let Some(ref mut analysis) = self.analysis_screen {
                    analysis.select_right(id);
                    if analysis.is_ready() {
                        let left_id = analysis.left_session_id.as_ref().unwrap().clone();
                        let right_id = analysis.right_session_id.as_ref().unwrap().clone();
                        if let (Some(left_text), Some(right_text)) =
                            (self.session_text(&left_id), self.session_text(&right_id))
                        {
                            return UpdateAction::ComputeAnalysis { left_text, right_text };
                        }
                    }
                }
                UpdateAction::None
            }
            Message::AnalysisResultReady { score, shared, left_only, right_only } => {
                if let Some(ref mut analysis) = self.analysis_screen {
                    analysis.set_result(score, shared, left_only, right_only);
                }
                UpdateAction::None
            }
            Message::AnalysisCycleFocus => {
                if let Some(ref mut analysis) = self.analysis_screen {
                    analysis.cycle_focus();
                }
                UpdateAction::None
            }

            // ── History ─────────────────────────────────────────
            Message::HistorySearchChanged(query) => {
                if let Some(ref mut history) = self.history {
                    history.search(query);
                }
                UpdateAction::None
            }
            Message::HistorySortBy(column) => {
                if let Some(ref mut history) = self.history {
                    history.sort_by(column);
                }
                UpdateAction::None
            }
            Message::HistoryReverseSort => {
                if let Some(ref mut history) = self.history {
                    history.reverse_sort();
                }
                UpdateAction::None
            }
            Message::HistorySelectNext => {
                if let Some(ref mut history) = self.history {
                    history.select_next();
                }
                UpdateAction::None
            }
            Message::HistorySelectPrev => {
                if let Some(ref mut history) = self.history {
                    history.select_prev();
                }
                UpdateAction::None
            }
            Message::HistoryOpenSelected => {
                if let Some(ref history) = self.history {
                    if let Some(id) = history.selected_session_id() {
                        let id = id.to_string();
                        return self.update(Message::NavigateToChat(id));
                    }
                }
                UpdateAction::None
            }
            Message::HistoryDeleteSelected => {
                if let Some(ref history) = self.history {
                    if let Some(id) = history.selected_session_id() {
                        let id = id.to_string();
                        return self.update(Message::DeleteSession(id));
                    }
                }
                UpdateAction::None
            }

            // ── Loading ─────────────────────────────────────────
            Message::ConnectionCheckResult(result) => {
                match result {
                    Ok(models) => {
                        self.connection_status = ConnectionStatus::Connected;
                        self.loading.update_step(1, crate::screens::loading::StepStatus::Done);
                        self.loading.set_models(models.clone());
                        self.settings.set_models(models);
                        self.loading.update_step(2, crate::screens::loading::StepStatus::Done);
                    }
                    Err(e) => {
                        self.connection_status = ConnectionStatus::Disconnected;
                        self.loading.update_step(1, crate::screens::loading::StepStatus::Failed(e.clone()));
                        self.loading.update_step(2, crate::screens::loading::StepStatus::Failed(e));
                    }
                }
                if self.loading.is_ready() {
                    return self.update(Message::LoadingComplete);
                }
                UpdateAction::None
            }
            Message::LoadingComplete => {
                self.screen = Screen::SessionList;
                self.session_list.loading = true;
                UpdateAction::ListSessions
            }

            // ── Connection health ───────────────────────────────
            Message::ConnectionHealthCheck => {
                self.connection_status = ConnectionStatus::Checking;
                UpdateAction::CheckConnection(self.base_url.clone())
            }
            Message::ConnectionHealthResult(ok) => {
                self.connection_status = if ok {
                    ConnectionStatus::Connected
                } else {
                    ConnectionStatus::Disconnected
                };
                UpdateAction::None
            }

            // ── UI ──────────────────────────────────────────────
            Message::DismissChatError => {
                if let Some(ref mut chat) = self.chat {
                    chat.state = chat::ChatState::Idle;
                }
                UpdateAction::None
            }
            Message::DismissError => {
                self.error = None;
                UpdateAction::None
            }
            Message::KeyboardEvent(key, modifiers) => {
                use iced::keyboard::{key::Named, Key};

                if modifiers.control() {
                    if let Key::Character(ref c) = key {
                        match c.as_str() {
                            "n" => return self.update(Message::NavigateToNewChat),
                            "e" if !modifiers.shift() => return self.update(Message::NavigateToExport),
                            "s" if modifiers.shift() => {
                                return self.update(Message::NavigateToSettings)
                            }
                            "h" => return self.update(Message::NavigateToHistory),
                            "a" if !modifiers.shift() => return self.update(Message::NavigateToAnalysis),
                            "1" => return self.update(Message::NavigateToSessionList),
                            "2" => return self.update(Message::NavigateToHistory),
                            "3" => return self.update(Message::NavigateToAnalysis),
                            "4" => return self.update(Message::NavigateToExport),
                            _ => {}
                        }
                    }
                }

                // Screen-specific keyboard handling
                match self.screen {
                    Screen::History => {
                        match key {
                            Key::Named(Named::ArrowDown) => return self.update(Message::HistorySelectNext),
                            Key::Named(Named::ArrowUp) => return self.update(Message::HistorySelectPrev),
                            Key::Named(Named::Enter) => return self.update(Message::HistoryOpenSelected),
                            Key::Named(Named::Delete) => return self.update(Message::HistoryDeleteSelected),
                            Key::Character(ref c) => match c.as_str() {
                                "r" if !modifiers.control() => return self.update(Message::HistoryReverseSort),
                                _ => {}
                            }
                            _ => {}
                        }
                    }
                    Screen::Analysis => {
                        if key == Key::Named(Named::Tab) {
                            return self.update(Message::AnalysisCycleFocus);
                        }
                    }
                    Screen::Export => {
                        if let Key::Character(ref c) = key {
                            if c.as_str() == "e" && modifiers.control() && modifiers.shift() {
                                return self.update(Message::ExportRequested);
                            }
                            if c.as_str() == "a" && modifiers.control() && modifiers.shift() {
                                return self.update(Message::ExportSelectAll);
                            }
                        }
                        if key == Key::Named(Named::Space) {
                            // Space toggles selection — handled in view, not here
                        }
                    }
                    _ => {}
                }

                // Escape → Cancel stream or navigate back
                if key == Key::Named(Named::Escape) {
                    if matches!(self.screen, Screen::Chat | Screen::NewChat) {
                        if let Some(ref chat) = self.chat {
                            if chat.state == crate::screens::chat::ChatState::Streaming {
                                return self.update(Message::CancelStream);
                            }
                        }
                    }
                    return self.update(Message::NavigateToSessionList);
                }
                UpdateAction::None
            }

            // ── Content analysis results ─────────────────────
            Message::ContentAnalysisReady { session_id, turn_index, result } => {
                if let Some(ref mut chat) = self.chat {
                    if chat.session_id == session_id {
                        if let Some(turn) = chat.metrics.turn_metrics.get_mut(turn_index) {
                            // Merge content analysis into the turn metrics
                            turn.sentiment_score = result.sentiment_score;
                            turn.user_sentiment_score = result.user_sentiment_score;
                            turn.dominant_emotion = result.dominant_emotion.clone();
                            turn.emotion_counts = result.emotion_counts.clone();
                            turn.emotional_range = result.emotional_range;
                            turn.reading_level = result.reading_level;
                            turn.avg_sentence_length = result.avg_sentence_length;
                            turn.avg_word_length = result.avg_word_length;
                            turn.type_token_ratio = result.type_token_ratio;
                            turn.hapax_percentage = result.hapax_percentage;
                            turn.lexical_density = result.lexical_density;
                            turn.response_amplification = result.response_amplification;
                            turn.question_density = result.question_density;
                            turn.hedging_index = result.hedging_index;
                            turn.code_density = result.code_density;
                            turn.list_density = result.list_density;
                            turn.topic_similarity_prev = result.topic_similarity_prev;
                            turn.topic_similarity_first = result.topic_similarity_first;
                            turn.formality_score = result.formality_score;
                            turn.repetition_index = result.repetition_index;
                            turn.instructional_density = result.instructional_density;
                            turn.certainty_score = result.certainty_score;

                            chat.content_analysis_pending = false;

                            let turn_clone = turn.clone();
                            return UpdateAction::SaveTurnMetrics {
                                session_id,
                                turn_metrics: turn_clone,
                            };
                        }
                    }
                }
                UpdateAction::None
            }
            Message::TurnMetricsSaved(session_id, result) => {
                if let Err(e) = result {
                    self.error = Some(format!("Failed to save turn metrics for {session_id}: {e}"));
                }
                UpdateAction::None
            }
            Message::ToggleMetricsSection(section) => {
                if let Some(ref mut chat) = self.chat {
                    if chat.metrics_collapsed.contains(&section) {
                        chat.metrics_collapsed.remove(&section);
                    } else {
                        chat.metrics_collapsed.insert(section);
                    }
                }
                UpdateAction::None
            }

            // ── General ─────────────────────────────────────────
            Message::Tick => {
                // Update TPS samples during streaming
                if let Some(ref mut chat) = self.chat {
                    if chat.state == chat::ChatState::Streaming {
                        let now = std::time::Instant::now();
                        let (tps, count) = chat.token_session
                            .as_ref()
                            .map(|ts| (ts.tps(now), ts.token_count()))
                            .unwrap_or((0.0, 0));
                        chat.record_tps_sample(tps);
                        chat.chunk_count = count;
                    }
                }
                UpdateAction::None
            }
            Message::Noop => UpdateAction::None,
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Actions the app requests the iced runtime to perform.
#[derive(Debug)]
pub enum UpdateAction {
    None,
    StartStream(crate::types::OllamaChatRequest, String),
    AbortStream,
    SaveSession(Session),
    SaveSessionAndAnalyze {
        session: Session,
        turn_index: usize,
        user_text: String,
        assistant_text: String,
        first_assistant_text: String,
        previous_assistant_text: Option<String>,
    },
    SaveTurnMetrics {
        session_id: String,
        turn_metrics: crate::types::TurnMetrics,
    },
    LoadSession(String),
    DeleteSession(String),
    ListSessions,
    ExportSessions(Vec<Session>),
    FetchModels(String),
    CheckConnection(String),
    ComputeAnalysis { left_text: String, right_text: String },
}
