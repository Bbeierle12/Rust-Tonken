use iced::widget::{button, container, text_input};
use iced::{Border, Color, Theme};

// ── Hex-to-Color helper ──────────────────────────
const fn hex(r: u8, g: u8, b: u8) -> Color {
    Color {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

// ── Backgrounds ──────────────────────────────────
pub const BG_ROOT: Color = hex(0x0D, 0x11, 0x17);
pub const BG_SURFACE: Color = hex(0x16, 0x1B, 0x22);
pub const BG_ELEVATED: Color = hex(0x1C, 0x21, 0x28);
pub const BG_HOVER: Color = hex(0x25, 0x2C, 0x35);
pub const BG_ACTIVE: Color = hex(0x2D, 0x33, 0x3B);

// ── Borders ──────────────────────────────────────
pub const BORDER_DEFAULT: Color = hex(0x30, 0x36, 0x3D);
pub const BORDER_FOCUS: Color = hex(0x58, 0xA6, 0xFF);

// ── Text ─────────────────────────────────────────
pub const TEXT_PRIMARY: Color = hex(0xE6, 0xED, 0xF3);
pub const TEXT_SECONDARY: Color = hex(0x8B, 0x94, 0x9E);
pub const TEXT_MUTED: Color = hex(0x48, 0x4F, 0x58);
pub const TEXT_ACCENT: Color = hex(0x58, 0xA6, 0xFF);

// ── Roles ────────────────────────────────────────
pub const ROLE_USER: Color = hex(0x3F, 0xB9, 0x50);
pub const ROLE_ASSISTANT: Color = hex(0x58, 0xA6, 0xFF);
pub const ROLE_SYSTEM: Color = hex(0xD2, 0x99, 0x22);

// ── Metrics ──────────────────────────────────────
pub const METRIC_TPS: Color = hex(0x58, 0xA6, 0xFF);
pub const METRIC_TTFT: Color = hex(0x3F, 0xB9, 0x50);
pub const METRIC_TOKENS: Color = hex(0xD2, 0xA8, 0xFF);
pub const METRIC_ERROR: Color = hex(0xF8, 0x51, 0x49);

// ── Status ───────────────────────────────────────
pub const STATUS_CONNECTED: Color = hex(0x3F, 0xB9, 0x50);
pub const STATUS_DISCONNECTED: Color = hex(0xF8, 0x51, 0x49);
pub const STATUS_STREAMING: Color = hex(0x58, 0xA6, 0xFF);

// ── Layout dimensions ────────────────────────────
pub const SIDEBAR_WIDTH: u16 = 220;
pub const STATUS_BAR_HEIGHT: u16 = 28;
pub const SHORTCUT_BAR_HEIGHT: u16 = 24;
pub const METRICS_PANEL_WIDTH: u16 = 340;
pub const SPARKLINE_WIDTH: f32 = 316.0;
pub const SPARKLINE_HEIGHT: f32 = 80.0;

// ── Style functions ──────────────────────────────

/// General container style with background color and optional border.
pub fn container_style(
    bg: Color,
    border: Option<Border>,
) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(bg)),
        border: border.unwrap_or_default(),
        ..container::Style::default()
    }
}

/// Navigation button style: active tab vs inactive.
pub fn nav_button_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let base_bg = if active { BG_ACTIVE } else { Color::TRANSPARENT };
        let text_color = if active { TEXT_ACCENT } else { TEXT_SECONDARY };

        let bg = match status {
            button::Status::Hovered => BG_HOVER,
            button::Status::Pressed => BG_ACTIVE,
            _ => base_bg,
        };

        let border = if active {
            Border {
                color: BORDER_FOCUS,
                width: 0.0,
                radius: 4.0.into(),
            }
        } else {
            Border {
                radius: 4.0.into(),
                ..Border::default()
            }
        };

        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color,
            border,
            ..button::Style::default()
        }
    }
}

/// Return the indicator color for a message role.
pub fn role_indicator_color(role: &str) -> Color {
    match role {
        "user" => ROLE_USER,
        "assistant" => ROLE_ASSISTANT,
        "system" => ROLE_SYSTEM,
        _ => TEXT_MUTED,
    }
}

/// Status bar style: BG_SURFACE with bottom border.
pub fn status_bar_style() -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(BG_SURFACE)),
        border: Border {
            color: BORDER_DEFAULT,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Sidebar style: BG_SURFACE with right border.
pub fn sidebar_style() -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(BG_SURFACE)),
        border: Border {
            color: BORDER_DEFAULT,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Metrics panel style: BG_SURFACE with left border.
pub fn metrics_panel_style() -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(BG_SURFACE)),
        border: Border {
            color: BORDER_DEFAULT,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Shortcut bar style: BG_ROOT with top border.
pub fn shortcut_bar_style() -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(BG_ROOT)),
        border: Border {
            color: BORDER_DEFAULT,
            width: 1.0,
            radius: 0.0.into(),
        },
        ..container::Style::default()
    }
}

/// Chat input text_input style.
pub fn input_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_theme: &Theme, status: text_input::Status| {
        let border_color = match status {
            text_input::Status::Focused => BORDER_FOCUS,
            _ => BORDER_DEFAULT,
        };
        text_input::Style {
            background: iced::Background::Color(BG_ELEVATED),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 4.0.into(),
            },
            icon: TEXT_MUTED,
            placeholder: TEXT_SECONDARY,
            value: TEXT_PRIMARY,
            selection: BORDER_FOCUS,
        }
    }
}

/// Disabled chat input style (dimmed background, muted text).
pub fn input_disabled_style() -> impl Fn(&Theme, text_input::Status) -> text_input::Style {
    move |_theme: &Theme, _status: text_input::Status| text_input::Style {
        background: iced::Background::Color(BG_ROOT),
        border: Border {
            color: BORDER_DEFAULT,
            width: 1.0,
            radius: 4.0.into(),
        },
        icon: TEXT_MUTED,
        placeholder: TEXT_MUTED,
        value: TEXT_MUTED,
        selection: BORDER_DEFAULT,
    }
}

/// Flat transparent button (for delete, dismiss, etc.).
pub fn flat_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => BG_HOVER,
            button::Status::Pressed => BG_ACTIVE,
            _ => Color::TRANSPARENT,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: TEXT_SECONDARY,
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// Accent-colored button (primary action).
pub fn accent_button_style() -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = match status {
            button::Status::Hovered => Color {
                a: 0.9,
                ..BORDER_FOCUS
            },
            button::Status::Pressed => Color {
                a: 0.7,
                ..BORDER_FOCUS
            },
            _ => BORDER_FOCUS,
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: Color::WHITE,
            border: Border {
                radius: 4.0.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// Chat message bubble with a colored border accent.
pub fn message_bubble_style(role_color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(BG_SURFACE)),
        border: Border {
            color: role_color,
            width: 2.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

/// Error banner container style.
pub fn error_banner_style() -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(iced::Background::Color(Color {
            a: 0.15,
            ..METRIC_ERROR
        })),
        border: Border {
            color: METRIC_ERROR,
            width: 1.0,
            radius: 4.0.into(),
        },
        ..container::Style::default()
    }
}

/// Session list entry style (active vs inactive).
pub fn session_entry_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_theme: &Theme, status: button::Status| {
        let bg = if active {
            BG_ACTIVE
        } else {
            match status {
                button::Status::Hovered => BG_HOVER,
                _ => Color::TRANSPARENT,
            }
        };
        let border = if active {
            Border {
                color: BORDER_FOCUS,
                width: 2.0,
                radius: 4.0.into(),
            }
        } else {
            Border {
                radius: 4.0.into(),
                ..Border::default()
            }
        };
        button::Style {
            background: Some(iced::Background::Color(bg)),
            text_color: TEXT_PRIMARY,
            border,
            ..button::Style::default()
        }
    }
}
