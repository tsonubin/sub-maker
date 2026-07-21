use ratatui::style::{Color, Modifier, Style};

/// Sub-maker TUI palette — dark terminal, cyan primary, amber focus.
pub struct Theme;

impl Theme {
    pub const FG: Color = Color::Rgb(220, 224, 232);
    pub const MUTED: Color = Color::Rgb(120, 130, 150);
    pub const BORDER: Color = Color::Rgb(50, 70, 95);
    pub const PRIMARY: Color = Color::Rgb(80, 200, 220);
    pub const ACCENT: Color = Color::Rgb(255, 190, 80);
    pub const SUCCESS: Color = Color::Rgb(100, 210, 140);
    pub const DANGER: Color = Color::Rgb(240, 100, 110);
    pub const FOCUS_BG: Color = Color::Rgb(30, 55, 75);
    pub const SELECT_BG: Color = Color::Rgb(40, 90, 110);

    pub fn title() -> Style {
        Style::default()
            .fg(Self::PRIMARY)
            .add_modifier(Modifier::BOLD)
    }

    pub fn border() -> Style {
        Style::default().fg(Self::BORDER)
    }

    pub fn border_focus() -> Style {
        Style::default().fg(Self::ACCENT)
    }

    pub fn muted() -> Style {
        Style::default().fg(Self::MUTED)
    }

    pub fn body() -> Style {
        Style::default().fg(Self::FG)
    }

    pub fn accent() -> Style {
        Style::default().fg(Self::ACCENT)
    }

    pub fn success() -> Style {
        Style::default().fg(Self::SUCCESS)
    }

    pub fn danger() -> Style {
        Style::default().fg(Self::DANGER)
    }

    pub fn selected() -> Style {
        Style::default()
            .fg(Self::FG)
            .bg(Self::SELECT_BG)
            .add_modifier(Modifier::BOLD)
    }

    pub fn focused_field() -> Style {
        Style::default().fg(Self::FG).bg(Self::FOCUS_BG)
    }

    pub fn key() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }

    pub fn step_done() -> Style {
        Style::default().fg(Self::SUCCESS)
    }

    pub fn step_current() -> Style {
        Style::default()
            .fg(Self::PRIMARY)
            .add_modifier(Modifier::BOLD)
    }

    pub fn step_pending() -> Style {
        Style::default().fg(Self::MUTED)
    }
}
