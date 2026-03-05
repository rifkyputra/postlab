use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Filter/Search bar
            Constraint::Min(0),    // Table
            Constraint::Length(1), // Hints
        ])
        .split(area);

    render_filter(f, app, chunks[0]);
    render_table(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);
}

fn render_filter(f: &mut Frame, app: &App, area: Rect) {
    let style = if app.services.filter_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Filter Services (/) ")
        .border_style(style);

    let filter_text = if app.services.filter.is_empty() && app.services.filter_mode == InputMode::Normal {
        Span::styled("Type to filter...", Style::default().fg(Color::DarkGray))
    } else {
        Span::raw(&app.services.filter)
    };

    let p = Paragraph::new(filter_text).block(block);
    f.render_widget(p, area);

    if app.services.filter_mode == InputMode::Editing {
        f.set_cursor(
            area.x + app.services.filter.len() as u16 + 1,
            area.y + 1,
        );
    }
}

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().bg(Color::DarkGray);

    let headers = Row::new([
        Cell::from("Unit"),
        Cell::from("Status"),
        Cell::from("Sub"),
        Cell::from("Description"),
    ])
    .style(header_style);

    let filter = app.services.filter.to_lowercase();
    let rows: Vec<Row> = app.services.list
        .iter()
        .filter(|s| {
            filter.is_empty() 
                || s.name.to_lowercase().contains(&filter) 
                || s.description.to_lowercase().contains(&filter)
        })
        .map(|s| {
            let status_color = match s.active_state.as_str() {
                "active" => Color::Green,
                "failed" => Color::Red,
                "inactive" => Color::Yellow,
                _ => Color::White,
            };

            Row::new([
                Cell::from(s.name.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(s.active_state.as_str()).style(Style::default().fg(status_color)),
                Cell::from(s.sub_state.as_str()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(s.description.as_str()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Fill(1),
        Constraint::Length(12),
        Constraint::Length(12),
        Constraint::Fill(2),
    ];

    let count = rows.len();
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default()
            .title(format!(" Services ({}) ", count))
            .borders(Borders::ALL))
        .row_highlight_style(selected_style)
        .highlight_symbol("› ");

    let mut state = app.services.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_hints(f: &mut Frame, _app: &App, area: Rect) {
    let hints = Paragraph::new(Span::styled(
        " [/] filter [s] start [k] stop [r] restart [e] enable [d] disable [R] refresh ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, area);
}
