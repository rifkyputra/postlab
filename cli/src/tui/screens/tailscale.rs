use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_status(f, app, chunks[0]);
    render_peers(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let ts = &app.tailscale;

    if ts.loading {
        let p = Paragraph::new(" Loading…")
            .block(Block::default().title(" Tailscale ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let (dot_color, dot) = if ts.installed {
        match ts.backend_state.as_str() {
            "Running" => (Color::Green, "●"),
            "Stopped" | "NoState" => (Color::Yellow, "●"),
            _ => (Color::Red, "○"),
        }
    } else {
        (Color::Red, "○")
    };

    let version = ts.version.as_deref().unwrap_or("not installed");
    let state_label = if ts.backend_state.is_empty() {
        "not installed"
    } else {
        ts.backend_state.as_str()
    };

    let ip_span = match &ts.self_ip {
        Some(ip) => Span::styled(format!("  {}", ip), Style::default().fg(Color::Cyan)),
        None => Span::raw(""),
    };
    let name_span = match &ts.self_name {
        Some(n) => Span::styled(format!("  {}", n), Style::default().fg(Color::DarkGray)),
        None => Span::raw(""),
    };

    let line = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw("  tailscale "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::raw("  "),
        Span::styled(
            state_label,
            Style::default().fg(match state_label {
                "Running" => Color::Green,
                "Stopped" | "NoState" => Color::Yellow,
                _ => Color::DarkGray,
            }),
        ),
        ip_span,
        name_span,
    ]);

    let p = Paragraph::new(line)
        .block(Block::default().title(" Tailscale ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_peers(f: &mut Frame, app: &App, area: Rect) {
    let ts = &app.tailscale;

    let block = Block::default()
        .title(format!(" Peers ({}) ", ts.peers.len()))
        .borders(Borders::ALL);

    if !ts.installed {
        let p = Paragraph::new(Span::styled(
            " Tailscale is not installed. Press [i] to install.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    }

    if ts.peers.is_empty() {
        let msg = match ts.backend_state.as_str() {
            "Running" => " No peers — connect more devices at https://login.tailscale.com",
            "NeedsLogin" => " Run 'tailscale up' then authenticate in the browser",
            _ => " Tailscale is stopped. Press [u] to bring it up.",
        };
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = ts
        .peers
        .iter()
        .map(|p| {
            let (dot, dot_color) = if p.online {
                ("●", Color::Green)
            } else {
                ("○", Color::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(dot, Style::default().fg(dot_color)),
                Span::raw("  "),
                Span::styled(
                    p.name.as_str(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", p.ip),
                    Style::default().fg(Color::Cyan),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = app.tailscale.peers_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let ts = &app.tailscale;
    let text = if !ts.installed {
        " [i] install Tailscale  [r] refresh"
    } else if ts.backend_state == "Running" {
        " [d] tailscale down  [r] refresh"
    } else {
        " [u] tailscale up  [i] (re)install  [r] refresh"
    };
    let hints = Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints, area);
}
