use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::core::portcheck::PortStatus;
use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // IP banner
            Constraint::Min(0),    // port table
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_ip_banner(f, app, chunks[0]);
    render_port_table(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);

    if app.portchecker.input_mode == InputMode::Editing {
        render_add_popup(f, app, area);
    }
}

// ── IP banner ──────────────────────────────────────────────────────────────

fn render_ip_banner(f: &mut Frame, app: &App, area: Rect) {
    let (ip_span, status_span) = if app.portchecker.ip_loading {
        (
            Span::styled("fetching…", Style::default().fg(Color::Yellow)),
            Span::raw(""),
        )
    } else if let Some(ip) = &app.portchecker.public_ip {
        let checking = if app.portchecker.checking {
            Span::styled("  checking ports via portchecker.co…", Style::default().fg(Color::Yellow))
        } else {
            Span::raw("")
        };
        (Span::styled(ip.clone(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)), checking)
    } else {
        (
            Span::styled("unknown — press [r] to fetch", Style::default().fg(Color::DarkGray)),
            Span::raw(""),
        )
    };

    let line = Line::from(vec![
        Span::styled("Public IP: ", Style::default().fg(Color::White)),
        ip_span,
        status_span,
    ]);

    let p = Paragraph::new(line)
        .block(Block::default().title(" Port Checker ").borders(Borders::ALL));
    f.render_widget(p, area);
}

// ── Port table ─────────────────────────────────────────────────────────────

fn render_port_table(f: &mut Frame, app: &App, area: Rect) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Port"),
        Cell::from("Label"),
        Cell::from("Status"),
        Cell::from("Note"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.portchecker.entries.iter().map(|entry| {
        let status_style = Style::default()
            .fg(entry.status.color())
            .add_modifier(Modifier::BOLD);

        let note = match &entry.status {
            PortStatus::Open    => Span::styled("reachable from internet", Style::default().fg(Color::Green)),
            PortStatus::Closed  => Span::styled("blocked or not forwarded", Style::default().fg(Color::Red)),
            PortStatus::Checking => Span::styled("checking…", Style::default().fg(Color::Yellow)),
            PortStatus::Error(e) => Span::styled(e.clone(), Style::default().fg(Color::Magenta)),
            PortStatus::Unknown => Span::styled("not checked yet", Style::default().fg(Color::DarkGray)),
        };

        Row::new([
            Cell::from(entry.port.to_string()).style(Style::default().fg(Color::Cyan)),
            Cell::from(entry.label.as_str()).style(Style::default().fg(Color::White)),
            Cell::from(entry.status.label()).style(status_style),
            Cell::from(note),
        ])
    }).collect();

    let count = rows.len();
    let widths = [
        Constraint::Length(6),
        Constraint::Fill(1),
        Constraint::Length(8),
        Constraint::Fill(2),
    ];

    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Ports ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    f.render_widget(table, area);
}

// ── Hints bar ──────────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let checking = app.portchecker.checking;
    let msg = if checking {
        " waiting for portchecker.co… "
    } else {
        " [c] check all  [a] add port  [d] delete  [r] refresh IP  [↑/↓] navigate "
    };
    let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

// ── Add-port popup ─────────────────────────────────────────────────────────

fn render_add_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(50, 7, area);
    f.render_widget(Clear, popup);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // port
            Constraint::Length(3), // label
            Constraint::Length(1), // hints
        ])
        .split(popup);

    let cursor = "█";

    let focused = |field: usize| -> Style {
        if app.portchecker.input_focus == field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        }
    };

    let port_text = if app.portchecker.input_focus == 0 {
        format!("{}{}", app.portchecker.input_port, cursor)
    } else {
        app.portchecker.input_port.clone()
    };
    let port_p = Paragraph::new(port_text)
        .style(focused(0))
        .block(Block::default().title(" Port (1–65535) ").borders(Borders::ALL));
    f.render_widget(port_p, chunks[0]);

    let label_text = if app.portchecker.input_focus == 1 {
        format!("{}{}", app.portchecker.input_label, cursor)
    } else if app.portchecker.input_label.is_empty() {
        String::new()
    } else {
        app.portchecker.input_label.clone()
    };
    let label_p = Paragraph::new(label_text)
        .style(focused(1))
        .block(Block::default().title(" Label (optional) ").borders(Borders::ALL));
    f.render_widget(label_p, chunks[1]);

    let hints = Paragraph::new(Span::styled(
        " [Tab] next  [Enter] add  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, chunks[2]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}
