use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(10), // Maintenance Actions
            Constraint::Min(0),     // Console/Output
            Constraint::Length(1),  // Hints
        ])
        .split(area);

    render_actions(f, app, chunks[0]);
    render_output(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);
}

fn render_actions(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Janitor - System Maintenance ")
        .borders(Borders::ALL);

    let mut lines = Vec::new();
    
    // Action 1: Package Cache
    let pkg_manager = app.platform.packages.name();
    lines.push(Line::from(vec![
        Span::styled(" [c] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled(format!("Clean {} Package Cache", pkg_manager), Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" - Remove cached installer files to free up space."),
    ]));
    
    lines.push(Line::from(""));

    // Info
    lines.push(Line::from(vec![
        Span::styled(" Tip: ", Style::default().fg(Color::Cyan)),
        Span::raw("Periodic cleaning helps keep the server lean, especially after heavy updates."),
    ]));

    if let Some(ref op) = app.maintenance.running_op {
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!(" ⚙ Running: {}... ", op), Style::default().bg(Color::Blue).fg(Color::White)),
        ]));
    }

    let p = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);
    
    f.render_widget(p, area);
}

fn render_output(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Output ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let content = if app.maintenance.last_output.is_empty() {
        "No recent maintenance actions."
    } else {
        &app.maintenance.last_output
    };

    let p = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(Color::Gray));
    
    f.render_widget(p, area);
}

fn render_hints(f: &mut Frame, _app: &App, area: Rect) {
    let hints = Paragraph::new(Span::styled(
        " Choose an action above to start cleanup. ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, area);
}
