use ollama_scope::app::{App, Screen, UpdateAction};
use ollama_scope::message::Message;
use ollama_scope::stream::{FinalStreamMetrics, StreamEvent};
use ollama_scope::types::{ChatMessage, Session, SessionMetrics};

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
