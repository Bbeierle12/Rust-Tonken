use crate::stream::StreamEvent;
use crate::types::Session;

/// Root message enum for the iced application.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    NavigateToChat(String),
    NavigateToSessionList,
    NavigateToNewChat,
    NavigateToExport,
    NavigateToSettings,

    // Chat screen
    ChatInputChanged(String),
    SendMessage,
    StreamEventReceived(StreamEvent),
    CancelStream,

    // Session list
    SessionSelected(String),
    DeleteSession(String),
    RefreshSessions,

    // Export
    ExportRequested,
    ExportCompleted(Result<String, String>),

    // Settings
    BaseUrlChanged(String),

    // DB results (from spawn_blocking)
    DbSessionLoaded(String, Result<Option<Session>, String>),
    DbSessionSaved(String, Result<(), String>),
    DbSessionsListed(Result<Vec<Session>, String>),
    DbSessionDeleted(String, Result<(), String>),

    // Model management
    ModelsLoaded(Result<Vec<String>, String>),
    ModelSelected(String),

    // Analysis
    SimilarityComputed(String, f64),

    // UI
    DismissError,
    KeyboardEvent(iced::keyboard::Key, iced::keyboard::Modifiers),

    // General
    Tick,
    Noop,
}
