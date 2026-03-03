use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode, ACTIONS, PROTOS};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    render_status(f, app, chunks[0]);
    render_rules(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);

    if app.firewall.input_mode == InputMode::Editing {
        render_add_popup(f, app, area);
    }
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot, state_label) = match app.firewall.enabled {
        Some(true) => (Color::Green, "●", "active"),
        Some(false) => (Color::Red, "○", "inactive"),
        None => (Color::DarkGray, "○", "unknown"),
    };
    let backend = if app.firewall.backend.is_empty() {
        "detecting…".to_string()
    } else {
        app.firewall.backend.clone()
    };
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(format!(" {}  ", backend)),
        Span::styled(state_label, Style::default().fg(dot_color).add_modifier(Modifier::BOLD)),
    ]);
    let p = Paragraph::new(text)
        .block(Block::default().title(" Firewall ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_rules(f: &mut Frame, app: &App, area: Rect) {
    if app.firewall.backend == "none" || (app.firewall.backend.is_empty() && app.firewall.enabled.is_none()) {
        let p = Paragraph::new(Span::styled(
            "No supported firewall detected (ufw not found).",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Rules ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("#"),
        Cell::from("To / Port"),
        Cell::from("Action"),
        Cell::from("From"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.firewall.rules.iter().map(|rule| {
        let action_color = if rule.action.contains("ALLOW") {
            Color::Green
        } else if rule.action.contains("DENY") || rule.action.contains("REJECT") {
            Color::Red
        } else {
            Color::Yellow
        };
        Row::new([
            Cell::from(rule.num.to_string()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(rule.to.as_str()).style(Style::default().fg(Color::Cyan)),
            Cell::from(rule.action.as_str()).style(Style::default().fg(action_color).add_modifier(Modifier::BOLD)),
            Cell::from(rule.from.as_str()).style(Style::default().fg(Color::White)),
        ])
    }).collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Fill(2),
        Constraint::Length(12),
        Constraint::Fill(1),
    ];
    let count = rows.len();
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Rules ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.firewall.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let enabled = app.firewall.enabled.unwrap_or(false);
    let toggle_hint = if enabled { "[d] disable" } else { "[e] enable" };
    let msg = format!(
        " [a] add rule  [D] delete  {}  [r] refresh ",
        toggle_hint
    );
    let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

fn render_add_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(52, 10, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // port
            Constraint::Length(3), // proto
            Constraint::Length(3), // from
            Constraint::Length(1), // hints
        ])
        .split(popup_area);

    let cursor = "█";

    // Field styles: yellow when focused
    let style = |focus: usize, field: usize| {
        if focus == field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        }
    };

    // Port field (focus=0)
    let port_text = if app.firewall.input_focus == 0 {
        format!("{}{}", app.firewall.input_port, cursor)
    } else {
        app.firewall.input_port.clone()
    };
    let port_p = Paragraph::new(port_text)
        .style(style(app.firewall.input_focus, 0))
        .block(Block::default().title(" Port ").borders(Borders::ALL));
    f.render_widget(port_p, chunks[0]);

    // Proto selector (focus=1) — cycle with Space/Left/Right
    let proto_val = PROTOS[app.firewall.input_proto];
    let proto_text = if app.firewall.input_focus == 1 {
        format!("‹ {} ›", proto_val)
    } else {
        proto_val.to_string()
    };
    let proto_p = Paragraph::new(proto_text)
        .style(style(app.firewall.input_focus, 1))
        .block(Block::default().title(" Protocol ").borders(Borders::ALL));
    f.render_widget(proto_p, chunks[1]);

    // From field — split chunks[2] horizontally: from text (left) + action selector (right)
    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Length(14)])
        .split(chunks[2]);

    let from_text = if app.firewall.input_focus == 2 {
        format!("{}{}", app.firewall.input_from, cursor)
    } else if app.firewall.input_from.is_empty() {
        String::new()
    } else {
        app.firewall.input_from.clone()
    };
    let from_p = Paragraph::new(from_text)
        .style(style(app.firewall.input_focus, 2))
        .block(Block::default().title(" From (blank=any) ").borders(Borders::ALL));
    f.render_widget(from_p, inner[0]);

    // Action selector (focus=3)
    let action_val = ACTIONS[app.firewall.input_action];
    let action_color = if action_val == "allow" { Color::Green } else { Color::Red };
    let action_text = if app.firewall.input_focus == 3 {
        format!("‹ {} ›", action_val.to_uppercase())
    } else {
        action_val.to_uppercase()
    };
    let action_p = Paragraph::new(action_text)
        .style(style(app.firewall.input_focus, 3).fg(action_color))
        .block(Block::default().title(" Action ").borders(Borders::ALL));
    f.render_widget(action_p, inner[1]);

    let hints_p = Paragraph::new(Span::styled(
        " [Tab] next  [Space] cycle  [Enter] confirm  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints_p, chunks[3]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}
