use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode, TunnelPanel};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(rows[1]);

    render_status(f, app, rows[0]);
    render_tunnels(f, app, cols[0]);
    render_right_panel(f, app, cols[1]);
    render_hints(f, app, rows[2]);

    if app.tunnel.input_mode == InputMode::Editing {
        render_input_popup(f, app, area);
    }
    if app.tunnel.input_mode == InputMode::AddingDomain {
        render_add_domain_popup(f, app, area);
    }
    if app.tunnel.input_mode == InputMode::EditingIngress {
        render_edit_ingress_popup(f, app, area);
    }
}

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.tunnel.installed {
        (Color::Green, "●")
    } else {
        (Color::Red, "○")
    };
    let version = app.tunnel.version.as_deref().unwrap_or("not installed");
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" cloudflared  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
    ]);
    let p = Paragraph::new(text)
        .block(Block::default().title(" Cloudflare Tunnel ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_tunnels(f: &mut Frame, app: &App, area: Rect) {
    if !app.tunnel.installed {
        let p = Paragraph::new(Span::styled(
            "cloudflared is not installed. Press [i] to install.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Tunnels ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    if app.tunnel.tunnels.is_empty() {
        let items = vec![
            ListItem::new(Span::styled(
                "No tunnels yet.",
                Style::default().fg(Color::DarkGray),
            )),
            ListItem::new(Span::styled(
                "Press [l] to login, then [a] to create a tunnel.",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let list = List::new(items)
            .block(Block::default().title(" Tunnels ").borders(Borders::ALL));
        f.render_widget(list, area);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Name"),
        Cell::from("ID"),
        Cell::from("Status"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.tunnel.tunnels
        .iter()
        .map(|t| {
            let is_active = app.tunnel.active_tunnel_id.as_deref() == Some(t.id.as_str());
            let name_cell = if is_active {
                Cell::from(format!("★ {}", t.name))
                    .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Cell::from(t.name.as_str())
                    .style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            };
            let status_color = if t.status == "active" { Color::Green } else { Color::DarkGray };
            Row::new([
                name_cell,
                Cell::from(&t.id[..t.id.len().min(12)])
                    .style(Style::default().fg(Color::DarkGray)),
                Cell::from(t.status.as_str())
                    .style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    let widths = [Constraint::Fill(1), Constraint::Length(14), Constraint::Length(10)];
    let count = rows.len();
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Tunnels ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.tunnel.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_right_panel(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(0)])
        .split(area);

    // ── Service status ────────────────────────────────────────────────────
    let (active_dot, active_color, active_label) = match app.tunnel.service_active {
        Some(true)  => ("●", Color::Green,  "active"),
        Some(false) => ("●", Color::Red,    "inactive"),
        None        => ("○", Color::DarkGray, "unknown"),
    };
    let (enabled_label, enabled_color) = match app.tunnel.service_enabled {
        Some(true)  => ("enabled",  Color::Green),
        Some(false) => ("disabled", Color::Yellow),
        None        => ("?",        Color::DarkGray),
    };
    let svc_line = Line::from(vec![
        Span::styled(active_dot, Style::default().fg(active_color)),
        Span::raw(format!(" cloudflared  {active_label}  ")),
        Span::styled(enabled_label, Style::default().fg(enabled_color)),
        Span::styled("  [T]start [X]stop [R]restart [u]sync", Style::default().fg(Color::DarkGray)),
    ]);
    let svc_block = Paragraph::new(svc_line)
        .block(Block::default().title(" Service ").borders(Borders::ALL));
    f.render_widget(svc_block, chunks[0]);

    // ── Ingress rules list ────────────────────────────────────────────────
    let ingress_focused = app.tunnel.panel_focus == TunnelPanel::Ingress;
    let border_style = if ingress_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    let title = if ingress_focused {
        " Ingress Rules  [e]edit  [D]delete  [c]reload  [f]switch panel "
    } else {
        " Ingress Rules  [f]focus  [c]reload "
    };

    if app.tunnel.ingress_entries.is_empty() {
        let msg = match &app.tunnel.config_content {
            None => "Press [Enter] on a tunnel to load its config",
            Some(c) if c.is_empty() || c.contains("not found") => {
                "No config yet — press [d] to add a domain"
            }
            _ => "(no named ingress entries — only catch-all)",
        };
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))
            .block(Block::default().title(title).borders(Borders::ALL).border_style(border_style));
        f.render_widget(p, chunks[1]);
        return;
    }

    let items: Vec<ListItem> = app.tunnel.ingress_entries
        .iter()
        .map(|(host, svc)| {
            ListItem::new(Line::from(vec![
                Span::styled(host.as_str(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled("  →  ", Style::default().fg(Color::DarkGray)),
                Span::styled(svc.as_str(), Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let count = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .title(format!(" Ingress ({})  {} ", count, if ingress_focused { "[e]edit  [D]del  [f]switch" } else { "[f]focus" }))
                .borders(Borders::ALL)
                .border_style(border_style),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("› ");

    let mut state = app.tunnel.ingress_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let text = match app.tunnel.panel_focus {
        TunnelPanel::Ingress => {
            " [f/Esc] tunnels panel  [e] edit  [D] delete  [T]start [X]stop [R]restart [u]sync [c]reload".to_string()
        }
        TunnelPanel::Tunnels => {
            let domain_hint = if app.tunnel.table_state.selected().is_some() { "  [d] add domain" } else { "" };
            format!(
                " [l] login  [a] create  [s] install svc  [f] ingress panel{}  [i] install cloudflared",
                domain_hint,
            )
        }
    };
    let hints = Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints, area);
}

fn render_input_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(55, 10, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let field_style = |focus: usize, current: usize| {
        if focus == current { Style::default().fg(Color::Yellow) } else { Style::default() }
    };
    let cursor = "█";
    let cursor_if = |idx: usize| if app.tunnel.input_focus == idx { cursor } else { "" };

    let name_p = Paragraph::new(format!("{}{}", app.tunnel.input_name, cursor_if(0)))
        .style(field_style(app.tunnel.input_focus, 0))
        .block(Block::default().title(" Tunnel Name ").borders(Borders::ALL));
    let host_p = Paragraph::new(format!("{}{}", app.tunnel.input_host, cursor_if(1)))
        .style(field_style(app.tunnel.input_focus, 1))
        .block(Block::default().title(" Hostname (e.g. app.example.com) ").borders(Borders::ALL));
    let svc_p = Paragraph::new(format!("{}{}", app.tunnel.input_service, cursor_if(2)))
        .style(field_style(app.tunnel.input_focus, 2))
        .block(Block::default().title(" Service (e.g. localhost:3000) ").borders(Borders::ALL));
    let hints = Paragraph::new(Span::styled(
        " [Tab] next  [Enter] create  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));

    f.render_widget(name_p, chunks[0]);
    f.render_widget(host_p, chunks[1]);
    f.render_widget(svc_p, chunks[2]);
    f.render_widget(hints, chunks[3]);
}

fn render_add_domain_popup(f: &mut Frame, app: &App, area: Rect) {
    let selected_name = app.tunnel.table_state.selected()
        .and_then(|i| app.tunnel.tunnels.get(i))
        .map(|t| t.name.as_str())
        .unwrap_or("?");

    let popup_area = centered_rect(55, 8, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let field_style = |focus: usize, current: usize| {
        if focus == current { Style::default().fg(Color::Yellow) } else { Style::default() }
    };
    let cursor = "█";
    let cursor_if = |idx: usize| if app.tunnel.input_focus == idx { cursor } else { "" };

    let host_p = Paragraph::new(format!("{}{}", app.tunnel.input_host, cursor_if(0)))
        .style(field_style(app.tunnel.input_focus, 0))
        .block(Block::default()
            .title(format!(" Hostname — tunnel: {} ", selected_name))
            .borders(Borders::ALL));
    let svc_p = Paragraph::new(format!("{}{}", app.tunnel.input_service, cursor_if(1)))
        .style(field_style(app.tunnel.input_focus, 1))
        .block(Block::default().title(" Service (e.g. localhost:8080) ").borders(Borders::ALL));
    let hints = Paragraph::new(Span::styled(
        " [Tab] next field  [Enter] add  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));

    f.render_widget(host_p, chunks[0]);
    f.render_widget(svc_p, chunks[1]);
    f.render_widget(hints, chunks[2]);
}

fn render_edit_ingress_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(60, 8, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(popup_area);

    let field_style = |focus: usize, current: usize| {
        if focus == current { Style::default().fg(Color::Yellow) } else { Style::default() }
    };
    let cursor = "█";
    let cursor_if = |idx: usize| if app.tunnel.input_focus == idx { cursor } else { "" };

    let host_p = Paragraph::new(format!("{}{}", app.tunnel.input_host, cursor_if(0)))
        .style(field_style(app.tunnel.input_focus, 0))
        .block(Block::default().title(" Hostname ").borders(Borders::ALL));
    let svc_p = Paragraph::new(format!("{}{}", app.tunnel.input_service, cursor_if(1)))
        .style(field_style(app.tunnel.input_focus, 1))
        .block(Block::default().title(" Service (e.g. localhost:8080) ").borders(Borders::ALL));
    let hints = Paragraph::new(Span::styled(
        " [Tab] next field  [Enter] save  [Esc] cancel ",
        Style::default().fg(Color::DarkGray),
    ));

    f.render_widget(host_p, chunks[0]);
    f.render_widget(svc_p, chunks[1]);
    f.render_widget(hints, chunks[2]);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}
