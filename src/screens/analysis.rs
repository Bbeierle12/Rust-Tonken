use std::collections::HashSet;

/// Which picker has focus in the analysis screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnalysisFocus {
    LeftPicker,
    RightPicker,
    Results,
}

/// State for the analysis/comparison screen.
pub struct AnalysisScreen {
    pub left_session_id: Option<String>,
    pub right_session_id: Option<String>,
    pub similarity_score: Option<f64>,
    pub shared_terms: Vec<String>,
    pub left_only_terms: Vec<String>,
    pub right_only_terms: Vec<String>,
    pub focus: AnalysisFocus,
}

impl AnalysisScreen {
    pub fn new() -> Self {
        Self {
            left_session_id: None,
            right_session_id: None,
            similarity_score: None,
            shared_terms: Vec::new(),
            left_only_terms: Vec::new(),
            right_only_terms: Vec::new(),
            focus: AnalysisFocus::LeftPicker,
        }
    }

    /// Select the left session for comparison.
    pub fn select_left(&mut self, session_id: String) {
        self.left_session_id = Some(session_id);
        self.clear_results();
    }

    /// Select the right session for comparison.
    pub fn select_right(&mut self, session_id: String) {
        self.right_session_id = Some(session_id);
        self.clear_results();
    }

    /// Check if both sessions are selected and ready for analysis.
    pub fn is_ready(&self) -> bool {
        self.left_session_id.is_some() && self.right_session_id.is_some()
    }

    /// Store the analysis result.
    pub fn set_result(
        &mut self,
        score: f64,
        shared: HashSet<String>,
        left_only: HashSet<String>,
        right_only: HashSet<String>,
    ) {
        self.similarity_score = Some(score);
        self.shared_terms = shared.into_iter().collect();
        self.shared_terms.sort();
        self.left_only_terms = left_only.into_iter().collect();
        self.left_only_terms.sort();
        self.right_only_terms = right_only.into_iter().collect();
        self.right_only_terms.sort();
    }

    /// Cycle focus: LeftPicker → RightPicker → Results → LeftPicker.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            AnalysisFocus::LeftPicker => AnalysisFocus::RightPicker,
            AnalysisFocus::RightPicker => AnalysisFocus::Results,
            AnalysisFocus::Results => AnalysisFocus::LeftPicker,
        };
    }

    fn clear_results(&mut self) {
        self.similarity_score = None;
        self.shared_terms.clear();
        self.left_only_terms.clear();
        self.right_only_terms.clear();
    }
}

impl Default for AnalysisScreen {
    fn default() -> Self {
        Self::new()
    }
}
