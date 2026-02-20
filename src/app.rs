use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

use crate::message::Message;
use crate::screens::chat::{self, ChatScreen};
use crate::screens::export::{self, ExportScreen};
use crate::screens::session_list::{self, SessionListScreen};
use crate::screens::settings::SettingsScreen;
use crate::storage;
use crate::types::Session;

/// The five screens of the application.
pub enum Screen {
    SessionList,
    Chat,
    NewChat,
    Export,
    Settings,
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
        }
    }

    /// Initialize the database pool and run migrations.
    pub fn init_db(&mut self, db_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let pool = storage::init_pool(db_path)?;
        storage::run_migrations(&pool)?;
        self.pool = Some(pool);
        Ok(())
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
                self.export = Some(ExportScreen::new(self.session_list.sessions.clone()));
                UpdateAction::None
            }
            Message::NavigateToSettings => {
                self.screen = Screen::Settings;
                self.settings.loading_models = true;
                UpdateAction::FetchModels(self.base_url.clone())
            }

            // ── Chat screen messages ────────────────────────────
            Message::ChatInputChanged(input) => {
                // Guard: only process if we're on a chat screen
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
                // Guard: only process if chat exists AND session_id matches
                match self.chat {
                    Some(ref mut chat) if event.session_id() == chat.session_id => {
                        match chat.handle_stream_event(&event) {
                            chat::Action::SaveSession(session) => {
                                UpdateAction::SaveSession(session)
                            }
                            _ => UpdateAction::None,
                        }
                    }
                    _ => UpdateAction::None, // Stale event for wrong/no session
                }
            }
            Message::CancelStream => match self.chat {
                Some(ref mut chat) => match chat.cancel_stream() {
                    chat::Action::CancelStream => UpdateAction::AbortStream,
                    _ => UpdateAction::None,
                },
                None => UpdateAction::None,
            },

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
                // Guard: only apply if we're still expecting this session
                match (&self.screen, &self.chat) {
                    (Screen::Chat, None) | (Screen::Chat, Some(_)) => {
                        match result {
                            Ok(Some(session)) => {
                                // Guard: make sure session_id matches what we requested
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
                    _ => UpdateAction::None, // Stale result, user navigated away
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
                // Could display in UI later
                UpdateAction::None
            }

            // ── UI ──────────────────────────────────────────────
            Message::DismissError => {
                self.error = None;
                UpdateAction::None
            }
            Message::KeyboardEvent(key, modifiers) => {
                use iced::keyboard::{key::Named, Key};
                // Ctrl+N → New Chat
                if modifiers.control() {
                    if let Key::Character(ref c) = key {
                        match c.as_str() {
                            "n" => return self.update(Message::NavigateToNewChat),
                            "e" => return self.update(Message::NavigateToExport),
                            "s" if modifiers.shift() => {
                                return self.update(Message::NavigateToSettings)
                            }
                            _ => {}
                        }
                    }
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

            // ── General ─────────────────────────────────────────
            Message::Tick => UpdateAction::None,
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
    LoadSession(String),
    DeleteSession(String),
    ListSessions,
    ExportSessions(Vec<Session>),
    FetchModels(String),
}
