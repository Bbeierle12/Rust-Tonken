use iced::widget::{button, column, container, row, scrollable, text, text_input, Column, Row};
use iced::{Element, Length};

use crate::app::{App, Screen};
use crate::message::Message;
use crate::screens::chat::ChatState;
use crate::screens::export::ExportStatus;

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = match self.screen {
            Screen::SessionList => self.view_session_list(),
            Screen::Chat | Screen::NewChat => self.view_chat(),
            Screen::Export => self.view_export(),
            Screen::Settings => self.view_settings(),
        };

        let error_bar: Element<'_, Message> = if let Some(ref err) = self.error {
            row![
                text(format!("Error: {err}")).size(14).width(Length::Fill),
                button(text("Dismiss").size(12)).on_press(Message::DismissError),
            ]
            .spacing(8)
            .padding(8)
            .into()
        } else {
            column![].into()
        };

        column![error_bar, row![sidebar, content].height(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let nav_buttons = column![
            button("Sessions")
                .on_press(Message::NavigateToSessionList)
                .width(Length::Fill),
            button("New Chat")
                .on_press(Message::NavigateToNewChat)
                .width(Length::Fill),
            button("Export")
                .on_press(Message::NavigateToExport)
                .width(Length::Fill),
            button("Settings")
                .on_press(Message::NavigateToSettings)
                .width(Length::Fill),
        ]
        .spacing(8)
        .padding(12)
        .width(180);

        container(nav_buttons)
            .height(Length::Fill)
            .into()
    }

    fn view_session_list(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(4).padding(16);

        if self.session_list.loading {
            list = list.push(text("Loading sessions..."));
        } else if self.session_list.sessions.is_empty() {
            list = list.push(text("No sessions yet. Start a new chat!"));
        } else {
            for session in &self.session_list.sessions {
                let sid = session.id.clone();
                let sid_delete = session.id.clone();
                let session_row = row![
                    button(text(&session.title).size(14))
                        .on_press(Message::SessionSelected(sid))
                        .width(Length::Fill),
                    button(text("X").size(12))
                        .on_press(Message::DeleteSession(sid_delete)),
                ]
                .spacing(4);
                list = list.push(session_row);
            }
        }

        container(scrollable(list))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(8)
            .into()
    }

    fn view_chat(&self) -> Element<'_, Message> {
        match &self.chat {
            None => container(text("Loading chat..."))
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16)
                .into(),
            Some(chat) => {
                // Messages display
                let mut messages_col = Column::new().spacing(8).padding(16);

                for msg in &chat.messages {
                    let label = if msg.role == "user" { "You" } else { "AI" };
                    messages_col = messages_col.push(
                        column![
                            text(format!("{label}:")).size(12),
                            text(&msg.content).size(14),
                        ]
                        .spacing(2),
                    );
                }

                // Show streaming content
                if !chat.streaming_content.is_empty() {
                    messages_col = messages_col.push(
                        column![
                            text("AI:").size(12),
                            text(&chat.streaming_content).size(14),
                        ]
                        .spacing(2),
                    );
                }

                // Metrics display
                let metrics_row = row![
                    text(format!("TPS: {:.1}", chat.metrics.tps)).size(12),
                    text(format!("TTFT: {:.0}ms", chat.metrics.ttft_ms)).size(12),
                    text(format!(
                        "Tokens: {}",
                        chat.metrics.prompt_tokens + chat.metrics.completion_tokens
                    ))
                    .size(12),
                ]
                .spacing(16);

                // Input area
                let mut input_row = Row::new().spacing(8);

                let input = text_input("Type a message...", &chat.input)
                    .on_input(Message::ChatInputChanged)
                    .on_submit(Message::SendMessage)
                    .width(Length::Fill);

                input_row = input_row.push(input);

                match chat.state {
                    ChatState::Idle => {
                        input_row =
                            input_row.push(button("Send").on_press(Message::SendMessage));
                    }
                    ChatState::Streaming => {
                        input_row =
                            input_row.push(button("Cancel").on_press(Message::CancelStream));
                    }
                    ChatState::Error(ref err) => {
                        input_row = input_row
                            .push(text(format!("Error: {err}")).size(12));
                        input_row =
                            input_row.push(button("Send").on_press(Message::SendMessage));
                    }
                }

                // Status indicator
                let status = match chat.state {
                    ChatState::Idle => text("Ready").size(12),
                    ChatState::Streaming => text("Streaming...").size(12),
                    ChatState::Error(ref e) => text(format!("Error: {e}")).size(12),
                };

                column![
                    text(format!("{} — {}", chat.title, chat.model)).size(18),
                    scrollable(messages_col).height(Length::Fill),
                    metrics_row,
                    status,
                    input_row,
                ]
                .spacing(8)
                .padding(16)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
        }
    }

    fn view_export(&self) -> Element<'_, Message> {
        match &self.export {
            None => container(text("No export data"))
                .width(Length::Fill)
                .padding(16)
                .into(),
            Some(export_screen) => {
                let status_text = match &export_screen.status {
                    ExportStatus::Ready => "Ready to export".to_string(),
                    ExportStatus::Exporting => "Exporting...".to_string(),
                    ExportStatus::Done(path) => format!("Exported to: {path}"),
                    ExportStatus::Error(e) => format!("Error: {e}"),
                };

                column![
                    text("Export Sessions to CSV").size(20),
                    text(format!(
                        "{} session(s) available",
                        export_screen.sessions.len()
                    ))
                    .size(14),
                    text(status_text).size(14),
                    button("Export CSV").on_press(Message::ExportRequested),
                ]
                .spacing(12)
                .padding(16)
                .width(Length::Fill)
                .into()
            }
        }
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let url_input = text_input("Ollama base URL", &self.settings.base_url)
            .on_input(Message::BaseUrlChanged)
            .width(400);

        let mut models_col = Column::new().spacing(4);
        if self.settings.loading_models {
            models_col = models_col.push(text("Loading models...").size(14));
        } else {
            for model in &self.settings.available_models {
                let m = model.clone();
                models_col = models_col.push(
                    button(text(model).size(14))
                        .on_press(Message::ModelSelected(m)),
                );
            }
        }

        column![
            text("Settings").size(20),
            text("Ollama API URL:").size(14),
            url_input,
            text(format!("Current model: {}", self.settings.selected_model)).size(14),
            text("Available models:").size(14),
            models_col,
        ]
        .spacing(12)
        .padding(16)
        .width(Length::Fill)
        .into()
    }
}
