use crate::types::Session;
use std::collections::HashSet;

/// Actions the export screen requests from the parent.
#[derive(Debug)]
pub enum Action {
    None,
    ExportToFile(Vec<Session>),
}

/// State for the export screen.
pub struct ExportScreen {
    pub sessions: Vec<Session>,
    pub status: ExportStatus,
    pub last_export_path: Option<String>,
    pub selected_ids: HashSet<String>,
    pub preview: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportStatus {
    Ready,
    Exporting,
    Done(String),
    Error(String),
}

impl ExportScreen {
    pub fn new(sessions: Vec<Session>) -> Self {
        let selected_ids = sessions.iter().map(|s| s.id.clone()).collect();
        Self {
            sessions,
            status: ExportStatus::Ready,
            last_export_path: None,
            selected_ids,
            preview: None,
        }
    }

    pub fn start_export(&mut self) -> Action {
        let selected: Vec<Session> = self.selected_sessions();
        if selected.is_empty() {
            self.status = ExportStatus::Error("No sessions selected".to_string());
            return Action::None;
        }
        self.status = ExportStatus::Exporting;
        Action::ExportToFile(selected)
    }

    /// Toggle a session's selection state.
    pub fn toggle_session(&mut self, id: &str) {
        if self.selected_ids.contains(id) {
            self.selected_ids.remove(id);
        } else {
            self.selected_ids.insert(id.to_string());
        }
    }

    /// Select all sessions.
    pub fn select_all(&mut self) {
        self.selected_ids = self.sessions.iter().map(|s| s.id.clone()).collect();
    }

    /// Deselect all sessions.
    pub fn deselect_all(&mut self) {
        self.selected_ids.clear();
    }

    /// Get the list of selected sessions.
    pub fn selected_sessions(&self) -> Vec<Session> {
        self.sessions
            .iter()
            .filter(|s| self.selected_ids.contains(&s.id))
            .cloned()
            .collect()
    }

    /// Generate a CSV preview string.
    pub fn generate_preview(&mut self) {
        let selected = self.selected_sessions();
        if selected.is_empty() {
            self.preview = None;
            return;
        }
        let mut buf = Vec::new();
        if crate::export::export_sessions(&selected, &mut buf).is_ok() {
            self.preview = String::from_utf8(buf).ok();
        }
    }

    pub fn export_completed(&mut self, result: Result<String, String>) {
        match result {
            Ok(path) => {
                self.last_export_path = Some(path.clone());
                self.status = ExportStatus::Done(path);
            }
            Err(e) => {
                self.status = ExportStatus::Error(e);
            }
        }
    }
}
