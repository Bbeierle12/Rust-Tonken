use crate::types::Session;

/// Actions the session list screen requests from the parent.
#[derive(Debug)]
pub enum Action {
    None,
    LoadSession(String),
    DeleteSession(String),
    RefreshList,
    NavigateToNewChat,
}

/// State for the session list sidebar/screen.
pub struct SessionListScreen {
    pub sessions: Vec<Session>,
    pub loading: bool,
}

impl SessionListScreen {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            loading: true,
        }
    }

    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.sessions = sessions;
        self.loading = false;
    }

    pub fn select_session(&self, id: &str) -> Action {
        if self.sessions.iter().any(|s| s.id == id) {
            Action::LoadSession(id.to_string())
        } else {
            Action::None
        }
    }

    pub fn delete_session(&mut self, id: &str) -> Action {
        self.sessions.retain(|s| s.id != id);
        Action::DeleteSession(id.to_string())
    }

    pub fn refresh(&mut self) -> Action {
        self.loading = true;
        Action::RefreshList
    }
}

impl Default for SessionListScreen {
    fn default() -> Self {
        Self::new()
    }
}
