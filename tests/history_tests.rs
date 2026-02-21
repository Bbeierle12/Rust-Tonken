use ollama_scope::screens::history::{HistoryScreen, SortColumn, SortDirection};
use ollama_scope::types::{ChatMessage, Session, SessionMetrics};

fn make_session(id: &str, title: &str, model: &str, tps: f64, ttft: f64, date: &str) -> Session {
    Session {
        id: id.to_string(),
        title: title.to_string(),
        model: model.to_string(),
        messages: vec![
            ChatMessage { role: "user".to_string(), content: "Hello".to_string() },
            ChatMessage { role: "assistant".to_string(), content: "Hi".to_string() },
        ],
        metrics: SessionMetrics {
            tps,
            ttft_ms: ttft,
            ..SessionMetrics::default()
        },
        created_at: date.to_string(),
        updated_at: date.to_string(),
    }
}

fn test_sessions() -> Vec<Session> {
    vec![
        make_session("s1", "Alpha chat", "llama3", 25.0, 100.0, "2025-01-01T00:00:00Z"),
        make_session("s2", "Beta analysis", "mistral", 30.0, 80.0, "2025-01-02T00:00:00Z"),
        make_session("s3", "Gamma query", "llama3", 20.0, 150.0, "2025-01-03T00:00:00Z"),
    ]
}

#[test]
fn test_history_new_defaults_to_date_desc() {
    let screen = HistoryScreen::new(test_sessions());
    assert_eq!(screen.sort_column, SortColumn::Date);
    assert_eq!(screen.sort_direction, SortDirection::Desc);
}

#[test]
fn test_history_sort_by_title() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.sort_by(SortColumn::Title);
    let filtered = screen.filtered_sessions();
    // Desc by default when switching columns
    assert_eq!(filtered[0].title, "Gamma query");
}

#[test]
fn test_history_sort_by_tps() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.sort_by(SortColumn::Tps);
    let filtered = screen.filtered_sessions();
    // Desc: highest TPS first
    assert_eq!(filtered[0].id, "s2"); // 30.0 TPS
    assert_eq!(filtered[1].id, "s1"); // 25.0 TPS
    assert_eq!(filtered[2].id, "s3"); // 20.0 TPS
}

#[test]
fn test_history_sort_by_same_column_reverses() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.sort_by(SortColumn::Tps);
    assert_eq!(screen.sort_direction, SortDirection::Desc);
    screen.sort_by(SortColumn::Tps);
    assert_eq!(screen.sort_direction, SortDirection::Asc);
}

#[test]
fn test_history_reverse_sort() {
    let mut screen = HistoryScreen::new(test_sessions());
    assert_eq!(screen.sort_direction, SortDirection::Desc);
    screen.reverse_sort();
    assert_eq!(screen.sort_direction, SortDirection::Asc);
    screen.reverse_sort();
    assert_eq!(screen.sort_direction, SortDirection::Desc);
}

#[test]
fn test_history_search_by_title() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.search("Alpha".to_string());
    let filtered = screen.filtered_sessions();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "s1");
}

#[test]
fn test_history_search_by_model() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.search("mistral".to_string());
    let filtered = screen.filtered_sessions();
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id, "s2");
}

#[test]
fn test_history_search_case_insensitive() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.search("alpha".to_string());
    assert_eq!(screen.filtered_sessions().len(), 1);
}

#[test]
fn test_history_search_empty_shows_all() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.search("Alpha".to_string());
    assert_eq!(screen.filtered_sessions().len(), 1);
    screen.search("".to_string());
    assert_eq!(screen.filtered_sessions().len(), 3);
}

#[test]
fn test_history_search_no_match() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.search("nonexistent".to_string());
    assert_eq!(screen.filtered_sessions().len(), 0);
}

#[test]
fn test_history_select_next() {
    let mut screen = HistoryScreen::new(test_sessions());
    assert_eq!(screen.selected_index, None);
    screen.select_next();
    assert_eq!(screen.selected_index, Some(0));
    screen.select_next();
    assert_eq!(screen.selected_index, Some(1));
    screen.select_next();
    assert_eq!(screen.selected_index, Some(2));
    // Wrap around
    screen.select_next();
    assert_eq!(screen.selected_index, Some(0));
}

#[test]
fn test_history_select_prev() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.select_prev();
    assert_eq!(screen.selected_index, Some(2)); // Wraps to last
    screen.select_prev();
    assert_eq!(screen.selected_index, Some(1));
    screen.select_prev();
    assert_eq!(screen.selected_index, Some(0));
    screen.select_prev();
    assert_eq!(screen.selected_index, Some(2)); // Wraps again
}

#[test]
fn test_history_selected_session_id() {
    let mut screen = HistoryScreen::new(test_sessions());
    assert_eq!(screen.selected_session_id(), None);
    screen.select_next();
    assert!(screen.selected_session_id().is_some());
}

#[test]
fn test_history_search_resets_selection() {
    let mut screen = HistoryScreen::new(test_sessions());
    screen.select_next();
    assert_eq!(screen.selected_index, Some(0));
    screen.search("Alpha".to_string());
    assert_eq!(screen.selected_index, None);
}

#[test]
fn test_history_set_sessions() {
    let mut screen = HistoryScreen::new(test_sessions());
    assert_eq!(screen.filtered_sessions().len(), 3);
    screen.set_sessions(vec![make_session("s4", "New", "llama3", 10.0, 200.0, "2025-01-04T00:00:00Z")]);
    assert_eq!(screen.filtered_sessions().len(), 1);
}

#[test]
fn test_history_empty() {
    let screen = HistoryScreen::new(vec![]);
    assert_eq!(screen.filtered_sessions().len(), 0);
    assert_eq!(screen.selected_session_id(), None);
}

#[test]
fn test_history_select_on_empty_is_noop() {
    let mut screen = HistoryScreen::new(vec![]);
    screen.select_next();
    assert_eq!(screen.selected_index, None);
    screen.select_prev();
    assert_eq!(screen.selected_index, None);
}
