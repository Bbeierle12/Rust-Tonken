use crate::types::Session;

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
        Self {
            sessions,
            status: ExportStatus::Ready,
            last_export_path: None,
        }
    }

    pub fn start_export(&mut self) -> Action {
        if self.sessions.is_empty() {
            self.status = ExportStatus::Error("No sessions to export".to_string());
            return Action::None;
        }
        self.status = ExportStatus::Exporting;
        Action::ExportToFile(self.sessions.clone())
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
