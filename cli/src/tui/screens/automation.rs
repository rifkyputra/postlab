use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::{App, InputMode, ZeroclawTab};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.automation.active_tab {
        ZeroclawTab::Overview => render_overview(f, app, chunks[2]),
        ZeroclawTab::Channels => render_channels(f, app, chunks[2]),
        ZeroclawTab::Cron => render_cron(f, app, chunks[2]),
        ZeroclawTab::Memory => render_memory(f, app, chunks[2]),
        ZeroclawTab::Config => render_config(f, app, chunks[2]),
    }

    render_hints(f, app, chunks[3]);

    if app.automation.cron_form_mode == InputMode::Editing {
        render_cron_form(f, app, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.automation.info.installed {
        if app.automation.status.daemon_running {
            (Color::Green, "●")
        } else {
            (Color::Yellow, "●")
        }
    } else {
        (Color::Red, "○")
    };
    let version = app
        .automation
        .info
        .version
        .as_deref()
        .unwrap_or("not installed");
    let daemon_state = if !app.automation.info.installed {
        ""
    } else if app.automation.status.daemon_running {
        "  daemon running"
    } else {
        "  daemon stopped"
    };
    let loading = if app.automation.loading { "  Loading…" } else { "" };
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" ZeroClaw  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::styled(daemon_state, Style::default().fg(Color::DarkGray)),
        Span::styled(loading, Style::default().fg(Color::Yellow)),
    ]);
    let p = Paragraph::new(text)
        .block(Block::default().title(" Automation ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = ZeroclawTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .select(app.automation.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.automation.active_tab {
        ZeroclawTab::Overview => {
            if app.automation.info.installed {
                "[←/→] tabs  [s] start daemon  [S] stop  [i] install service  [u] update check  [U] update  [d] doctor  [r] refresh"
            } else if app.automation.installing {
                "Installing zeroclaw…  please wait"
            } else {
                "[←/→] tabs  [I] install zeroclaw  [r] refresh"
            }
        }
        ZeroclawTab::Channels => "[←/→] tabs  [↑↓/jk] select  [r] refresh",
        ZeroclawTab::Cron => "[←/→] tabs  [↑↓/jk] select  [a] add  [d] delete  [r] refresh",
        ZeroclawTab::Memory => "[←/→] tabs  [↑↓/jk] select  [d] delete  [r] refresh",
        ZeroclawTab::Config => "[←/→] tabs  [↑↓/jk] scroll  [r] refresh",
    };
    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

// ── Overview ──────────────────────────────────────────────────────────────

fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    if !app.automation.info.installed {
        // Show install log if in progress, otherwise the install prompt
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  ZeroClaw is not installed.",
                Style::default().fg(Color::Red),
            )]),
            Line::from(""),
        ];
        if app.automation.installing || !app.automation.install_log.is_empty() {
            for entry in &app.automation.install_log {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", entry),
                    Style::default().fg(Color::Yellow),
                )]));
            }
            if app.automation.installing {
                lines.push(Line::from(vec![Span::styled(
                    "  Installing…",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("  Press ", Style::default().fg(Color::DarkGray)),
                Span::styled("[I]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(
                    " to install from GitHub Releases (pre-built binary, no Rust needed)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  Installs to ~/.local/bin/zeroclaw",
                Style::default().fg(Color::DarkGray),
            )]));
        }
        let p = Paragraph::new(lines).block(Block::default().title(" ZeroClaw ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(6), Constraint::Min(0)])
        .split(area);

    // Status summary
    let daemon_color = if app.automation.status.daemon_running {
        Color::Green
    } else {
        Color::Red
    };
    let daemon_text = if app.automation.status.daemon_running {
        "● Running"
    } else {
        "○ Stopped"
    };
    let status_lines = vec![
        Line::from(vec![
            Span::styled("  Daemon:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(daemon_text, Style::default().fg(daemon_color)),
        ]),
        Line::from(vec![
            Span::styled("  Gateway: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("port {}", app.automation.status.gateway_port),
                Style::default().fg(Color::White),
            ),
        ]),
    ];
    let p = Paragraph::new(status_lines)
        .block(Block::default().title(" Status ").borders(Borders::ALL));
    f.render_widget(p, chunks[0]);

    // Components table
    if app.automation.status.components.is_empty() {
        let raw = app.automation.status.raw.trim();
        let content = if raw.is_empty() {
            "  No component data — is the daemon running?".to_string()
        } else {
            raw.to_string()
        };
        let p = Paragraph::new(content)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title(" Components ").borders(Borders::ALL));
        f.render_widget(p, chunks[1]);
    } else {
        let header = Row::new(vec!["Component", "Status"])
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
        let rows: Vec<Row> = app
            .automation
            .status
            .components
            .iter()
            .map(|c| {
                let status_color = match c.status.as_str() {
                    "ok" => Color::Green,
                    "error" => Color::Red,
                    "degraded" => Color::Yellow,
                    _ => Color::DarkGray,
                };
                Row::new(vec![
                    Cell::from(c.name.clone()),
                    Cell::from(c.status.clone())
                        .style(Style::default().fg(status_color)),
                ])
            })
            .collect();
        let table = Table::new(rows, [Constraint::Fill(1), Constraint::Length(10)])
            .header(header)
            .block(Block::default().title(" Components ").borders(Borders::ALL));
        f.render_widget(table, chunks[1]);
    }
}

// ── Channels ──────────────────────────────────────────────────────────────

fn render_channels(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.channels.is_empty() {
        let msg = "  No channels configured in ~/.zeroclaw/config.toml";
        let p = Paragraph::new(msg)
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title(" Channels ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Name", "Platform", "Enabled"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = app
        .automation
        .channels
        .iter()
        .map(|ch| {
            let enabled_color = if ch.enabled { Color::Green } else { Color::DarkGray };
            let enabled_text = if ch.enabled { "yes" } else { "no" };
            Row::new(vec![
                Cell::from(ch.name.clone()),
                Cell::from(ch.platform.clone()),
                Cell::from(enabled_text).style(Style::default().fg(enabled_color)),
            ])
        })
        .collect();

    let title = format!(" Channels ({}) ", app.automation.channels.len());
    let widths = [Constraint::Fill(1), Constraint::Length(12), Constraint::Length(8)];
    let mut state = app.automation.channels_state.clone();
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(table, area, &mut state);
}

// ── Cron ──────────────────────────────────────────────────────────────────

fn render_cron(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.cron.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No cron jobs.  Press [a] to add one.",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p = Paragraph::new(msg)
            .block(Block::default().title(" Cron Jobs ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["ID", "Schedule", "Command", "Last Run", "Next Run"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    let rows: Vec<Row> = app
        .automation
        .cron
        .iter()
        .map(|job| {
            Row::new(vec![
                Cell::from(job.id.as_str()),
                Cell::from(job.schedule.as_str()),
                Cell::from(job.command.as_str()),
                Cell::from(job.last_run.as_deref().unwrap_or("-")),
                Cell::from(job.next_run.as_deref().unwrap_or("-")),
            ])
        })
        .collect();

    let title = format!(" Cron Jobs ({}) ", app.automation.cron.len());
    let widths = [
        Constraint::Length(10),
        Constraint::Length(16),
        Constraint::Fill(1),
        Constraint::Length(16),
        Constraint::Length(16),
    ];
    let mut state = app.automation.cron_state.clone();
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(table, area, &mut state);
}

// ── Memory ────────────────────────────────────────────────────────────────

fn render_memory(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.memory.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No memory entries.  Run the zeroclaw agent to build memory.",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p = Paragraph::new(msg)
            .block(Block::default().title(" Memory ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let items: Vec<ListItem> = app
        .automation
        .memory
        .iter()
        .map(|e| {
            let line = Line::from(vec![
                Span::styled(
                    format!("  {:30}", e.key),
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", e.preview),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = format!(" Memory ({}) ", app.automation.memory.len());
    let mut state = app.automation.memory_state.clone();
    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(list, area, &mut state);
}

// ── Config ────────────────────────────────────────────────────────────────

fn render_config(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.automation.config_text.is_empty() {
        "  Loading config…  (press [r] to refresh)"
    } else {
        &app.automation.config_text
    };
    let p = Paragraph::new(text)
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" ~/.zeroclaw/config.toml (secrets masked) ")
                .borders(Borders::ALL),
        )
        .scroll((app.automation.config_scroll, 0));
    f.render_widget(p, area);
}

// ── Cron-add form popup ───────────────────────────────────────────────────

fn render_cron_form(f: &mut Frame, app: &App, area: Rect) {
    let w = area.width.min(60).max(40);
    let h = 7u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // padding
            Constraint::Length(1), // schedule label
            Constraint::Length(1), // schedule input
            Constraint::Length(1), // command label
            Constraint::Length(1), // command input
            Constraint::Length(1), // hint
        ])
        .margin(1)
        .split(popup);

    let block = Block::default()
        .title(" Add Cron Job ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(block, popup);

    let sched_style = if app.automation.cron_form_focus == 0 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let cmd_style = if app.automation.cron_form_focus == 1 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    f.render_widget(
        Paragraph::new("Schedule (cron expr):").style(Style::default().fg(Color::DarkGray)),
        inner[1],
    );
    f.render_widget(
        Paragraph::new(format!(
            "{}{}",
            app.automation.cron_form_schedule,
            if app.automation.cron_form_focus == 0 { "█" } else { "" }
        ))
        .style(sched_style),
        inner[2],
    );
    f.render_widget(
        Paragraph::new("Command:").style(Style::default().fg(Color::DarkGray)),
        inner[3],
    );
    f.render_widget(
        Paragraph::new(format!(
            "{}{}",
            app.automation.cron_form_command,
            if app.automation.cron_form_focus == 1 { "█" } else { "" }
        ))
        .style(cmd_style),
        inner[4],
    );
    f.render_widget(
        Paragraph::new("[Tab] switch  [Enter] save  [Esc] cancel")
            .style(Style::default().fg(Color::DarkGray)),
        inner[5],
    );
}
