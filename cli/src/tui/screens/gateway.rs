use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    render_status(f, app, chunks[0]);
    render_routes(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);

    // Input popup (rendered on top)
    if app.gateway.input_mode == InputMode::Editing {
        render_input_popup(f, app, area);
    }
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.gateway.installed {
        (Color::Green, "●")
    } else {
        (Color::Red, "○")
    };
    let version = app.gateway.version.as_deref().unwrap_or("not installed");
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" Caddy  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
    ]);
    let p = Paragraph::new(text)
        .block(Block::default().title(" Gateway (Caddy) ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_routes(f: &mut Frame, app: &App, area: Rect) {
    if !app.gateway.installed {
        let p = Paragraph::new(Span::styled(
            "Caddy is not installed. Press [i] to install.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Routes ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Domain"),
        Cell::from("Port"),
        Cell::from("TLS"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.gateway.routes
        .iter()
        .map(|r| {
            Row::new([
                Cell::from(r.domain.as_str())
                    .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Cell::from(format!(":{}", r.port))
                    .style(Style::default().fg(Color::Cyan)),
                Cell::from(if r.tls { "auto" } else { "off" })
                    .style(Style::default().fg(Color::Green)),
            ])
        })
        .collect();

    let widths = [Constraint::Fill(1), Constraint::Length(8), Constraint::Length(6)];
    let count = rows.len();
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Routes ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.gateway.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_hints(f: &mut Frame, _app: &App, area: Rect) {
    let hints = Paragraph::new(Span::styled(
        " [a] add route  [D] delete  [r] reload  [i] install Caddy ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, area);
}

fn render_input_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 7, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(3), Constraint::Length(1)])
        .split(popup_area);

    let domain_style = if app.gateway.input_focus == 0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let port_style = if app.gateway.input_focus == 1 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let cursor = "█";
    let domain_text = if app.gateway.input_focus == 0 {
        format!("{}{}", app.gateway.input_domain, cursor)
    } else {
        app.gateway.input_domain.clone()
    };
    let port_text = if app.gateway.input_focus == 1 {
        format!("{}{}", app.gateway.input_port, cursor)
    } else {
        app.gateway.input_port.clone()
    };

    let domain_p = Paragraph::new(domain_text)
        .style(domain_style)
        .block(Block::default().title(" Domain ").borders(Borders::ALL));
    let port_p = Paragraph::new(port_text)
        .style(port_style)
        .block(Block::default().title(" Port ").borders(Borders::ALL));
    let hints = Paragraph::new(Span::styled(
        " [Tab] next field  [Enter] confirm  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));

    f.render_widget(domain_p, chunks[0]);
    f.render_widget(port_p, chunks[1]);
    f.render_widget(hints, chunks[2]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}
