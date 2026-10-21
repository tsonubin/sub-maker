//! Shared drawing helpers for the multi-step wizard chrome.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

use super::theme::Theme;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Done,
    Current,
    Pending,
}

pub struct StepMeta {
    pub label: &'static str,
    pub status: StepStatus,
}

/// Root layout: header · (sidebar | main) · footer.
pub fn root_chunks(area: Rect) -> (Rect, Rect, Rect, Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(area);
    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(26), Constraint::Min(40)])
        .split(rows[1]);
    (rows[0], body[0], body[1], rows[2])
}

pub fn draw_header(f: &mut Frame, area: Rect, title: &str, subtitle: &str) {
    let line = Line::from(vec![
        Span::styled(" sub-maker ", Theme::title()),
        Span::styled("│ ", Theme::muted()),
        Span::styled(title, Theme::body().add_modifier(Modifier::BOLD)),
        Span::styled("  ", Theme::muted()),
        Span::styled(subtitle, Theme::muted()),
    ]);
    f.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Theme::border()),
        ),
        area,
    );
}

pub fn draw_sidebar(f: &mut Frame, area: Rect, steps: &[StepMeta]) {
    let items: Vec<ListItem> = steps
        .iter()
        .map(|s| {
            let (mark, style) = match s.status {
                StepStatus::Done => ("✓", Theme::step_done()),
                StepStatus::Current => ("▸", Theme::step_current()),
                StepStatus::Pending => ("·", Theme::step_pending()),
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!(" {mark} "), style),
                Span::styled(s.label, style),
            ]))
        })
        .collect();

    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Theme::border())
                .title(Span::styled(" steps ", Theme::muted())),
        ),
        area,
    );
}

pub fn draw_footer(f: &mut Frame, area: Rect, hints: &[(&str, &str)]) {
    let mut spans = Vec::new();
    for (i, (key, label)) in hints.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("  ·  ", Theme::muted()));
        }
        spans.push(Span::styled(format!(" {key} "), Theme::key()));
        spans.push(Span::styled(*label, Theme::muted()));
    }
    f.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Theme::border()),
        ),
        area,
    );
}

pub fn panel(title: &str, focused: bool) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(if focused {
            Theme::border_focus()
        } else {
            Theme::border()
        })
        .title(Span::styled(
            format!(" {title} "),
            if focused {
                Theme::accent().add_modifier(Modifier::BOLD)
            } else {
                Theme::muted()
            },
        ))
}

/// Draw a list of options; `cursor` is highlighted; optional multi-checkmarks.
pub fn draw_option_list(
    f: &mut Frame,
    area: Rect,
    title: &str,
    options: &[String],
    cursor: usize,
    checked: Option<&[bool]>,
) {
    let items: Vec<ListItem> = options
        .iter()
        .enumerate()
        .map(|(i, opt)| {
            let prefix = if let Some(c) = checked {
                let mark = if c.get(i).copied().unwrap_or(false) {
                    "[x]"
                } else {
                    "[ ]"
                };
                format!("{mark} ")
            } else if i == cursor {
                "▸ ".into()
            } else {
                "  ".into()
            };
            let style = if i == cursor {
                Theme::selected()
            } else if checked.map(|c| c.get(i).copied().unwrap_or(false)).unwrap_or(false) {
                Theme::success()
            } else {
                Theme::body()
            };
            ListItem::new(Line::from(Span::styled(format!("{prefix}{opt}"), style)))
        })
        .collect();

    f.render_widget(
        List::new(items).block(panel(title, true)),
        area,
    );
}

/// Vertical stack of labeled fields. `active` field gets focus styling.
pub fn draw_fields(
    f: &mut Frame,
    area: Rect,
    title: &str,
    fields: &[(String, String, bool, usize)], // label, value, password, cursor
    active: usize,
    help: &str,
) {
    let block = panel(title, true);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let n = fields.len().max(1) as u16;
    let help_h = 2u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(n * 3),
            Constraint::Length(help_h),
        ])
        .split(inner);

    // Scroll so the focused field stays visible when the page overflows
    // (six 3-row fields do not fit an 80x24 terminal).
    let slots = (chunks[0].height / 3).max(1) as usize;
    let total = fields.len();
    let start = if active + 1 > slots {
        (active + 1 - slots).min(total.saturating_sub(slots))
    } else {
        0
    };
    let end = (start + slots).min(total);
    let visible = &fields[start..end];

    let field_constraints: Vec<Constraint> = (0..visible.len())
        .map(|_| Constraint::Length(3))
        .collect();
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(field_constraints)
        .split(chunks[0]);

    for (i, (label, value, password, pos)) in visible.iter().enumerate() {
        let focused = start + i == active;
        let shown = if *password && !value.is_empty() {
            "•".repeat(value.chars().count())
        } else if value.is_empty() {
            "·".to_string()
        } else {
            value.clone()
        };
        // Render the caret at the field's cursor position (char-indexed).
        let caret_at = value[..(*pos).min(value.len())].chars().count();
        let mut chars = shown.chars();
        let before: String = chars.by_ref().take(caret_at).collect();
        let after: String = chars.collect();
        let border = if focused {
            Theme::border_focus()
        } else {
            Theme::border()
        };
        let label_style = if focused {
            Theme::accent().add_modifier(Modifier::BOLD)
        } else {
            Theme::muted()
        };
        let value_style = if focused {
            Theme::focused_field()
        } else {
            Theme::body()
        };
        let caret = if focused { "▏" } else { "" };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(before, value_style),
                Span::styled(caret, Theme::accent()),
                Span::styled(after, value_style),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border)
                    .title(Span::styled(format!(" {label} "), label_style)),
            ),
            rows[i],
        );
    }

    f.render_widget(
        Paragraph::new(help).style(Theme::muted()).wrap(Wrap { trim: true }),
        chunks[1],
    );
}

pub fn draw_review(f: &mut Frame, area: Rect, lines: Vec<Line<'static>>) {
    f.render_widget(
        Paragraph::new(lines)
            .block(panel("review", true))
            .wrap(Wrap { trim: false }),
        area,
    );
}

pub fn kv_line(key: &str, value: String) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<16}"), Style::default().fg(Theme::MUTED)),
        Span::styled(value, Style::default().fg(Theme::FG)),
    ])
}
