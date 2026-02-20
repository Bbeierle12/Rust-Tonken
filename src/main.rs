use iced::{Element, Subscription, Task};
use ollama_scope::app::{App, UpdateAction};
use ollama_scope::client::stream_chat;
use ollama_scope::export::export_sessions;
use ollama_scope::message::Message;
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

        // Initialize DB
        let db_path = dirs_db_path();
        let pool = match storage::init_pool(&db_path) {
            Ok(pool) => {
                if let Err(e) = storage::run_migrations(&pool) {
                    app.error = Some(format!("Migration error: {e}"));
                }
                Some(Arc::new(pool))
            }
            Err(e) => {
                app.error = Some(format!("DB init error: {e}"));
                None
            }
        };

        let init_task = if let Some(ref p) = pool {
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
        iced::keyboard::on_key_press(|key, modifiers| {
            Some(Message::KeyboardEvent(key, modifiers))
        })
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
        .run_with(OllamaScope::new)
}
