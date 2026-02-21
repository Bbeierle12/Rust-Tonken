use ollama_scope::app::{App, Screen, UpdateAction};
use ollama_scope::message::Message;
use ollama_scope::screens::history::SortColumn;
use ollama_scope::stream::{FinalStreamMetrics, StreamEvent};
use ollama_scope::types::{ChatMessage, ConnectionStatus, Session, SessionMetrics};

fn test_app() -> App {
    App::new()
}

fn sample_session(id: &str) -> Session {
    Session {
        id: id.to_string(),
        title: "Test".to_string(),
        model: "llama3".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: "Hello".to_string(),
        }],
        metrics: SessionMetrics::default(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    }
}

fn sample_session_with_metrics(id: &str, title: &str, model: &str, tps: f64, ttft: f64) -> Session {
    Session {
        id: id.to_string(),
        title: title.to_string(),
        model: model.to_string(),
        messages: vec![
            ChatMessage { role: "user".to_string(), content: "Hello".to_string() },
            ChatMessage { role: "assistant".to_string(), content: "Hi there".to_string() },
        ],
        metrics: SessionMetrics {
            tps,
            ttft_ms: ttft,
            ..SessionMetrics::default()
        },
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
    }
}

fn app_with_sessions() -> App {
    let mut app = App::new();
    app.session_list.set_sessions(vec![
        sample_session_with_metrics("s1", "Chat about Rust", "llama3", 25.0, 100.0),
        sample_session_with_metrics("s2", "Python basics", "mistral", 30.0, 80.0),
        sample_session_with_metrics("s3", "JavaScript tips", "llama3", 20.0, 150.0),
    ]);
    app
}

// ── Screen transitions ──────────────────────────

#[test]
fn test_navigate_to_session_list() {
    let mut app = test_app();
    let action = app.update(Message::NavigateToSessionList);
    assert!(matches!(app.screen, Screen::SessionList));
    assert!(matches!(action, UpdateAction::ListSessions));
}

#[test]
fn test_navigate_to_new_chat() {
    let mut app = test_app();
    let action = app.update(Message::NavigateToNewChat);
    assert!(matches!(app.screen, Screen::NewChat));
    assert!(matches!(action, UpdateAction::None));
    assert!(app.chat.is_some());
}

#[test]
fn test_navigate_to_chat_loads_session() {
    let mut app = test_app();
    let action = app.update(Message::NavigateToChat("s1".to_string()));
    assert!(matches!(app.screen, Screen::Chat));
    assert!(matches!(action, UpdateAction::LoadSession(ref id) if id == "s1"));
}

#[test]
fn test_navigate_to_export() {
    let mut app = test_app();
    app.update(Message::NavigateToExport);
    assert!(matches!(app.screen, Screen::Export));
    assert!(app.export.is_some());
}

#[test]
fn test_navigate_to_settings() {
    let mut app = test_app();
    app.update(Message::NavigateToSettings);
    assert!(matches!(app.screen, Screen::Settings));
}

// ── Guard clauses ──────────────────────────────

#[test]
fn test_chat_input_without_chat_is_noop() {
    let mut app = test_app();
    // No chat screen active
    let action = app.update(Message::ChatInputChanged("hello".to_string()));
    assert!(matches!(action, UpdateAction::None));
}

#[test]
fn test_send_message_without_chat_is_noop() {
    let mut app = test_app();
    let action = app.update(Message::SendMessage);
    assert!(matches!(action, UpdateAction::None));
}

#[test]
fn test_stream_event_for_wrong_session_is_dropped() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);
    let _chat_sid = app.chat.as_ref().unwrap().session_id.clone();

    // Send event for a different session
    let event = StreamEvent::Chunk {
        session_id: "wrong-session".to_string(),
        chunk: ollama_scope::types::OllamaChatChunk {
            model: "llama3".to_string(),
            created_at: "".to_string(),
            message: Some(ollama_scope::types::ChunkMessage {
                role: "assistant".to_string(),
                content: "data".to_string(),
            }),
            done: false,
            total_duration: None,
            load_duration: None,
            prompt_eval_count: None,
            prompt_eval_duration: None,
            eval_count: None,
            eval_duration: None,
        },
    };
    let action = app.update(Message::StreamEventReceived(event));
    assert!(matches!(action, UpdateAction::None));

    // Verify the chat didn't change
    assert!(app.chat.as_ref().unwrap().streaming_content.is_empty());
}

#[test]
fn test_stale_db_result_when_navigated_away() {
    let mut app = test_app();
    // Navigate to chat
    app.update(Message::NavigateToChat("s1".to_string()));
    // Then navigate away to session list
    app.update(Message::NavigateToSessionList);

    // Now a stale DB result arrives
    let action = app.update(Message::DbSessionLoaded(
        "s1".to_string(),
        Ok(Some(sample_session("s1"))),
    ));
    // Should be ignored since we're not on the Chat screen anymore
    assert!(matches!(action, UpdateAction::None));
}

// ── Stream event routing ───────────────────────

#[test]
fn test_stream_completed_triggers_save() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);

    // Type and send a message
    let chat = app.chat.as_mut().unwrap();
    chat.update_input("Hello".to_string());
    let sid = chat.session_id.clone();
    app.update(Message::SendMessage);

    // Simulate stream completion
    let event = StreamEvent::Completed {
        session_id: sid,
        metrics: FinalStreamMetrics {
            total_duration: 1_000_000_000,
            load_duration: 100_000_000,
            prompt_eval_count: 5,
            prompt_eval_duration: 500_000_000,
            eval_count: 10,
            eval_duration: 400_000_000,
        },
    };
    let action = app.update(Message::StreamEventReceived(event));
    assert!(matches!(action, UpdateAction::SaveSession(_)));
}

// ── DB result handling ─────────────────────────

#[test]
fn test_db_session_loaded_success() {
    let mut app = test_app();
    app.update(Message::NavigateToChat("s1".to_string()));

    let action = app.update(Message::DbSessionLoaded(
        "s1".to_string(),
        Ok(Some(sample_session("s1"))),
    ));
    assert!(matches!(action, UpdateAction::None));
    assert!(app.chat.is_some());
    assert_eq!(app.chat.as_ref().unwrap().session_id, "s1");
}

#[test]
fn test_db_session_loaded_not_found() {
    let mut app = test_app();
    app.update(Message::NavigateToChat("s1".to_string()));

    app.update(Message::DbSessionLoaded("s1".to_string(), Ok(None)));
    // Should navigate back to session list
    assert!(matches!(app.screen, Screen::SessionList));
    assert!(app.error.is_some());
}

#[test]
fn test_db_sessions_listed_success() {
    let mut app = test_app();
    let sessions = vec![sample_session("s1"), sample_session("s2")];
    app.update(Message::DbSessionsListed(Ok(sessions)));
    assert_eq!(app.session_list.sessions.len(), 2);
    assert!(!app.session_list.loading);
}

#[test]
fn test_db_sessions_listed_error() {
    let mut app = test_app();
    app.update(Message::DbSessionsListed(Err("DB error".to_string())));
    assert!(app.error.is_some());
    assert!(app.session_list.sessions.is_empty());
}

// ── Settings ───────────────────────────────────

#[test]
fn test_base_url_change() {
    let mut app = test_app();
    app.update(Message::BaseUrlChanged("http://example.com:11434".to_string()));
    assert_eq!(app.base_url, "http://example.com:11434");
    assert_eq!(app.settings.base_url, "http://example.com:11434");
}

#[test]
fn test_model_selection() {
    let mut app = test_app();
    app.update(Message::ModelSelected("mistral".to_string()));
    assert_eq!(app.selected_model, "mistral");
}

// ── Delete session ─────────────────────────────

#[test]
fn test_delete_session_from_list() {
    let mut app = test_app();
    app.session_list.set_sessions(vec![sample_session("s1"), sample_session("s2")]);

    let action = app.update(Message::DeleteSession("s1".to_string()));
    assert!(matches!(action, UpdateAction::DeleteSession(ref id) if id == "s1"));
    assert_eq!(app.session_list.sessions.len(), 1);
}

#[test]
fn test_noop_message() {
    let mut app = test_app();
    let action = app.update(Message::Noop);
    assert!(matches!(action, UpdateAction::None));
}

#[test]
fn test_tick_message() {
    let mut app = test_app();
    let action = app.update(Message::Tick);
    assert!(matches!(action, UpdateAction::None));
}

// ── Phase 7: Polish tests ──────────────────────

#[test]
fn test_dismiss_error() {
    let mut app = test_app();
    app.error = Some("test error".to_string());
    app.update(Message::DismissError);
    assert!(app.error.is_none());
}

#[test]
fn test_navigate_to_settings_fetches_models() {
    let mut app = test_app();
    let action = app.update(Message::NavigateToSettings);
    assert!(matches!(app.screen, Screen::Settings));
    assert!(matches!(action, UpdateAction::FetchModels(_)));
    assert!(app.settings.loading_models);
}

#[test]
fn test_models_loaded_success() {
    let mut app = test_app();
    app.update(Message::ModelsLoaded(Ok(vec![
        "llama3".to_string(),
        "mistral".to_string(),
    ])));
    assert_eq!(app.settings.available_models.len(), 2);
    assert!(!app.settings.loading_models);
}

#[test]
fn test_models_loaded_error() {
    let mut app = test_app();
    app.update(Message::ModelsLoaded(Err("connection refused".to_string())));
    assert!(app.error.is_some());
}

#[test]
fn test_session_auto_title_from_first_message() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);

    let chat = app.chat.as_mut().unwrap();
    chat.update_input("What is the meaning of life?".to_string());
    chat.send_message();

    assert_eq!(chat.title, "What is the meaning of life?");
}

#[test]
fn test_keyboard_escape_navigates_to_session_list() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);

    app.update(Message::KeyboardEvent(
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
        iced::keyboard::Modifiers::empty(),
    ));

    assert!(matches!(app.screen, Screen::SessionList));
}

// ── New screen navigation ─────────────────────

#[test]
fn test_navigate_to_history() {
    let mut app = app_with_sessions();
    let action = app.update(Message::NavigateToHistory);
    assert!(matches!(app.screen, Screen::History));
    assert!(matches!(action, UpdateAction::None));
    assert!(app.history.is_some());
}

#[test]
fn test_navigate_to_analysis() {
    let mut app = test_app();
    let action = app.update(Message::NavigateToAnalysis);
    assert!(matches!(app.screen, Screen::Analysis));
    assert!(matches!(action, UpdateAction::None));
    assert!(app.analysis_screen.is_some());
}

// ── History interactions ──────────────────────

#[test]
fn test_history_sort_by_column() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToHistory);
    app.update(Message::HistorySortBy(SortColumn::Tps));
    let history = app.history.as_ref().unwrap();
    assert_eq!(history.sort_column, SortColumn::Tps);
}

#[test]
fn test_history_search() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToHistory);
    app.update(Message::HistorySearchChanged("Rust".to_string()));
    let history = app.history.as_ref().unwrap();
    let filtered = history.filtered_sessions();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].title, "Chat about Rust");
}

#[test]
fn test_history_arrow_navigation() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToHistory);

    // Select first item
    app.update(Message::HistorySelectNext);
    assert_eq!(app.history.as_ref().unwrap().selected_index, Some(0));

    // Move down
    app.update(Message::HistorySelectNext);
    assert_eq!(app.history.as_ref().unwrap().selected_index, Some(1));

    // Move up
    app.update(Message::HistorySelectPrev);
    assert_eq!(app.history.as_ref().unwrap().selected_index, Some(0));
}

#[test]
fn test_history_open_selected() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToHistory);
    app.update(Message::HistorySelectNext);

    let action = app.update(Message::HistoryOpenSelected);
    assert!(matches!(app.screen, Screen::Chat));
    assert!(matches!(action, UpdateAction::LoadSession(_)));
}

#[test]
fn test_history_reverse_sort() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToHistory);
    let initial_dir = app.history.as_ref().unwrap().sort_direction;
    app.update(Message::HistoryReverseSort);
    assert_ne!(app.history.as_ref().unwrap().sort_direction, initial_dir);
}

// ── Analysis interactions ─────────────────────

#[test]
fn test_analysis_select_sessions() {
    let mut app = test_app();
    app.update(Message::NavigateToAnalysis);
    app.update(Message::AnalysisSelectLeft("s1".to_string()));
    app.update(Message::AnalysisSelectRight("s2".to_string()));

    let analysis = app.analysis_screen.as_ref().unwrap();
    assert_eq!(analysis.left_session_id, Some("s1".to_string()));
    assert_eq!(analysis.right_session_id, Some("s2".to_string()));
    assert!(analysis.is_ready());
}

#[test]
fn test_analysis_result_ready() {
    let mut app = test_app();
    app.update(Message::NavigateToAnalysis);

    let shared: std::collections::HashSet<String> = ["term1".to_string()].into();
    let left: std::collections::HashSet<String> = ["left1".to_string()].into();
    let right: std::collections::HashSet<String> = ["right1".to_string()].into();

    app.update(Message::AnalysisResultReady {
        score: 0.75,
        shared: shared.clone(),
        left_only: left.clone(),
        right_only: right.clone(),
    });

    let analysis = app.analysis_screen.as_ref().unwrap();
    assert_eq!(analysis.similarity_score, Some(0.75));
    assert_eq!(analysis.shared_terms.len(), 1);
}

#[test]
fn test_analysis_cycle_focus() {
    use ollama_scope::screens::analysis::AnalysisFocus;
    let mut app = test_app();
    app.update(Message::NavigateToAnalysis);

    assert_eq!(app.analysis_screen.as_ref().unwrap().focus, AnalysisFocus::LeftPicker);
    app.update(Message::AnalysisCycleFocus);
    assert_eq!(app.analysis_screen.as_ref().unwrap().focus, AnalysisFocus::RightPicker);
    app.update(Message::AnalysisCycleFocus);
    assert_eq!(app.analysis_screen.as_ref().unwrap().focus, AnalysisFocus::Results);
    app.update(Message::AnalysisCycleFocus);
    assert_eq!(app.analysis_screen.as_ref().unwrap().focus, AnalysisFocus::LeftPicker);
}

// ── Export interactions ───────────────────────

#[test]
fn test_export_toggle_session() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToExport);

    // All selected by default
    let export = app.export.as_ref().unwrap();
    assert_eq!(export.selected_ids.len(), 3);

    // Toggle one off
    app.update(Message::ExportToggleSession("s1".to_string()));
    assert_eq!(app.export.as_ref().unwrap().selected_ids.len(), 2);

    // Toggle back on
    app.update(Message::ExportToggleSession("s1".to_string()));
    assert_eq!(app.export.as_ref().unwrap().selected_ids.len(), 3);
}

#[test]
fn test_export_select_all() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToExport);

    app.update(Message::ExportDeselectAll);
    assert_eq!(app.export.as_ref().unwrap().selected_ids.len(), 0);

    app.update(Message::ExportSelectAll);
    assert_eq!(app.export.as_ref().unwrap().selected_ids.len(), 3);
}

#[test]
fn test_export_deselect_all() {
    let mut app = app_with_sessions();
    app.update(Message::NavigateToExport);

    app.update(Message::ExportDeselectAll);
    assert!(app.export.as_ref().unwrap().selected_ids.is_empty());
}

// ── Keyboard shortcuts ───────────────────────

#[test]
fn test_keyboard_ctrl_h_navigates_to_history() {
    let mut app = test_app();
    app.update(Message::KeyboardEvent(
        iced::keyboard::Key::Character("h".into()),
        iced::keyboard::Modifiers::CTRL,
    ));
    assert!(matches!(app.screen, Screen::History));
}

#[test]
fn test_keyboard_ctrl_a_navigates_to_analysis() {
    let mut app = test_app();
    app.update(Message::KeyboardEvent(
        iced::keyboard::Key::Character("a".into()),
        iced::keyboard::Modifiers::CTRL,
    ));
    assert!(matches!(app.screen, Screen::Analysis));
}

#[test]
fn test_keyboard_ctrl_1_navigates_to_session_list() {
    let mut app = test_app();
    app.update(Message::NavigateToHistory);
    app.update(Message::KeyboardEvent(
        iced::keyboard::Key::Character("1".into()),
        iced::keyboard::Modifiers::CTRL,
    ));
    assert!(matches!(app.screen, Screen::SessionList));
}

// ── Connection health ────────────────────────

#[test]
fn test_connection_health_check() {
    let mut app = test_app();
    let action = app.update(Message::ConnectionHealthCheck);
    assert!(matches!(action, UpdateAction::CheckConnection(_)));
    assert_eq!(app.connection_status, ConnectionStatus::Checking);
}

#[test]
fn test_connection_health_result_connected() {
    let mut app = test_app();
    app.update(Message::ConnectionHealthResult(true));
    assert_eq!(app.connection_status, ConnectionStatus::Connected);
}

#[test]
fn test_connection_health_result_disconnected() {
    let mut app = test_app();
    app.update(Message::ConnectionHealthResult(false));
    assert_eq!(app.connection_status, ConnectionStatus::Disconnected);
}

// ── Tick and blink ───────────────────────────

#[test]
fn test_tick_updates_tps_samples_during_streaming() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);

    // Start streaming
    let chat = app.chat.as_mut().unwrap();
    chat.update_input("Hello".to_string());
    app.update(Message::SendMessage);

    // Tick while streaming
    let action = app.update(Message::Tick);
    assert!(matches!(action, UpdateAction::None));
    // Should have recorded at least one TPS sample
    assert!(!app.chat.as_ref().unwrap().tps_samples.is_empty());
}

#[test]
fn test_blink_toggle() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);

    let initial = app.chat.as_ref().unwrap().blink_visible;
    app.update(Message::ToggleBlink);
    assert_ne!(app.chat.as_ref().unwrap().blink_visible, initial);
    app.update(Message::ToggleBlink);
    assert_eq!(app.chat.as_ref().unwrap().blink_visible, initial);
}

// ── Loading flow ─────────────────────────────

#[test]
fn test_connection_check_result_success_transitions() {
    let mut app = test_app();
    app.screen = Screen::Loading;

    // Mark DB step as done first
    app.loading.update_step(0, ollama_scope::screens::loading::StepStatus::Done);

    let action = app.update(Message::ConnectionCheckResult(Ok(vec!["llama3".to_string()])));
    // Loading should be complete, should transition to SessionList
    assert!(matches!(app.screen, Screen::SessionList));
    assert!(matches!(action, UpdateAction::ListSessions));
    assert_eq!(app.connection_status, ConnectionStatus::Connected);
}

#[test]
fn test_connection_check_result_failure() {
    let mut app = test_app();
    app.screen = Screen::Loading;
    app.loading.update_step(0, ollama_scope::screens::loading::StepStatus::Done);

    let action = app.update(Message::ConnectionCheckResult(Err("refused".to_string())));
    assert_eq!(app.connection_status, ConnectionStatus::Disconnected);
    // Should still transition since all steps resolved (even if failed)
    assert!(matches!(app.screen, Screen::SessionList));
}

#[test]
fn test_loading_complete_navigates_to_session_list() {
    let mut app = test_app();
    app.screen = Screen::Loading;
    let action = app.update(Message::LoadingComplete);
    assert!(matches!(app.screen, Screen::SessionList));
    assert!(matches!(action, UpdateAction::ListSessions));
}

// ── is_streaming helper ──────────────────────

#[test]
fn test_is_streaming_false_when_idle() {
    let app = test_app();
    assert!(!app.is_streaming());
}

#[test]
fn test_is_streaming_true_during_stream() {
    let mut app = test_app();
    app.update(Message::NavigateToNewChat);
    let chat = app.chat.as_mut().unwrap();
    chat.update_input("Test".to_string());
    chat.send_message();
    assert!(app.is_streaming());
}
