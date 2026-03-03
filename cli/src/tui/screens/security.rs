use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::core::models::Severity;
use crate::tui::app::{App, SecurityTab};
use super::{firewall, portcheck, ssh};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_tabs(f, app, chunks[0]);

    match app.security.active_tab {
        SecurityTab::Findings => {
            render_findings_header(f, app, chunks[1]);
        }
        SecurityTab::Firewall => {
            firewall::render(f, app, chunks[1]);
        }
        SecurityTab::Ports => {
            portcheck::render(f, app, chunks[1]);
        }
        SecurityTab::Ssh => {
            ssh::render(f, app, chunks[1]);
        }
        SecurityTab::Fail2Ban => {
            render_fail2ban(f, app, chunks[1]);
        }
    }

    render_hints(f, app, chunks[2]);
}

// ── Tab bar ───────────────────────────────────────────────────────────────

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = SecurityTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .select(app.security.active_tab.index())
        .block(Block::default().borders(Borders::ALL).title(" Security "))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));
    f.render_widget(tabs, area);
}

// ── Findings tab ──────────────────────────────────────────────────────────

fn render_findings_header(f: &mut Frame, app: &App, area: Rect) {
    // Split vertically: status line (1) + findings list (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Status line
    let scan_status = if app.security.scanning {
        Span::styled(" Scanning…", Style::default().fg(Color::Yellow))
    } else {
        let count = app.security.findings.len();
        let critical = app
            .security
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Critical)
            .count();
        let high = app
            .security
            .findings
            .iter()
            .filter(|f| f.severity == Severity::High)
            .count();
        if let Some(t) = app.security.last_scan {
            let elapsed = t.elapsed().unwrap_or_default().as_secs();
            Span::styled(
                format!(
                    " {} findings ({} critical, {} high)  last scan: {}s ago",
                    count, critical, high, elapsed
                ),
                Style::default().fg(Color::White),
            )
        } else {
            Span::styled(
                " Not scanned — press [s] to scan",
                Style::default().fg(Color::DarkGray),
            )
        }
    };
    f.render_widget(Paragraph::new(Line::from(scan_status)), chunks[0]);

    // Findings list
    render_findings_list(f, app, chunks[1]);
}

fn render_findings_list(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .security
        .findings
        .iter()
        .flat_map(|finding| {
            let selected = app.security.selected.contains(&finding.id);
            let sel_sym = if selected { "[*]" } else { "   " };
            let dot_color = finding.severity.color();
            vec![
                ListItem::new(Line::from(vec![
                    Span::styled(sel_sym, Style::default().fg(Color::Yellow)),
                    Span::raw(" "),
                    Span::styled("●", Style::default().fg(dot_color)),
                    Span::raw(" "),
                    Span::styled(
                        finding.severity.label(),
                        Style::default()
                            .fg(dot_color)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(
                        &finding.title,
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                ListItem::new(Line::from(vec![
                    Span::raw("         "),
                    Span::styled(
                        &finding.description,
                        Style::default().fg(Color::DarkGray),
                    ),
                ])),
                ListItem::new(Line::from(vec![
                    Span::raw("         "),
                    Span::styled("Fix: ", Style::default().fg(Color::Green)),
                    Span::styled(
                        &finding.fix_description,
                        Style::default().fg(Color::Green),
                    ),
                ])),
                ListItem::new(Line::from("")),
            ]
        })
        .collect();

    let empty = if app.security.findings.is_empty() && !app.security.scanning {
        "No findings — press [s] to scan"
    } else {
        ""
    };

    let list = if items.is_empty() {
        List::new(vec![ListItem::new(Span::styled(
            empty,
            Style::default().fg(Color::DarkGray),
        ))])
    } else {
        List::new(items)
            .highlight_style(Style::default().bg(Color::DarkGray))
            .highlight_symbol("")
    };

    let list = list.block(
        Block::default()
            .title(" Security Findings ")
            .borders(Borders::ALL),
    );
    let mut state = app.security.list_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

// ── Fail2Ban tab ──────────────────────────────────────────────────────────

fn render_fail2ban(f: &mut Frame, app: &App, area: Rect) {
    // Header row: summary line (1) + list (rest)
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(area);

    // Summary / status line
    let summary = if app.security.f2b_loading {
        Span::styled(" Loading fail2ban data…", Style::default().fg(Color::Yellow))
    } else if !app.security.f2b_installed {
        Span::styled(
            " fail2ban-client not found — install fail2ban first",
            Style::default().fg(Color::DarkGray),
        )
    } else {
        let n = app.security.jailed.len();
        Span::styled(
            format!(" {} IP{} currently jailed", n, if n == 1 { "" } else { "s" }),
            Style::default().fg(Color::White),
        )
    };
    f.render_widget(Paragraph::new(Line::from(summary)), chunks[0]);

    // Jailed IP list
    render_jailed_list(f, app, chunks[1]);
}

fn render_jailed_list(f: &mut Frame, app: &App, area: Rect) {
    if app.security.jailed.is_empty() {
        let msg = if app.security.f2b_loading {
            ""
        } else if !app.security.f2b_installed {
            "fail2ban not available"
        } else {
            "No IPs currently jailed  ✓"
        };
        let block = Block::default()
            .title(" Jailed IPs ")
            .borders(Borders::ALL);
        let p = Paragraph::new(Span::styled(
            format!(" {}", msg),
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .security
        .jailed
        .iter()
        .map(|entry| {
            ListItem::new(Line::from(vec![
                Span::styled(" ⛔ ", Style::default().fg(Color::Red)),
                Span::styled(
                    format!("{:<18}", entry.ip),
                    Style::default()
                        .fg(Color::LightRed)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  jail: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{:<20}", entry.jail),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled("  failures: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    entry.total_failures.to_string(),
                    Style::default().fg(Color::Yellow),
                ),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .title(" Jailed IPs ")
                .borders(Borders::ALL),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = app.security.jailed_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

// ── Hints bar ─────────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let text = match app.security.active_tab {
        SecurityTab::Findings => {
            " [←/→] switch tab  [s] scan  [Space] toggle  [Enter] apply selected "
        }
        SecurityTab::Firewall => {
            " [←/→] switch tab  [a] add rule  [D] delete  [e/d] enable/disable  [r] refresh "
        }
        SecurityTab::Ports => {
            " [←/→] switch tab  [c] check all  [a] add port  [d] delete  [r] refresh IP "
        }
        SecurityTab::Ssh => {
            " [←/→] switch tab  [r] refresh  [g] generate key  [a/D] authorize/deauthorize "
        }
        SecurityTab::Fail2Ban => {
            " [←/→] switch tab  [↑/↓] navigate  [f] Forgive (unban)  [b] Banish (perm. block)  [r] refresh "
        }
    };
    let hints = Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints, area);
}
