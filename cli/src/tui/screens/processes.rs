use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, ProcessSort};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    render_table(f, app, chunks[0]);
    render_hints(f, app, chunks[1]);
}

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().bg(Color::DarkGray);

    let sort_indicator = |col: ProcessSort| -> &'static str {
        if app.processes.sort == col { " ↓" } else { "" }
    };

    let headers = Row::new([
        Cell::from(format!("PID{}", sort_indicator(ProcessSort::Pid))),
        Cell::from("Name"),
        Cell::from(format!("CPU%{}", sort_indicator(ProcessSort::Cpu))),
        Cell::from(format!("Memory{}", sort_indicator(ProcessSort::Memory))),
        Cell::from("User"),
        Cell::from("Status"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.processes.list
        .iter()
        .map(|p| {
            let cpu_color = if p.cpu_pct > 50.0 { Color::Red }
                else if p.cpu_pct > 20.0 { Color::Yellow }
                else { Color::White };
            Row::new([
                Cell::from(p.pid.to_string()),
                Cell::from(p.name.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
                Cell::from(format!("{:.1}%", p.cpu_pct))
                    .style(Style::default().fg(cpu_color)),
                Cell::from(format_bytes(p.mem_bytes)),
                Cell::from(p.user.as_str()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(p.status.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(7),
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(10),
    ];

    let count = app.processes.list.len();
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default()
            .title(format!(" Processes ({}) ", count))
            .borders(Borders::ALL))
        .row_highlight_style(selected_style)
        .highlight_symbol("› ");

    let mut state = app.processes.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_hints(f: &mut Frame, _app: &App, area: Rect) {
    let hints = Paragraph::new(Span::styled(
        " [k] kill  [c] sort CPU  [m] sort mem  [p] sort PID  [r] refresh ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, area);
}

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1_048_576;
    const GB: u64 = 1_073_741_824;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", bytes / 1024)
    }
}
