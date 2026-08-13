use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::{
    core::homelab::HomelabFeatureStatus,
    tui::app::App,
};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(1),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Enabled", Style::default().fg(Color::Green)),
            Span::raw(" means the named homelab optimization is active. Settings are persistent when supported."),
        ]))
        .block(Block::default().title(" Homelab server stability ").borders(Borders::ALL)),
        chunks[0],
    );

    let header = Row::new(["Setting", "State", "Details"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let rows = app.homelab.statuses.iter().map(|item| {
        let (label, color) = match item.status {
            HomelabFeatureStatus::Enabled => ("Enabled", Color::Green),
            HomelabFeatureStatus::Disabled => ("Disabled", Color::Yellow),
            HomelabFeatureStatus::Unavailable => ("Unavailable", Color::DarkGray),
            HomelabFeatureStatus::Error => ("Error", Color::Red),
        };
        Row::new([
            Cell::from(item.feature.label()),
            Cell::from(Span::styled(label, Style::default().fg(color))),
            Cell::from(item.detail.as_str()),
        ])
    });
    let title = if app.homelab.loading {
        " Settings — loading… "
    } else if app.homelab.mutating.is_some() {
        " Settings — applying… "
    } else {
        " Settings "
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(38),
            Constraint::Length(13),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol("▶ ")
    .block(Block::default().title(title).borders(Borders::ALL));
    f.render_stateful_widget(table, chunks[1], &mut app.homelab.table_state);

    f.render_widget(
        Paragraph::new(Span::styled(
            "[↑/↓] select  [Space/Enter] toggle  [r] refresh",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[2],
    );
}
