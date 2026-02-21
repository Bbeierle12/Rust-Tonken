use iced::{Element, Subscription, Task};
use ollama_scope::app::{App, Screen, UpdateAction};
use ollama_scope::client::stream_chat;
use ollama_scope::export::export_sessions;
use ollama_scope::message::Message;
use ollama_scope::screens::loading::StepStatus;
use ollama_scope::storage;

use futures::StreamExt;
use std::sync::Arc;

struct OllamaScope {
    app: App,
    pool: Option<Arc<r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>>>,
    stream_abort: Option<iced::task::Handle>,
}

impl OllamaScope {
    fn new() -> (Self, Task<Message>) {
        let mut app = App::new();
        app.screen = Screen::Loading;

        // Initialize DB
        let db_path = dirs_db_path();
        let pool = match storage::init_pool(&db_path) {
            Ok(pool) => {
                if let Err(e) = storage::run_migrations(&pool) {
                    app.error = Some(format!("Migration error: {e}"));
                    app.loading.update_step(0, StepStatus::Failed(e.to_string()));
                } else {
                    app.loading.update_step(0, StepStatus::Done);
                }
                Some(Arc::new(pool))
            }
            Err(e) => {
                app.error = Some(format!("DB init error: {e}"));
                app.loading.update_step(0, StepStatus::Failed(e.to_string()));
                None
            }
        };

        // Kick off connection check + model fetch
        let base_url = app.base_url.clone();
        app.loading.update_step(1, StepStatus::InProgress);

        let connection_task = Task::perform(
            async move {
                let url = format!("{base_url}/api/tags");
                let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
                let body: serde_json::Value =
                    resp.json().await.map_err(|e| e.to_string())?;
                let models = body
                    .get("models")
                    .and_then(|m| m.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                            .map(String::from)
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                Ok(models)
            },
            Message::ConnectionCheckResult,
        );

        // Load font
        let font_task = iced::font::load(include_bytes!("../assets/JetBrainsMono-Regular.ttf").as_slice())
            .map(|_| Message::Noop);

        // Load sessions from DB
        let session_task = if let Some(ref p) = pool {
            let pool = Arc::clone(p);
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || storage::list_sessions(&pool))
                        .await
                        .map_err(|e| e.to_string())
                        .and_then(|r| r.map_err(|e| e.to_string()))
                },
                Message::DbSessionsListed,
            )
        } else {
            Task::none()
        };

        let init_task = Task::batch([connection_task, font_task, session_task]);

        (
            Self {
                app,
                pool,
                stream_abort: None,
            },
            init_task,
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        let action = self.app.update(message);
        self.handle_action(action)
    }

    fn handle_action(&mut self, action: UpdateAction) -> Task<Message> {
        match action {
            UpdateAction::None => Task::none(),

            UpdateAction::StartStream(request, session_id) => {
                let base_url = self.app.base_url.clone();
                let sid = session_id.clone();
                let (task, handle) = Task::abortable(Task::stream(
                    stream_chat(&base_url, &request, &sid).map(Message::StreamEventReceived),
                ));
                self.stream_abort = Some(handle);
                task
            }

            UpdateAction::AbortStream => {
                if let Some(handle) = self.stream_abort.take() {
                    handle.abort();
                }
                Task::none()
            }

            UpdateAction::SaveSession(session) => {
                if let Some(ref pool) = self.pool {
                    let pool = Arc::clone(pool);
                    let sid = session.id.clone();
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                storage::save_session(&pool, &session)
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r.map_err(|e| e.to_string()))
                        },
                        move |result| Message::DbSessionSaved(sid.clone(), result),
                    )
                } else {
                    Task::none()
                }
            }

            UpdateAction::LoadSession(session_id) => {
                if let Some(ref pool) = self.pool {
                    let pool = Arc::clone(pool);
                    let sid = session_id.clone();
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                storage::load_session(&pool, &sid)
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r.map_err(|e| e.to_string()))
                        },
                        move |result| Message::DbSessionLoaded(session_id.clone(), result),
                    )
                } else {
                    Task::none()
                }
            }

            UpdateAction::DeleteSession(session_id) => {
                if let Some(ref pool) = self.pool {
                    let pool = Arc::clone(pool);
                    let sid = session_id.clone();
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || {
                                storage::delete_session(&pool, &sid)
                            })
                            .await
                            .map_err(|e| e.to_string())
                            .and_then(|r| r.map_err(|e| e.to_string()))
                        },
                        move |result| Message::DbSessionDeleted(session_id.clone(), result),
                    )
                } else {
                    Task::none()
                }
            }

            UpdateAction::ListSessions => {
                if let Some(ref pool) = self.pool {
                    let pool = Arc::clone(pool);
                    Task::perform(
                        async move {
                            tokio::task::spawn_blocking(move || storage::list_sessions(&pool))
                                .await
                                .map_err(|e| e.to_string())
                                .and_then(|r| r.map_err(|e| e.to_string()))
                        },
                        Message::DbSessionsListed,
                    )
                } else {
                    Task::none()
                }
            }

            UpdateAction::ExportSessions(sessions) => Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                        let filename = format!("ollama_export_{timestamp}.csv");
                        let file = std::fs::File::create(&filename)
                            .map_err(|e| e.to_string())?;
                        export_sessions(&sessions, file).map_err(|e| e.to_string())?;
                        Ok(filename)
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r)
                },
                Message::ExportCompleted,
            ),

            UpdateAction::CheckConnection(base_url) => {
                Task::perform(
                    async move {
                        let url = format!("{base_url}/api/tags");
                        let resp = reqwest::get(&url).await.map_err(|_| ())?;
                        let _body: serde_json::Value =
                            resp.json().await.map_err(|_| ())?;
                        Ok::<bool, ()>(true)
                    },
                    |result| Message::ConnectionHealthResult(result.unwrap_or(false)),
                )
            }

            UpdateAction::ComputeAnalysis { left_text, right_text } => {
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            let pipeline = ollama_scope::analysis::TextPipeline::new(1);
                            let set_a = pipeline.process(&left_text);
                            let set_b = pipeline.process(&right_text);
                            let shared: std::collections::HashSet<String> =
                                set_a.intersection(&set_b).cloned().collect();
                            let left_only: std::collections::HashSet<String> =
                                set_a.difference(&set_b).cloned().collect();
                            let right_only: std::collections::HashSet<String> =
                                set_b.difference(&set_a).cloned().collect();
                            let union_count = set_a.union(&set_b).count();
                            let score = if union_count == 0 {
                                1.0
                            } else {
                                shared.len() as f64 / union_count as f64
                            };
                            (score, shared, left_only, right_only)
                        })
                        .await
                        .unwrap()
                    },
                    |(score, shared, left_only, right_only)| Message::AnalysisResultReady {
                        score,
                        shared,
                        left_only,
                        right_only,
                    },
                )
            }

            UpdateAction::FetchModels(base_url) => {
                Task::perform(
                    async move {
                        let url = format!("{base_url}/api/tags");
                        let resp = reqwest::get(&url).await.map_err(|e| e.to_string())?;
                        let body: serde_json::Value =
                            resp.json().await.map_err(|e| e.to_string())?;
                        let models = body
                            .get("models")
                            .and_then(|m| m.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                                    .map(String::from)
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();
                        Ok(models)
                    },
                    Message::ModelsLoaded,
                )
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        self.app.view()
    }

    fn subscription(&self) -> Subscription<Message> {
        let keyboard = iced::keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyboardEvent(key, modifiers))
        });

        let is_streaming = self.app.is_streaming();

        if is_streaming {
            // During streaming: keyboard + tick (500ms) + blink (530ms)
            let tick = iced::time::every(std::time::Duration::from_millis(500))
                .map(|_| Message::Tick);
            let blink = iced::time::every(std::time::Duration::from_millis(530))
                .map(|_| Message::ToggleBlink);
            Subscription::batch([keyboard, tick, blink])
        } else {
            // When idle: keyboard + periodic health check (30s)
            let health = iced::time::every(std::time::Duration::from_secs(30))
                .map(|_| Message::ConnectionHealthCheck);
            Subscription::batch([keyboard, health])
        }
    }
}

fn dirs_db_path() -> String {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("ollama-scope");
    std::fs::create_dir_all(&dir).ok();
    dir.join("sessions.db")
        .to_string_lossy()
        .to_string()
}

fn main() -> iced::Result {
    iced::application("Ollama Scope", OllamaScope::update, OllamaScope::view)
        .subscription(OllamaScope::subscription)
        .theme(|_| iced::Theme::Dark)
        .window_size(iced::Size::new(1200.0, 800.0))
        .run_with(OllamaScope::new)
}
