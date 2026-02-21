use crate::content_analysis::ContentAnalysisResult;
use crate::screens::history::SortColumn;
use crate::stream::StreamEvent;
use crate::types::Session;
use std::collections::HashSet;

/// Root message enum for the iced application.
#[derive(Debug, Clone)]
pub enum Message {
    // Navigation
    NavigateToChat(String),
    NavigateToSessionList,
    NavigateToNewChat,
    NavigateToExport,
    NavigateToSettings,
    NavigateToHistory,
    NavigateToAnalysis,

    // Chat screen
    ChatInputChanged(String),
    SendMessage,
    StreamEventReceived(StreamEvent),
    CancelStream,
    ToggleBlink,

    // Session list
    SessionSelected(String),
    DeleteSession(String),
    RefreshSessions,

    // Export
    ExportRequested,
    ExportCompleted(Result<String, String>),
    ExportToggleSession(String),
    ExportSelectAll,
    ExportDeselectAll,

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
    AnalysisSelectLeft(String),
    AnalysisSelectRight(String),
    AnalysisResultReady {
        score: f64,
        shared: HashSet<String>,
        left_only: HashSet<String>,
        right_only: HashSet<String>,
    },
    AnalysisCycleFocus,

    // History
    HistorySearchChanged(String),
    HistorySortBy(SortColumn),
    HistoryReverseSort,
    HistorySelectNext,
    HistorySelectPrev,
    HistoryOpenSelected,
    HistoryDeleteSelected,

    // Loading
    ConnectionCheckResult(Result<Vec<String>, String>),
    LoadingComplete,

    // Connection health
    ConnectionHealthCheck,
    ConnectionHealthResult(bool),

    // UI
    DismissChatError,
    DismissError,
    KeyboardEvent(iced::keyboard::Key, iced::keyboard::Modifiers),

    // Content analysis
    ContentAnalysisReady {
        session_id: String,
        turn_index: usize,
        result: Box<ContentAnalysisResult>,
    },
    TurnMetricsSaved(String, Result<(), String>),
    ToggleMetricsSection(String),

    // General
    Tick,
    Noop,
}
