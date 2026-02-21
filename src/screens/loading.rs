/// Diagnostic step status during loading.
#[derive(Debug, Clone, PartialEq)]
pub enum StepStatus {
    Pending,
    InProgress,
    Done,
    Failed(String),
}

/// A single diagnostic step shown on the loading screen.
#[derive(Debug, Clone)]
pub struct DiagnosticStep {
    pub label: String,
    pub status: StepStatus,
}

/// State for the loading/startup screen.
pub struct LoadingScreen {
    pub steps: Vec<DiagnosticStep>,
    pub models: Vec<String>,
}

impl LoadingScreen {
    pub fn new() -> Self {
        Self {
            steps: vec![
                DiagnosticStep {
                    label: "Database initialization".to_string(),
                    status: StepStatus::Pending,
                },
                DiagnosticStep {
                    label: "Connection to Ollama".to_string(),
                    status: StepStatus::Pending,
                },
                DiagnosticStep {
                    label: "Loading models".to_string(),
                    status: StepStatus::Pending,
                },
            ],
            models: Vec::new(),
        }
    }

    /// Update the status of a step by index.
    pub fn update_step(&mut self, index: usize, status: StepStatus) {
        if let Some(step) = self.steps.get_mut(index) {
            step.status = status;
        }
    }

    /// Set the loaded models list.
    pub fn set_models(&mut self, models: Vec<String>) {
        self.models = models;
    }

    /// Check if all steps are done (or failed but non-blocking).
    pub fn is_ready(&self) -> bool {
        self.steps.iter().all(|s| {
            matches!(s.status, StepStatus::Done | StepStatus::Failed(_))
        })
    }
}

impl Default for LoadingScreen {
    fn default() -> Self {
        Self::new()
    }
}
