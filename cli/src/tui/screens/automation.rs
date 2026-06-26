use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::{App, PiAgentTab};
use ratatui::text::Text;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.automation.active_tab {
        PiAgentTab::Status => render_status(f, app, chunks[2]),
        PiAgentTab::Sessions => render_sessions(f, app, chunks[2]),
        PiAgentTab::Config => render_config(f, app, chunks[2]),
        PiAgentTab::Auth => render_auth(f, app, chunks[2]),
        PiAgentTab::Skills => render_skills(f, app, chunks[2]),
        PiAgentTab::Logs => render_logs(f, app, chunks[2]),
    }

    render_hints(f, app, chunks[3]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.automation.info.installed {
        (Color::Green, "●")
    } else {
        (Color::Red, "○")
    };
    let version = app
        .automation
        .info
        .version
        .as_deref()
        .unwrap_or("not installed");
    let loading = if app.automation.loading { "  Loading…" } else { "" };
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" Pi Agent  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::styled(loading, Style::default().fg(Color::Yellow)),
    ]);
    let p =
        Paragraph::new(text).block(Block::default().title(" Automation ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = PiAgentTab::all().iter().map(|t| t.title()).collect();
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
        PiAgentTab::Status => {
            if app.automation.info.installed {
                "[←/→] tabs  [u] update check  [U] update  [r] refresh"
            } else if app.automation.installing {
                "Installing pi…  please wait"
            } else {
                "[←/→] tabs  [I] install pi  [r] refresh"
            }
        }
        PiAgentTab::Sessions => "[←/→] tabs  [↑↓/jk] select  [r] refresh",
        PiAgentTab::Config => {
            "[←/→] tabs  [↑↓/jk] scroll  [/] search  [n/N] next/prev  [r] refresh"
        }
        PiAgentTab::Auth => "[←/→] tabs  [↑↓/jk] select  [r] refresh",
        PiAgentTab::Skills => "[←/→] tabs  [↑↓/jk] select  [d] remove  [r] refresh",
        PiAgentTab::Logs => "[←/→] tabs  [↑↓/jk] scroll  [f] toggle follow  [R] reload",
    };
    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

// ── Status ────────────────────────────────────────────────────────────────

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    if !app.automation.info.installed {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Pi Agent is not installed.",
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
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("  Press ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "[I]",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " to install via pi.dev/install.sh (requires curl)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  Or manually: npm install -g --ignore-scripts @earendil-works/pi-coding-agent",
                Style::default().fg(Color::DarkGray),
            )]));
        }
        let p =
            Paragraph::new(lines).block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let version = app.automation.info.version.as_deref().unwrap_or("unknown");
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Version:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(version, Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Config:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "~/.pi/agent/settings.json",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Auth:     ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.pi/auth.json", Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("  Sessions: ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.pi/sessions/", Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "  Default model: ",
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "deepseek/deepseek-v4-pro  [openrouter]",
                Style::default().fg(Color::Cyan),
            ),
        ]),
    ];

    if let Some(ref output) = app.automation.action_output {
        let mut all = lines;
        all.push(Line::from(""));
        all.push(Line::from(vec![Span::styled(
            "  Last action output:",
            Style::default().fg(Color::DarkGray),
        )]));
        for l in output.lines().take(6) {
            all.push(Line::from(vec![Span::styled(
                format!("    {}", l),
                Style::default().fg(Color::Yellow),
            )]));
        }
        let p =
            Paragraph::new(all).block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(lines)
            .block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
    }
}

// ── Sessions ──────────────────────────────────────────────────────────────

fn render_sessions(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.sessions.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No sessions found in ~/.pi/sessions/",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Sessions are created automatically when you run pi.",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p =
            Paragraph::new(msg).block(Block::default().title(" Sessions ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Name", "Modified", "Path"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = app
        .automation
        .sessions
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(s.name.as_str()),
                Cell::from(s.modified.as_str()),
                Cell::from(s.path.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let title = format!(" Sessions ({}) ", app.automation.sessions.len());
    let widths = [
        Constraint::Length(30),
        Constraint::Length(18),
        Constraint::Fill(1),
    ];
    let mut state = app.automation.sessions_state.clone();
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(table, area, &mut state);
}

// ── Config ────────────────────────────────────────────────────────────────

fn render_config(f: &mut Frame, app: &App, area: Rect) {
    let searching = !app.automation.config_search.is_empty()
        || app.automation.config_search_mode == crate::tui::app::InputMode::Editing;

    let chunks = if searching {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    let content_area = chunks[0];

    if app.automation.config_text.is_empty() {
        let p = Paragraph::new("  Loading config…  (press [r] to refresh)")
            .style(Style::default().fg(Color::DarkGray))
            .block(
                Block::default()
                    .title(" ~/.pi/agent/settings.json (secrets masked) ")
                    .borders(Borders::ALL),
            );
        f.render_widget(p, content_area);
    } else {
        let query = app.automation.config_search.to_lowercase();
        let lines: Vec<Line> = app
            .automation
            .config_text
            .lines()
            .map(|line| {
                if !query.is_empty() && line.to_lowercase().contains(&query) {
                    let lower = line.to_lowercase();
                    let mut spans = Vec::new();
                    let mut remaining = line;
                    let mut lower_remaining = lower.as_str();
                    while let Some(pos) = lower_remaining.find(query.as_str()) {
                        if pos > 0 {
                            spans.push(Span::raw(&remaining[..pos]));
                        }
                        spans.push(Span::styled(
                            &remaining[pos..pos + query.len()],
                            Style::default().fg(Color::Black).bg(Color::Yellow),
                        ));
                        remaining = &remaining[pos + query.len()..];
                        lower_remaining = &lower_remaining[pos + query.len()..];
                    }
                    if !remaining.is_empty() {
                        spans.push(Span::raw(remaining));
                    }
                    Line::from(spans)
                } else {
                    Line::from(Span::styled(line, Style::default().fg(Color::White)))
                }
            })
            .collect();

        let p = Paragraph::new(Text::from(lines))
            .block(
                Block::default()
                    .title(" ~/.pi/agent/settings.json (secrets masked) ")
                    .borders(Borders::ALL),
            )
            .scroll((app.automation.config_scroll, 0));
        f.render_widget(p, content_area);
    }

    if searching {
        let search_area = chunks[1];
        let in_edit =
            app.automation.config_search_mode == crate::tui::app::InputMode::Editing;
        let query_display = if in_edit {
            format!("/{}{}", app.automation.config_search, "█")
        } else {
            format!("/{}", app.automation.config_search)
        };
        let match_count = app
            .automation
            .config_text
            .lines()
            .filter(|l| {
                l.to_lowercase()
                    .contains(&app.automation.config_search.to_lowercase())
            })
            .count();
        let suffix = if app.automation.config_search.is_empty() {
            String::new()
        } else {
            format!(
                "  ({} matches)  [n] next  [N] prev  [Esc] clear",
                match_count
            )
        };
        let bar = Line::from(vec![
            Span::styled(
                query_display,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(suffix, Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(bar), search_area);
    }
}

// ── Auth ──────────────────────────────────────────────────────────────────

fn render_auth(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.auth_entries.is_empty() {
        let lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No providers configured in ~/.pi/auth.json",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Add API keys via environment variables (e.g. OPENROUTER_API_KEY)",
                Style::default().fg(Color::DarkGray),
            )]),
            Line::from(vec![Span::styled(
                "  or run pi and use /login inside the agent.",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p =
            Paragraph::new(lines).block(Block::default().title(" Auth ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Provider", "Status"]).style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = app
        .automation
        .auth_entries
        .iter()
        .map(|e| {
            let status_color = if e.status == "configured" {
                Color::Green
            } else {
                Color::Red
            };
            Row::new(vec![
                Cell::from(e.provider.as_str()),
                Cell::from(e.status.as_str()).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    let title = format!(" Auth ({} providers) ", app.automation.auth_entries.len());
    let mut state = app.automation.auth_state.clone();
    let table = Table::new(rows, [Constraint::Length(20), Constraint::Fill(1)])
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(table, area, &mut state);
}

// ── Skills ────────────────────────────────────────────────────────────────

fn render_skills(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    if app.automation.skills.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No skills found in ~/.pi/skills/",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p = Paragraph::new(msg)
            .block(Block::default().title(" Skills ").borders(Borders::ALL));
        f.render_widget(p, chunks[0]);
    } else {
        let header = Row::new(vec!["Name", "Description"]).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
        let rows: Vec<Row> = app
            .automation
            .skills
            .iter()
            .map(|s| {
                Row::new(vec![
                    Cell::from(s.name.as_str()),
                    Cell::from(s.description.as_str())
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let title = format!(" Skills ({}) ", app.automation.skills.len());
        let widths = [Constraint::Length(24), Constraint::Fill(1)];
        let mut state = app.automation.skills_state.clone();
        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().title(title).borders(Borders::ALL))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(table, chunks[0], &mut state);
    }

    let status_text = app.automation.skills_status.as_deref().unwrap_or("");
    let status_style = if status_text.starts_with("Failed") {
        Style::default().fg(Color::Red)
    } else {
        Style::default().fg(Color::Green)
    };
    f.render_widget(
        Paragraph::new(Span::styled(format!("  {}", status_text), status_style)),
        chunks[1],
    );
}

// ── Logs ──────────────────────────────────────────────────────────────────

fn render_logs(f: &mut Frame, app: &App, area: Rect) {
    if app.automation.logs.is_empty() {
        let p = Paragraph::new(
            "  No session log found. Run pi to create a session, then refresh.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().title(" Session Log ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let follow_indicator = if app.automation.logs_follow {
        " [following]"
    } else {
        ""
    };
    let title = format!(" Session Log{} ", follow_indicator);

    let lines: Vec<Line> = app
        .automation
        .logs
        .iter()
        .map(|l| Line::from(Span::styled(l.as_str(), Style::default().fg(Color::White))))
        .collect();

    let p = Paragraph::new(Text::from(lines))
        .block(Block::default().title(title).borders(Borders::ALL))
        .scroll((app.automation.logs_scroll, 0));
    f.render_widget(p, area);
}
