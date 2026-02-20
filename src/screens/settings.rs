/// Actions the settings screen requests from the parent.
#[derive(Debug)]
pub enum Action {
    None,
    UpdateBaseUrl(String),
    LoadModels,
}

/// State for the settings screen.
pub struct SettingsScreen {
    pub base_url: String,
    pub available_models: Vec<String>,
    pub selected_model: String,
    pub loading_models: bool,
}

impl SettingsScreen {
    pub fn new(base_url: String, selected_model: String) -> Self {
        Self {
            base_url,
            available_models: Vec::new(),
            selected_model,
            loading_models: false,
        }
    }

    pub fn update_base_url(&mut self, url: String) -> Action {
        self.base_url = url.clone();
        Action::UpdateBaseUrl(url)
    }

    pub fn set_models(&mut self, models: Vec<String>) {
        self.available_models = models;
        self.loading_models = false;
    }

    pub fn select_model(&mut self, model: String) -> Action {
        self.selected_model = model;
        Action::None
    }

    pub fn load_models(&mut self) -> Action {
        self.loading_models = true;
        Action::LoadModels
    }
}
