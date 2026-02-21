use iced::widget::{
    button, column, container, horizontal_space, row, scrollable, text, text_input, Column, Row,
    Space,
};
use iced::{Element, Length};

use crate::app::{App, Screen};
use crate::message::Message;
use crate::screens::chat::ChatState;
use crate::screens::export::ExportStatus;
use crate::screens::history::SortColumn;
use crate::screens::loading::StepStatus;
use crate::theme;
use crate::types::ConnectionStatus;

impl App {
    // ── Root layout ──────────────────────────────────
    pub fn view(&self) -> Element<'_, Message> {
        // Loading screen bypasses the standard layout
        if matches!(self.screen, Screen::Loading) {
            return self.view_loading();
        }

        let status_bar = self.view_status_bar();
        let sidebar = self.view_sidebar();
        let content = match self.screen {
            Screen::SessionList => self.view_session_list(),
            Screen::Chat | Screen::NewChat => self.view_chat(),
            Screen::Export => self.view_export(),
            Screen::Settings => self.view_settings(),
            Screen::History => self.view_history(),
            Screen::Analysis => self.view_analysis(),
            Screen::Loading => unreachable!(),
        };

        // Metrics panel only on Chat/NewChat
        let main_row = if matches!(self.screen, Screen::Chat | Screen::NewChat) {
            let metrics = self.view_metrics_panel();
            row![sidebar, content, metrics].height(Length::Fill)
        } else {
            row![sidebar, content].height(Length::Fill)
        };

        let shortcut_bar = self.view_shortcut_bar();

        // Error banner if present
        let error_banner: Element<'_, Message> = if let Some(ref err) = self.error {
            container(
                row![
                    text(format!(" {err}"))
                        .size(13)
                        .color(theme::METRIC_ERROR),
                    horizontal_space(),
                    button(text("Retry").size(11).color(theme::TEXT_PRIMARY))
                        .style(theme::flat_button_style())
                        .on_press(Message::ConnectionHealthCheck),
                    button(text("Dismiss").size(11).color(theme::TEXT_PRIMARY))
                        .style(theme::flat_button_style())
                        .on_press(Message::DismissError),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center),
            )
            .padding([4, 12])
            .style(theme::error_banner_style())
            .width(Length::Fill)
            .into()
        } else {
            Space::new(0, 0).into()
        };

        column![error_banner, status_bar, main_row, shortcut_bar]
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    // ── Status bar (28px) ────────────────────────────
    fn view_status_bar(&self) -> Element<'_, Message> {
        let dot_color = match self.connection_status {
            ConnectionStatus::Connected => theme::STATUS_CONNECTED,
            ConnectionStatus::Disconnected => theme::STATUS_DISCONNECTED,
            ConnectionStatus::Checking => theme::STATUS_STREAMING,
            ConnectionStatus::Unknown => theme::TEXT_MUTED,
        };

        let dot = text("\u{25CF}").size(8).color(dot_color); // ● circle
        let model = text(&self.selected_model)
            .size(12)
            .color(theme::TEXT_SECONDARY);
        let session_count = text(format!(
            "{} sessions",
            self.session_list.sessions.len()
        ))
        .size(12)
        .color(theme::TEXT_MUTED);

        let mut status_row = Row::new()
            .spacing(12)
            .align_y(iced::Alignment::Center)
            .push(dot)
            .push(model)
            .push(session_count);

        // Show streaming status if active
        if self.is_streaming() {
            if let Some(ref chat) = self.chat {
                let elapsed = chat.stream_start
                    .map(|s| s.elapsed().as_secs_f64())
                    .unwrap_or(0.0);
                status_row = status_row.push(
                    text(format!("Streaming {:.1}s", elapsed))
                        .size(12)
                        .color(theme::STATUS_STREAMING),
                );
            }
        }

        container(status_row.push(horizontal_space()))
            .padding([0, 16])
            .height(theme::STATUS_BAR_HEIGHT)
            .width(Length::Fill)
            .style(theme::status_bar_style())
            .center_y(theme::STATUS_BAR_HEIGHT)
            .into()
    }

    // ── Sidebar (220px) ──────────────────────────────
    fn view_sidebar(&self) -> Element<'_, Message> {
        let title = text("ollama-scope")
            .size(16)
            .color(theme::TEXT_PRIMARY);

        let new_btn = button(
            row![
                text("+").size(14).color(theme::TEXT_ACCENT),
                text(" New Session").size(13).color(theme::TEXT_PRIMARY),
            ]
            .align_y(iced::Alignment::Center),
        )
        .style(theme::flat_button_style())
        .on_press(Message::NavigateToNewChat)
        .width(Length::Fill);

        let nav_items = column![
            nav_button("Chat", matches!(self.screen, Screen::SessionList), Message::NavigateToSessionList),
            nav_button("History", matches!(self.screen, Screen::History), Message::NavigateToHistory),
            nav_button("Analysis", matches!(self.screen, Screen::Analysis), Message::NavigateToAnalysis),
            nav_button("Export", matches!(self.screen, Screen::Export), Message::NavigateToExport),
            nav_button("Settings", matches!(self.screen, Screen::Settings), Message::NavigateToSettings),
        ]
        .spacing(2);

        // Recent sessions
        let mut sessions_col = Column::new().spacing(2);
        let recent_header = text("RECENT SESSIONS")
            .size(10)
            .color(theme::TEXT_MUTED);
        sessions_col = sessions_col.push(container(recent_header).padding([6, 0]));

        let active_session_id = self.chat.as_ref().map(|c| c.session_id.as_str());

        for session in self.session_list.sessions.iter().take(20) {
            let is_active = active_session_id == Some(session.id.as_str())
                && matches!(self.screen, Screen::Chat | Screen::NewChat);
            let sid = session.id.clone();

            let title_text = text(truncate_str(&session.title, 24))
                .size(13)
                .color(theme::TEXT_PRIMARY);
            let detail = text(truncate_str(&session.model, 20))
                .size(11)
                .color(theme::TEXT_MUTED);

            let entry = button(column![title_text, detail].spacing(1))
                .style(theme::session_entry_style(is_active))
                .on_press(Message::SessionSelected(sid))
                .width(Length::Fill)
                .padding([4, 8]);

            sessions_col = sessions_col.push(entry);
        }

        let sidebar_content = column![
            container(column![title, new_btn].spacing(8)).padding([12, 12]),
            container(nav_items).padding([0, 8]),
            scrollable(sessions_col.padding([0, 8])).height(Length::Fill),
        ]
        .spacing(4);

        container(sidebar_content)
            .width(theme::SIDEBAR_WIDTH)
            .height(Length::Fill)
            .style(theme::sidebar_style())
            .into()
    }

    // ── Shortcut bar (24px) ──────────────────────────
    fn view_shortcut_bar(&self) -> Element<'_, Message> {
        let hints = match self.screen {
            Screen::SessionList => "Ctrl+N New  |  Ctrl+H History  |  Ctrl+E Export  |  Ctrl+Shift+S Settings",
            Screen::Chat | Screen::NewChat => "Ctrl+Enter Send  |  Esc Cancel/Back  |  Ctrl+N New Chat",
            Screen::History => "\u{2191}\u{2193} Navigate  |  Enter Open  |  Del Delete  |  / Search  |  s Sort",
            Screen::Analysis => "Tab Cycle Focus  |  Esc Back",
            Screen::Export => "Space Toggle  |  Ctrl+Shift+A Select All  |  Ctrl+Shift+E Export",
            Screen::Settings => "Esc Back",
            Screen::Loading => "Starting up...",
        };

        container(
            text(hints).size(11).color(theme::TEXT_MUTED),
        )
        .padding([0, 16])
        .height(theme::SHORTCUT_BAR_HEIGHT)
        .width(Length::Fill)
        .center_y(theme::SHORTCUT_BAR_HEIGHT)
        .style(theme::shortcut_bar_style())
        .into()
    }

    // ── Chat screen ──────────────────────────────────
    fn view_chat(&self) -> Element<'_, Message> {
        match &self.chat {
            None => container(
                text("Loading chat...")
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            )
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(16)
            .center(Length::Fill)
            .into(),
            Some(chat) => {
                let mut messages_col = Column::new().spacing(16).padding([16, 24]);

                for msg in &chat.messages {
                    let role_color = theme::role_indicator_color(&msg.role);
                    let role_label = msg.role.to_uppercase();

                    let indicator = container(Space::new(3, Length::Fill))
                        .style(theme::container_style(role_color, None));

                    let msg_content = column![
                        text(role_label).size(10).color(role_color),
                        text(&msg.content).size(14).color(theme::TEXT_PRIMARY),
                    ]
                    .spacing(4);

                    let msg_row = row![indicator, msg_content].spacing(12);
                    messages_col = messages_col.push(msg_row);
                }

                // Streaming content
                if !chat.streaming_content.is_empty() {
                    let indicator = container(Space::new(3, Length::Fill))
                        .style(theme::container_style(theme::ROLE_ASSISTANT, None));

                    let cursor = if chat.blink_visible { "\u{2588}" } else { "" };
                    let content = format!("{}{}", chat.streaming_content, cursor);

                    let msg_content = column![
                        text("ASSISTANT").size(10).color(theme::ROLE_ASSISTANT),
                        text(content).size(14).color(theme::TEXT_PRIMARY),
                    ]
                    .spacing(4);

                    messages_col = messages_col.push(row![indicator, msg_content].spacing(12));
                }

                // Input area
                let input = text_input("Type a message...", &chat.input)
                    .on_input(Message::ChatInputChanged)
                    .on_submit(Message::SendMessage)
                    .style(theme::input_style())
                    .size(14)
                    .width(Length::Fill);

                let mut input_row = Row::new().spacing(8).align_y(iced::Alignment::Center);

                match chat.state {
                    ChatState::Streaming => {
                        input_row = input_row
                            .push(input)
                            .push(
                                button(text("Cancel").size(13).color(theme::TEXT_PRIMARY))
                                    .style(theme::flat_button_style())
                                    .on_press(Message::CancelStream),
                            );
                    }
                    _ => {
                        input_row = input_row
                            .push(input)
                            .push(
                                button(text("Send").size(13).color(theme::TEXT_PRIMARY))
                                    .style(theme::accent_button_style())
                                    .on_press(Message::SendMessage),
                            );
                    }
                }

                let send_hint = text("Ctrl+Enter to send")
                    .size(11)
                    .color(theme::TEXT_MUTED);

                column![
                    scrollable(messages_col).height(Length::Fill),
                    container(column![input_row, send_hint].spacing(4))
                        .padding([12, 24])
                        .style(theme::container_style(theme::BG_SURFACE, None)),
                ]
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
            }
        }
    }

    // ── Metrics panel (340px, Chat only) ─────────────
    fn view_metrics_panel(&self) -> Element<'_, Message> {
        let panel_content = match &self.chat {
            None => {
                column![
                    text("Metrics").size(14).color(theme::TEXT_PRIMARY),
                    text("No active session")
                        .size(12)
                        .color(theme::TEXT_MUTED),
                ]
                .spacing(8)
            }
            Some(chat) => match chat.state {
                ChatState::Streaming => {
                    // Live streaming metrics
                    let tps = chat.token_session
                        .as_ref()
                        .map(|ts| ts.tps(std::time::Instant::now()))
                        .unwrap_or(0.0);

                    let ttft = chat.token_session
                        .as_ref()
                        .and_then(|ts| ts.ttft())
                        .unwrap_or(0.0);

                    let elapsed = chat.stream_start
                        .map(|s| s.elapsed().as_secs_f64())
                        .unwrap_or(0.0);

                    let tps_display = text(format!("{:.1}", tps))
                        .size(32)
                        .color(theme::METRIC_TPS);

                    let tps_label = text("tokens/sec")
                        .size(11)
                        .color(theme::TEXT_MUTED);

                    let sparkline = crate::sparkline::sparkline_view(&chat.tps_samples);

                    let live_stats = column![
                        metric_row("TTFT", &format!("{:.0}ms", ttft), theme::METRIC_TTFT),
                        metric_row("Tokens", &chat.chunk_count.to_string(), theme::METRIC_TOKENS),
                        metric_row("Elapsed", &format!("{:.1}s", elapsed), theme::TEXT_SECONDARY),
                        metric_row("Chunks", &chat.chunk_count.to_string(), theme::TEXT_SECONDARY),
                    ]
                    .spacing(6);

                    // Projections
                    let proj_total = if elapsed > 0.0 && tps > 0.0 {
                        format!("~{:.0} tokens", tps * elapsed * 1.5)
                    } else {
                        "-".to_string()
                    };

                    let estimates = column![
                        text("ESTIMATES").size(10).color(theme::TEXT_MUTED),
                        metric_row("Projected total", &proj_total, theme::TEXT_SECONDARY),
                    ]
                    .spacing(4);

                    column![
                        text("LIVE METRICS").size(10).color(theme::TEXT_MUTED),
                        tps_display,
                        tps_label,
                        sparkline,
                        live_stats,
                        Space::new(0, 8),
                        estimates,
                    ]
                    .spacing(8)
                }
                ChatState::Idle if chat.metrics.tps > 0.0 => {
                    // Post-response summary
                    let tps_display = text(format!("{:.1}", chat.metrics.tps))
                        .size(32)
                        .color(theme::METRIC_TPS);

                    let sparkline = crate::sparkline::sparkline_view(&chat.tps_samples);

                    let eval_tps = if chat.metrics.eval_duration_nanos > 0 {
                        (chat.metrics.completion_tokens as f64)
                            / (chat.metrics.eval_duration_nanos as f64 / 1e9)
                    } else {
                        0.0
                    };

                    let delta = chat.metrics.tps - eval_tps;
                    let delta_pct = if eval_tps > 0.0 {
                        (delta / eval_tps) * 100.0
                    } else {
                        0.0
                    };

                    let comparison = column![
                        text("COMPARISON").size(10).color(theme::TEXT_MUTED),
                        metric_row("Client measured", &format!("{:.1} t/s", chat.metrics.tps), theme::METRIC_TPS),
                        metric_row("Ollama reported", &format!("{:.1} t/s", eval_tps), theme::TEXT_SECONDARY),
                        metric_row("Delta", &format!("{:+.1} ({:+.1}%)", delta, delta_pct), if delta.abs() > 1.0 { theme::METRIC_ERROR } else { theme::TEXT_MUTED }),
                    ]
                    .spacing(4);

                    column![
                        text("LAST RESPONSE").size(10).color(theme::TEXT_MUTED),
                        tps_display,
                        text("tokens/sec").size(11).color(theme::TEXT_MUTED),
                        sparkline,
                        metric_row("TTFT", &format!("{:.0}ms", chat.metrics.ttft_ms), theme::METRIC_TTFT),
                        metric_row("Tokens", &format!("{}", chat.metrics.prompt_tokens + chat.metrics.completion_tokens), theme::METRIC_TOKENS),
                        Space::new(0, 8),
                        comparison,
                    ]
                    .spacing(8)
                }
                _ => {
                    // Idle, waiting
                    let session_total = format!(
                        "{} total tokens",
                        chat.metrics.prompt_tokens + chat.metrics.completion_tokens
                    );
                    column![
                        text("METRICS").size(10).color(theme::TEXT_MUTED),
                        text("Waiting for response...")
                            .size(13)
                            .color(theme::TEXT_SECONDARY),
                        Space::new(0, 8),
                        text("SESSION TOTALS").size(10).color(theme::TEXT_MUTED),
                        text(session_total).size(12).color(theme::TEXT_SECONDARY),
                        metric_row("TPS (last)", &format!("{:.1}", chat.metrics.tps), theme::METRIC_TPS),
                        metric_row("TTFT (last)", &format!("{:.0}ms", chat.metrics.ttft_ms), theme::METRIC_TTFT),
                    ]
                    .spacing(6)
                }
            },
        };

        container(
            scrollable(panel_content.padding(16)),
        )
        .width(theme::METRICS_PANEL_WIDTH)
        .height(Length::Fill)
        .style(theme::metrics_panel_style())
        .into()
    }

    // ── Session list screen ──────────────────────────
    fn view_session_list(&self) -> Element<'_, Message> {
        let mut list = Column::new().spacing(4).padding(16);

        let header = text("Sessions")
            .size(20)
            .color(theme::TEXT_PRIMARY);
        list = list.push(header);
        list = list.push(Space::new(0, 8));

        if self.session_list.loading {
            list = list.push(
                text("Loading sessions...")
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            );
        } else if self.session_list.sessions.is_empty() {
            list = list.push(
                text("No sessions yet. Press Ctrl+N to start a new chat!")
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            );
        } else {
            for session in &self.session_list.sessions {
                let sid = session.id.clone();
                let sid_delete = session.id.clone();

                let title_text = text(&session.title)
                    .size(14)
                    .color(theme::TEXT_PRIMARY);
                let model_text = text(&session.model)
                    .size(12)
                    .color(theme::TEXT_MUTED);
                let tps_text = text(format!("{:.1} t/s", session.metrics.tps))
                    .size(12)
                    .color(theme::METRIC_TPS);

                let session_row = row![
                    button(column![title_text, model_text].spacing(2))
                        .style(theme::flat_button_style())
                        .on_press(Message::SessionSelected(sid))
                        .width(Length::Fill),
                    tps_text,
                    button(text("\u{2715}").size(12).color(theme::TEXT_MUTED))
                        .style(theme::flat_button_style())
                        .on_press(Message::DeleteSession(sid_delete)),
                ]
                .spacing(8)
                .align_y(iced::Alignment::Center);

                list = list.push(session_row);
            }
        }

        container(scrollable(list))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::container_style(theme::BG_ROOT, None))
            .into()
    }

    // ── History screen ───────────────────────────────
    fn view_history(&self) -> Element<'_, Message> {
        let history = match &self.history {
            None => {
                return container(
                    text("No history data")
                        .size(14)
                        .color(theme::TEXT_SECONDARY),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16)
                .into();
            }
            Some(h) => h,
        };

        // Search input
        let search = text_input("Search sessions... (/)", &history.search_query)
            .on_input(Message::HistorySearchChanged)
            .style(theme::input_style())
            .size(13)
            .width(Length::Fill);

        // Sort indicator
        let sort_indicator = |col: SortColumn| -> &str {
            if history.sort_column == col {
                match history.sort_direction {
                    crate::screens::history::SortDirection::Asc => " \u{25B2}",
                    crate::screens::history::SortDirection::Desc => " \u{25BC}",
                }
            } else {
                ""
            }
        };

        // Table headers
        let headers = row![
            sort_header("Title", SortColumn::Title, sort_indicator(SortColumn::Title)),
            sort_header("Model", SortColumn::Model, sort_indicator(SortColumn::Model)),
            sort_header("TPS", SortColumn::Tps, sort_indicator(SortColumn::Tps)),
            sort_header("TTFT", SortColumn::Ttft, sort_indicator(SortColumn::Ttft)),
            sort_header("Turns", SortColumn::Turns, sort_indicator(SortColumn::Turns)),
            sort_header("Date", SortColumn::Date, sort_indicator(SortColumn::Date)),
        ]
        .spacing(4);

        // Table rows
        let filtered = history.filtered_sessions();
        let mut rows = Column::new().spacing(2);

        for (i, session) in filtered.iter().enumerate() {
            let is_selected = history.selected_index == Some(i);
            let sid = session.id.clone();
            let entry = button(
                row![
                    text(truncate_str(&session.title, 30))
                        .size(13)
                        .color(theme::TEXT_PRIMARY)
                        .width(Length::FillPortion(3)),
                    text(truncate_str(&session.model, 15))
                        .size(13)
                        .color(theme::TEXT_SECONDARY)
                        .width(Length::FillPortion(2)),
                    text(format!("{:.1}", session.metrics.tps))
                        .size(13)
                        .color(theme::METRIC_TPS)
                        .width(Length::FillPortion(1)),
                    text(format!("{:.0}ms", session.metrics.ttft_ms))
                        .size(13)
                        .color(theme::METRIC_TTFT)
                        .width(Length::FillPortion(1)),
                    text(format!("{}", session.messages.len()))
                        .size(13)
                        .color(theme::TEXT_SECONDARY)
                        .width(Length::FillPortion(1)),
                    text(truncate_str(&session.updated_at, 16))
                        .size(13)
                        .color(theme::TEXT_MUTED)
                        .width(Length::FillPortion(2)),
                ]
                .spacing(4),
            )
            .style(theme::session_entry_style(is_selected))
            .on_press(Message::SessionSelected(sid))
            .width(Length::Fill)
            .padding([4, 8]);

            rows = rows.push(entry);
        }

        // Footer
        let total_sessions = filtered.len();
        let avg_tps = if total_sessions > 0 {
            filtered.iter().map(|s| s.metrics.tps).sum::<f64>() / total_sessions as f64
        } else {
            0.0
        };
        let footer = text(format!(
            "{} sessions  |  Avg TPS: {:.1}",
            total_sessions, avg_tps
        ))
        .size(12)
        .color(theme::TEXT_MUTED);

        let content = column![
            text("History").size(20).color(theme::TEXT_PRIMARY),
            search,
            Space::new(0, 4),
            headers,
            scrollable(rows).height(Length::Fill),
            container(footer).padding([8, 0]),
        ]
        .spacing(8)
        .padding(16);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::container_style(theme::BG_ROOT, None))
            .into()
    }

    // ── Analysis screen ──────────────────────────────
    fn view_analysis(&self) -> Element<'_, Message> {
        let analysis = match &self.analysis_screen {
            None => {
                return container(
                    text("Analysis not initialized")
                        .size(14)
                        .color(theme::TEXT_SECONDARY),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(16)
                .into();
            }
            Some(a) => a,
        };

        let sessions = &self.session_list.sessions;

        // Left picker
        let left_picker = session_picker(
            "Left Session",
            &analysis.left_session_id,
            sessions,
            Message::AnalysisSelectLeft,
            analysis.focus == crate::screens::analysis::AnalysisFocus::LeftPicker,
        );

        // Right picker
        let right_picker = session_picker(
            "Right Session",
            &analysis.right_session_id,
            sessions,
            Message::AnalysisSelectRight,
            analysis.focus == crate::screens::analysis::AnalysisFocus::RightPicker,
        );

        let pickers = row![left_picker, right_picker].spacing(16);

        // Results
        let results: Element<'_, Message> = if let Some(score) = analysis.similarity_score {
            let score_color = if score > 0.7 {
                theme::STATUS_CONNECTED
            } else if score > 0.3 {
                theme::ROLE_SYSTEM
            } else {
                theme::METRIC_ERROR
            };

            let score_display = text(format!("{:.1}%", score * 100.0))
                .size(28)
                .color(score_color);

            let progress_width = (score * 300.0) as u16;
            let progress_bar = container(Space::new(progress_width, 4))
                .style(theme::container_style(score_color, None))
                .width(progress_width);

            // Shared terms
            let mut shared_col = Column::new().spacing(2);
            for term in &analysis.shared_terms {
                shared_col = shared_col.push(
                    text(format!("\u{25C9} {}", term))
                        .size(12)
                        .color(theme::TEXT_ACCENT),
                );
            }

            // Left-only terms
            let mut left_col = Column::new().spacing(2);
            for term in &analysis.left_only_terms {
                left_col = left_col.push(
                    text(format!("\u{25CF} {}", term))
                        .size(12)
                        .color(theme::TEXT_MUTED),
                );
            }

            // Right-only terms
            let mut right_col = Column::new().spacing(2);
            for term in &analysis.right_only_terms {
                right_col = right_col.push(
                    text(format!("\u{25CF} {}", term))
                        .size(12)
                        .color(theme::TEXT_MUTED),
                );
            }

            column![
                text("SIMILARITY").size(10).color(theme::TEXT_MUTED),
                score_display,
                progress_bar,
                Space::new(0, 12),
                row![
                    column![
                        text("Shared Terms").size(11).color(theme::TEXT_ACCENT),
                        scrollable(shared_col).height(120),
                    ]
                    .spacing(4)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Left Only").size(11).color(theme::TEXT_MUTED),
                        scrollable(left_col).height(120),
                    ]
                    .spacing(4)
                    .width(Length::FillPortion(1)),
                    column![
                        text("Right Only").size(11).color(theme::TEXT_MUTED),
                        scrollable(right_col).height(120),
                    ]
                    .spacing(4)
                    .width(Length::FillPortion(1)),
                ]
                .spacing(16),
            ]
            .spacing(8)
            .into()
        } else {
            text("Select two sessions to compare")
                .size(14)
                .color(theme::TEXT_SECONDARY)
                .into()
        };

        let content = column![
            text("Analysis").size(20).color(theme::TEXT_PRIMARY),
            pickers,
            Space::new(0, 8),
            results,
        ]
        .spacing(12)
        .padding(16);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::container_style(theme::BG_ROOT, None))
            .into()
    }

    // ── Export screen ────────────────────────────────
    fn view_export(&self) -> Element<'_, Message> {
        match &self.export {
            None => container(
                text("No export data")
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            )
            .width(Length::Fill)
            .padding(16)
            .into(),
            Some(export_screen) => {
                // Session checklist
                let mut checklist = Column::new().spacing(4);
                let all_selected = export_screen.selected_ids.len() == export_screen.sessions.len()
                    && !export_screen.sessions.is_empty();

                let toggle_all_label = if all_selected { "Deselect All" } else { "Select All" };
                let toggle_all_msg = if all_selected {
                    Message::ExportDeselectAll
                } else {
                    Message::ExportSelectAll
                };

                checklist = checklist.push(
                    button(
                        text(toggle_all_label)
                            .size(13)
                            .color(theme::TEXT_ACCENT),
                    )
                    .style(theme::flat_button_style())
                    .on_press(toggle_all_msg),
                );

                for session in &export_screen.sessions {
                    let is_selected = export_screen.selected_ids.contains(&session.id);
                    let checkbox = if is_selected { "\u{2611}" } else { "\u{2610}" };
                    let sid = session.id.clone();

                    checklist = checklist.push(
                        button(
                            row![
                                text(checkbox).size(14).color(theme::TEXT_ACCENT),
                                text(truncate_str(&session.title, 30))
                                    .size(13)
                                    .color(theme::TEXT_PRIMARY),
                                text(&session.model)
                                    .size(11)
                                    .color(theme::TEXT_MUTED),
                            ]
                            .spacing(8)
                            .align_y(iced::Alignment::Center),
                        )
                        .style(theme::flat_button_style())
                        .on_press(Message::ExportToggleSession(sid))
                        .width(Length::Fill),
                    );
                }

                // Preview pane
                let preview: Element<'_, Message> = if let Some(ref csv) = export_screen.preview {
                    container(
                        scrollable(
                            text(truncate_str(csv, 2000))
                                .size(12)
                                .color(theme::TEXT_SECONDARY),
                        )
                        .height(200),
                    )
                    .style(theme::container_style(theme::BG_ROOT, None))
                    .padding(8)
                    .width(Length::Fill)
                    .into()
                } else {
                    text("No preview available")
                        .size(12)
                        .color(theme::TEXT_MUTED)
                        .into()
                };

                // Status
                let status_text = match &export_screen.status {
                    ExportStatus::Ready => "Ready to export".to_string(),
                    ExportStatus::Exporting => "Exporting...".to_string(),
                    ExportStatus::Done(path) => format!("Exported to: {path}"),
                    ExportStatus::Error(e) => format!("Error: {e}"),
                };

                let status_color = match &export_screen.status {
                    ExportStatus::Done(_) => theme::STATUS_CONNECTED,
                    ExportStatus::Error(_) => theme::METRIC_ERROR,
                    _ => theme::TEXT_SECONDARY,
                };

                let selected_count = export_screen.selected_ids.len();
                let summary = format!(
                    "{} of {} sessions selected",
                    selected_count,
                    export_screen.sessions.len()
                );

                let left_panel = column![
                    text("Sessions").size(14).color(theme::TEXT_PRIMARY),
                    scrollable(checklist).height(Length::Fill),
                    text(summary).size(12).color(theme::TEXT_MUTED),
                ]
                .spacing(8)
                .width(Length::FillPortion(1));

                let right_panel = column![
                    text("CSV Preview").size(14).color(theme::TEXT_PRIMARY),
                    preview,
                    Space::new(0, 8),
                    text(status_text).size(13).color(status_color),
                    button(
                        text("Export CSV").size(13).color(theme::TEXT_PRIMARY),
                    )
                    .style(theme::accent_button_style())
                    .on_press(Message::ExportRequested),
                ]
                .spacing(8)
                .width(Length::FillPortion(1));

                let content = column![
                    text("Export Sessions to CSV")
                        .size(20)
                        .color(theme::TEXT_PRIMARY),
                    row![left_panel, right_panel].spacing(24),
                ]
                .spacing(12)
                .padding(16);

                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(theme::container_style(theme::BG_ROOT, None))
                    .into()
            }
        }
    }

    // ── Settings screen ──────────────────────────────
    fn view_settings(&self) -> Element<'_, Message> {
        let url_input = text_input("Ollama base URL", &self.settings.base_url)
            .on_input(Message::BaseUrlChanged)
            .style(theme::input_style())
            .size(14)
            .width(400);

        let mut models_col = Column::new().spacing(4);
        if self.settings.loading_models {
            models_col = models_col.push(
                text("Loading models...")
                    .size(14)
                    .color(theme::TEXT_SECONDARY),
            );
        } else {
            for model in &self.settings.available_models {
                let m = model.clone();
                let is_selected = *model == self.settings.selected_model;
                let label_color = if is_selected {
                    theme::TEXT_ACCENT
                } else {
                    theme::TEXT_PRIMARY
                };
                let prefix = if is_selected { "\u{25C9} " } else { "\u{25CB} " };

                models_col = models_col.push(
                    button(
                        text(format!("{prefix}{model}"))
                            .size(14)
                            .color(label_color),
                    )
                    .style(theme::flat_button_style())
                    .on_press(Message::ModelSelected(m))
                    .width(Length::Fill),
                );
            }
        }

        let content = column![
            text("Settings").size(20).color(theme::TEXT_PRIMARY),
            Space::new(0, 8),
            text("Ollama API URL").size(12).color(theme::TEXT_MUTED),
            url_input,
            Space::new(0, 12),
            text(format!("Current model: {}", self.settings.selected_model))
                .size(13)
                .color(theme::TEXT_SECONDARY),
            Space::new(0, 8),
            text("Available Models").size(12).color(theme::TEXT_MUTED),
            models_col,
        ]
        .spacing(4)
        .padding(16)
        .width(Length::Fill);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(theme::container_style(theme::BG_ROOT, None))
            .into()
    }

    // ── Loading screen (full-width, centered) ────────
    fn view_loading(&self) -> Element<'_, Message> {
        let title = text("ollama-scope")
            .size(24)
            .color(theme::TEXT_PRIMARY);

        let subtitle = text("Initializing...")
            .size(14)
            .color(theme::TEXT_SECONDARY);

        let mut steps = Column::new().spacing(6);
        for step in &self.loading.steps {
            let (icon, color) = match &step.status {
                StepStatus::Pending => ("\u{25CB}", theme::TEXT_MUTED),      // ○
                StepStatus::InProgress => ("\u{25CF}", theme::STATUS_STREAMING), // ●
                StepStatus::Done => ("\u{2713}", theme::STATUS_CONNECTED),   // ✓
                StepStatus::Failed(_) => ("\u{2717}", theme::METRIC_ERROR),  // ✗
            };

            let mut step_row = Row::new().spacing(8).align_y(iced::Alignment::Center);
            step_row = step_row.push(text(icon).size(14).color(color));
            step_row = step_row.push(text(&step.label).size(13).color(theme::TEXT_PRIMARY));

            if let StepStatus::Failed(ref e) = step.status {
                step_row = step_row.push(
                    text(e).size(11).color(theme::METRIC_ERROR),
                );
            }

            steps = steps.push(step_row);
        }

        // Model list if loaded
        let models: Element<'_, Message> = if !self.loading.models.is_empty() {
            let mut model_list = Column::new().spacing(2);
            model_list = model_list.push(
                text("Available Models")
                    .size(12)
                    .color(theme::TEXT_MUTED),
            );
            for m in &self.loading.models {
                model_list = model_list.push(
                    text(format!("  {m}"))
                        .size(12)
                        .color(theme::TEXT_SECONDARY),
                );
            }
            model_list.into()
        } else {
            Space::new(0, 0).into()
        };

        let content = column![title, subtitle, Space::new(0, 16), steps, Space::new(0, 8), models]
            .spacing(4)
            .align_x(iced::Alignment::Center)
            .max_width(400);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center(Length::Fill)
            .style(theme::container_style(theme::BG_ROOT, None))
            .into()
    }
}

// ── Helper functions ─────────────────────────────

fn nav_button(label: &str, active: bool, msg: Message) -> Element<'_, Message> {
    button(text(label.to_string()).size(13).color(if active {
        theme::TEXT_ACCENT
    } else {
        theme::TEXT_SECONDARY
    }))
    .style(theme::nav_button_style(active))
    .on_press(msg)
    .width(Length::Fill)
    .into()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn metric_row<'a>(label: &str, value: &str, color: iced::Color) -> Element<'a, Message> {
    row![
        text(label.to_string())
            .size(12)
            .color(theme::TEXT_MUTED)
            .width(Length::FillPortion(1)),
        text(value.to_string())
            .size(13)
            .color(color)
            .width(Length::FillPortion(1)),
    ]
    .spacing(8)
    .into()
}

fn sort_header<'a>(
    label: &str,
    column: SortColumn,
    indicator: &str,
) -> Element<'a, Message> {
    button(
        text(format!("{label}{indicator}"))
            .size(11)
            .color(theme::TEXT_MUTED),
    )
    .style(theme::flat_button_style())
    .on_press(Message::HistorySortBy(column))
    .width(Length::FillPortion(match column {
        SortColumn::Title => 3,
        SortColumn::Model => 2,
        SortColumn::Date => 2,
        _ => 1,
    }))
    .into()
}

fn session_picker<'a, F>(
    title: &str,
    selected_id: &Option<String>,
    sessions: &[crate::types::Session],
    on_select: F,
    focused: bool,
) -> Element<'a, Message>
where
    F: Fn(String) -> Message + 'a,
{
    let border = if focused {
        Some(iced::Border {
            color: theme::BORDER_FOCUS,
            width: 2.0,
            radius: 4.0.into(),
        })
    } else {
        Some(iced::Border {
            color: theme::BORDER_DEFAULT,
            width: 1.0,
            radius: 4.0.into(),
        })
    };

    let mut list = Column::new().spacing(2);
    list = list.push(
        text(title.to_string())
            .size(12)
            .color(theme::TEXT_MUTED),
    );

    for session in sessions {
        let is_selected = selected_id.as_deref() == Some(&session.id);
        let sid = session.id.clone();

        list = list.push(
            button(
                text(truncate_str(&session.title, 25))
                    .size(13)
                    .color(if is_selected {
                        theme::TEXT_ACCENT
                    } else {
                        theme::TEXT_PRIMARY
                    }),
            )
            .style(theme::session_entry_style(is_selected))
            .on_press(on_select(sid))
            .width(Length::Fill),
        );
    }

    container(scrollable(list.padding(8)).height(200))
        .style(theme::container_style(theme::BG_SURFACE, border))
        .width(Length::FillPortion(1))
        .into()
}
