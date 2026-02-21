use crate::types::Session;

/// Which column the history table is sorted by.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortColumn {
    Title,
    Model,
    Tps,
    Ttft,
    Turns,
    Date,
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// State for the history screen.
pub struct HistoryScreen {
    pub sessions: Vec<Session>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub search_query: String,
    pub selected_index: Option<usize>,
    filtered_ids: Vec<String>,
}

impl HistoryScreen {
    pub fn new(sessions: Vec<Session>) -> Self {
        let filtered_ids = sessions.iter().map(|s| s.id.clone()).collect();
        let mut screen = Self {
            sessions,
            sort_column: SortColumn::Date,
            sort_direction: SortDirection::Desc,
            search_query: String::new(),
            selected_index: None,
            filtered_ids,
        };
        screen.apply_sort();
        screen
    }

    /// Update sessions list (e.g. after DB refresh).
    pub fn set_sessions(&mut self, sessions: Vec<Session>) {
        self.sessions = sessions;
        self.apply_filter_and_sort();
    }

    /// Sort by a column. If already sorted by this column, reverse direction.
    pub fn sort_by(&mut self, column: SortColumn) {
        if self.sort_column == column {
            self.reverse_sort();
        } else {
            self.sort_column = column;
            self.sort_direction = SortDirection::Desc;
            self.apply_sort();
        }
    }

    /// Reverse current sort direction.
    pub fn reverse_sort(&mut self) {
        self.sort_direction = match self.sort_direction {
            SortDirection::Asc => SortDirection::Desc,
            SortDirection::Desc => SortDirection::Asc,
        };
        self.apply_sort();
    }

    /// Set search query and re-filter.
    pub fn search(&mut self, query: String) {
        self.search_query = query;
        self.selected_index = None;
        self.apply_filter_and_sort();
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        let count = self.filtered_ids.len();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(i) if i + 1 < count => i + 1,
            Some(_) => 0,
            None => 0,
        });
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        let count = self.filtered_ids.len();
        if count == 0 {
            return;
        }
        self.selected_index = Some(match self.selected_index {
            Some(0) => count - 1,
            Some(i) => i - 1,
            None => count - 1,
        });
    }

    /// Get the session ID of the currently selected row.
    pub fn selected_session_id(&self) -> Option<&str> {
        self.selected_index
            .and_then(|i| self.filtered_ids.get(i))
            .map(|s| s.as_str())
    }

    /// Get filtered sessions in sort order.
    pub fn filtered_sessions(&self) -> Vec<&Session> {
        self.filtered_ids
            .iter()
            .filter_map(|id| self.sessions.iter().find(|s| s.id == *id))
            .collect()
    }

    fn apply_filter_and_sort(&mut self) {
        self.apply_filter();
        self.apply_sort();
    }

    fn apply_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        if query.is_empty() {
            self.filtered_ids = self.sessions.iter().map(|s| s.id.clone()).collect();
        } else {
            self.filtered_ids = self
                .sessions
                .iter()
                .filter(|s| {
                    s.title.to_lowercase().contains(&query)
                        || s.model.to_lowercase().contains(&query)
                })
                .map(|s| s.id.clone())
                .collect();
        }
    }

    fn apply_sort(&mut self) {
        let sessions = &self.sessions;
        let col = self.sort_column;
        let dir = self.sort_direction;

        self.filtered_ids.sort_by(|a_id, b_id| {
            let a = sessions.iter().find(|s| s.id == *a_id);
            let b = sessions.iter().find(|s| s.id == *b_id);
            let (a, b) = match (a, b) {
                (Some(a), Some(b)) => (a, b),
                _ => return std::cmp::Ordering::Equal,
            };

            let ord = match col {
                SortColumn::Title => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
                SortColumn::Model => a.model.to_lowercase().cmp(&b.model.to_lowercase()),
                SortColumn::Tps => a.metrics.tps.partial_cmp(&b.metrics.tps).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Ttft => a.metrics.ttft_ms.partial_cmp(&b.metrics.ttft_ms).unwrap_or(std::cmp::Ordering::Equal),
                SortColumn::Turns => a.messages.len().cmp(&b.messages.len()),
                SortColumn::Date => a.updated_at.cmp(&b.updated_at),
            };

            match dir {
                SortDirection::Asc => ord,
                SortDirection::Desc => ord.reverse(),
            }
        });
    }
}
