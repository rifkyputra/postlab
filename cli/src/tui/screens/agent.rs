use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::{AgentRole, AgentTab, App, InputMode};
use ratatui::text::Text;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let uses_input = app.agent.active_tab == AgentTab::Chat;

    let constraints: Vec<Constraint> = if uses_input {
        vec![
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // messages
            Constraint::Length(3), // input
            Constraint::Length(1), // hints
        ]
    } else {
        vec![
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // hints
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    render_tab_bar(f, app, chunks[0]);

    match app.agent.active_tab {
        AgentTab::Chat => {
            render_messages(f, app, chunks[1]);
            render_input(f, app, chunks[2]);
            render_hints(f, app, chunks[3]);
        }
        AgentTab::Tools => {
            render_tools(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Tasks => {
            render_tasks(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Status => {
            render_status(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Sessions => {
            render_sessions(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Config => {
            render_config(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Auth => {
            render_auth(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Skills => {
            render_skills(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Library => {
            render_library(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
        AgentTab::Logs => {
            render_logs(f, app, chunks[1]);
            render_hints(f, app, chunks[2]);
        }
    }
}

fn render_tab_bar(f: &mut Frame, app: &App, area: Rect) {
    let status_style = if app.agent.rpc_active {
        if app.agent.streaming {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Green)
        }
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = Line::from(vec![
        Span::raw(" Agent "),
        Span::styled(&app.agent.status, status_style),
        Span::raw(" "),
    ]);

    let titles: Vec<&str> = AgentTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(title))
        .select(app.agent.active_tab.index())
        .highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for msg in &app.agent.messages {
        match msg.role {
            AgentRole::User => {
                let first = msg.content.lines().next().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled("[You] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    Span::raw(first.to_string()),
                ]));
                for rest in msg.content.lines().skip(1) {
                    lines.push(Line::from(format!("      {}", rest)));
                }
            }
            AgentRole::Assistant => {
                let first = msg.content.lines().next().unwrap_or("");
                lines.push(Line::from(vec![
                    Span::styled(" [Pi] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                    Span::raw(first.to_string()),
                ]));
                for rest in msg.content.lines().skip(1) {
                    lines.push(Line::from(format!("      {}", rest)));
                }
            }
            AgentRole::Tool => {
                lines.push(Line::from(Span::styled(
                    format!("  {}", msg.content),
                    Style::default().fg(Color::Yellow),
                )));
            }
        }
        lines.push(Line::from(""));
    }

    if app.agent.streaming {
        lines.push(Line::from(Span::styled(
            "  ▊",
            Style::default().fg(Color::Green).add_modifier(Modifier::SLOW_BLINK),
        )));
    }

    if app.agent.messages.is_empty() && !app.agent.rpc_active {
        lines.push(Line::from(Span::styled(
            "  No session active. Press [s] to connect the agent.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let scroll = (total.saturating_sub(inner_height)) as u16;

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Messages "))
        .scroll((scroll, 0));
    f.render_widget(para, area);
}

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let (cursor, border_style) = if app.agent.input_mode == InputMode::Editing {
        ("█", Style::default().fg(Color::Cyan))
    } else {
        ("", Style::default().fg(Color::DarkGray))
    };

    let content = format!("> {}{}", app.agent.input, cursor);
    let para = Paragraph::new(content).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(" Input "),
    );
    f.render_widget(para, area);
}

fn render_tools(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    for entry in &app.agent.tool_log {
        let color = if entry.starts_with('✓') {
            Color::Green
        } else if entry.starts_with('✗') {
            Color::Red
        } else {
            Color::Yellow
        };
        lines.push(Line::from(Span::styled(
            format!("  {}", entry),
            Style::default().fg(color),
        )));
    }

    if app.agent.tool_log.is_empty() {
        lines.push(Line::from(Span::styled(
            "  No tool executions yet.",
            Style::default().fg(Color::DarkGray),
        )));
    }

    let inner_height = area.height.saturating_sub(2) as usize;
    let total = lines.len();
    let scroll = (total.saturating_sub(inner_height)) as u16;

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" Tool Log "))
        .scroll((scroll, 0));
    f.render_widget(para, area);
}

// ── Tasks tab ─────────────────────────────────────────────────────────────

fn render_tasks(f: &mut Frame, app: &App, area: Rect) {
    if app.agent.task_form_open {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(4), Constraint::Length(7)])
            .split(area);
        render_tasks_table(f, app, chunks[0]);
        render_task_form(f, app, chunks[1]);
    } else {
        render_tasks_table(f, app, area);
    }
}

fn render_tasks_table(f: &mut Frame, app: &App, area: Rect) {
    use crate::db::agent_tasks::schedule_secs;

    let header = Row::new(vec![
        Cell::from("Name").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Interval").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Last Run").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Status").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("On").style(Style::default().add_modifier(Modifier::BOLD)),
    ])
    .style(Style::default().fg(Color::DarkGray))
    .height(1);

    let now = chrono::Utc::now().timestamp();

    let rows: Vec<Row> = app
        .agent
        .tasks
        .iter()
        .map(|t| {
            let last_run = match t.last_run_at {
                None => "never".to_string(),
                Some(ts) => {
                    let secs = (now - ts).max(0) as u64;
                    if secs < 120 {
                        format!("{}s ago", secs)
                    } else if secs < 7200 {
                        format!("{}m ago", secs / 60)
                    } else {
                        format!("{}h ago", secs / 3600)
                    }
                }
            };

            let interval = schedule_secs(&t.schedule);
            let due = match t.last_run_at {
                None => true,
                Some(last) => now - last >= interval,
            };

            let (status_text, status_color) = if !t.enabled {
                ("disabled", Color::DarkGray)
            } else if due {
                ("due", Color::Yellow)
            } else {
                match t.last_success {
                    Some(true) => ("ok", Color::Green),
                    Some(false) => ("failed", Color::Red),
                    None => ("pending", Color::DarkGray),
                }
            };

            let on = if t.enabled { "●" } else { "○" };
            let on_color = if t.enabled { Color::Green } else { Color::DarkGray };

            Row::new(vec![
                Cell::from(t.name.as_str()),
                Cell::from(t.schedule.as_str()),
                Cell::from(last_run),
                Cell::from(status_text).style(Style::default().fg(status_color)),
                Cell::from(on).style(Style::default().fg(on_color)),
            ])
        })
        .collect();

    if rows.is_empty() {
        let empty = Paragraph::new(Span::styled(
            "  No scheduled tasks. Press [n] to create one.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().borders(Borders::ALL).title(" Scheduled Tasks "));
        f.render_widget(empty, area);
        return;
    }

    let widths = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(12),
        Constraint::Length(9),
        Constraint::Length(4),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(" Scheduled Tasks "))
        .row_highlight_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));

    let mut state = app.agent.tasks_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_task_form(f: &mut Frame, app: &App, area: Rect) {
    let schedules = crate::db::agent_tasks::SCHEDULE_OPTIONS;
    let sched = schedules[app.agent.task_form_schedule_idx.min(schedules.len() - 1)];
    let focus = app.agent.task_form_focus;

    let focused = Style::default().fg(Color::Cyan);
    let normal = Style::default().fg(Color::White);

    let lines = vec![
        Line::from(vec![
            Span::styled("  Name:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{}", app.agent.task_form_name, if focus == 0 { "█" } else { "" }),
                if focus == 0 { focused } else { normal },
            ),
        ]),
        Line::from(vec![
            Span::styled("  Prompt:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{}{}", app.agent.task_form_prompt, if focus == 1 { "█" } else { "" }),
                if focus == 1 { focused } else { normal },
            ),
        ]),
        Line::from(vec![
            Span::styled("  Interval: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("[ {} ]", sched),
                if focus == 2 { focused } else { normal },
            ),
            Span::styled("  Tab to cycle", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  [Enter] save   [Esc] cancel   [Tab] next field",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let para = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(" New Task "));
    f.render_widget(para, area);
}

// ── Status tab ────────────────────────────────────────────────────────────

fn render_status(f: &mut Frame, app: &App, area: Rect) {
    if !app.agent.info.installed {
        let mut lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Pi Agent is not installed.",
                Style::default().fg(Color::Red),
            )]),
            Line::from(""),
        ];
        if app.agent.installing || !app.agent.install_log.is_empty() {
            for entry in &app.agent.install_log {
                lines.push(Line::from(vec![Span::styled(
                    format!("  {}", entry),
                    Style::default().fg(Color::Yellow),
                )]));
            }
            if app.agent.installing {
                lines.push(Line::from(vec![Span::styled(
                    "  Installing…",
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                )]));
            }
        } else {
            lines.push(Line::from(vec![
                Span::styled("  Press ", Style::default().fg(Color::DarkGray)),
                Span::styled("[I]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" to install via pi.dev/install.sh (requires curl)", Style::default().fg(Color::DarkGray)),
            ]));
            lines.push(Line::from(""));
            lines.push(Line::from(vec![Span::styled(
                "  Or manually: npm install -g --ignore-scripts @earendil-works/pi-coding-agent",
                Style::default().fg(Color::DarkGray),
            )]));
        }
        let p = Paragraph::new(lines).block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let version = app.agent.info.version.as_deref().unwrap_or("unknown");
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Version:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(version, Style::default().fg(Color::Green)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Config:   ", Style::default().fg(Color::DarkGray)),
            Span::styled("~/.pi/agent/settings.json", Style::default().fg(Color::White)),
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
            Span::styled("  Default model: ", Style::default().fg(Color::DarkGray)),
            Span::styled("deepseek/deepseek-v4-pro  [openrouter]", Style::default().fg(Color::Cyan)),
        ]),
    ];

    if let Some(ref output) = app.agent.action_output {
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
        let p = Paragraph::new(all).block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
    } else {
        let p = Paragraph::new(lines).block(Block::default().title(" Pi Agent ").borders(Borders::ALL));
        f.render_widget(p, area);
    }
}

// ── Sessions tab ──────────────────────────────────────────────────────────

fn render_sessions(f: &mut Frame, app: &App, area: Rect) {
    if app.agent.sessions.is_empty() {
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
        let p = Paragraph::new(msg).block(Block::default().title(" Sessions ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Name", "Modified", "Path"]).style(
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = app
        .agent
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

    let title = format!(" Sessions ({}) ", app.agent.sessions.len());
    let widths = [Constraint::Length(30), Constraint::Length(18), Constraint::Fill(1)];
    let mut state = app.agent.sessions_state.clone();
    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("› ");
    f.render_stateful_widget(table, area, &mut state);
}

// ── Config tab ────────────────────────────────────────────────────────────

fn render_config(f: &mut Frame, app: &App, area: Rect) {
    let searching = !app.agent.config_search.is_empty()
        || app.agent.config_search_mode == InputMode::Editing;

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

    if app.agent.config_text.is_empty() {
        let p = Paragraph::new("  Loading config…  (press [r] to refresh)")
            .style(Style::default().fg(Color::DarkGray))
            .block(Block::default().title(" ~/.pi/agent/settings.json (secrets masked) ").borders(Borders::ALL));
        f.render_widget(p, content_area);
    } else {
        let query = app.agent.config_search.to_lowercase();
        let lines: Vec<Line> = app
            .agent
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
            .block(Block::default().title(" ~/.pi/agent/settings.json (secrets masked) ").borders(Borders::ALL))
            .scroll((app.agent.config_scroll, 0));
        f.render_widget(p, content_area);
    }

    if searching {
        let search_area = chunks[1];
        let in_edit = app.agent.config_search_mode == InputMode::Editing;
        let query_display = if in_edit {
            format!("/{}{}", app.agent.config_search, "█")
        } else {
            format!("/{}", app.agent.config_search)
        };
        let match_count = app
            .agent
            .config_text
            .lines()
            .filter(|l| l.to_lowercase().contains(&app.agent.config_search.to_lowercase()))
            .count();
        let suffix = if app.agent.config_search.is_empty() {
            String::new()
        } else {
            format!("  ({} matches)  [n] next  [N] prev  [Esc] clear", match_count)
        };
        let bar = Line::from(vec![
            Span::styled(query_display, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(suffix, Style::default().fg(Color::DarkGray)),
        ]);
        f.render_widget(Paragraph::new(bar), search_area);
    }
}

// ── Auth tab ──────────────────────────────────────────────────────────────

fn render_auth(f: &mut Frame, app: &App, area: Rect) {
    let log_height = if app.agent.auth_login_output.is_empty() {
        0
    } else {
        (app.agent.auth_login_output.len().min(5) as u16) + 2
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(0),
            Constraint::Length(log_height),
        ])
        .split(area);

    // ── Model config section ──
    let editing = app.agent.auth_model_input_mode == InputMode::Editing;
    let model_display = if editing {
        format!("{}█", app.agent.auth_model_input)
    } else {
        app.agent.auth_model.clone()
    };
    let provider_display = app.agent.auth_provider.as_str();
    let model_lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  Provider : ", Style::default().fg(Color::DarkGray)),
            Span::styled(provider_display, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("  [p] cycle", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM)),
        ]),
        Line::from(vec![
            Span::styled("  Model    : ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &model_display,
                Style::default()
                    .fg(if editing { Color::Yellow } else { Color::Green })
                    .add_modifier(if editing { Modifier::BOLD } else { Modifier::empty() }),
            ),
            if editing {
                Span::styled("  Enter to save · Esc to cancel", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM))
            } else {
                Span::styled("  [m] edit", Style::default().fg(Color::DarkGray).add_modifier(Modifier::DIM))
            },
        ]),
        Line::from(""),
    ];
    let model_title = if editing { " Model Config (editing) " } else { " Model Config " };
    let model_block = Block::default().title(model_title).borders(Borders::ALL)
        .border_style(if editing { Style::default().fg(Color::Yellow) } else { Style::default() });
    f.render_widget(Paragraph::new(model_lines).block(model_block), chunks[0]);

    // ── Provider auth table ──
    let header = Row::new(vec!["Provider", "Status"]).style(
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    );
    let rows: Vec<Row> = app
        .agent
        .auth_entries
        .iter()
        .map(|e| {
            let status_color = if e.status == "configured" { Color::Green } else { Color::Red };
            Row::new(vec![
                Cell::from(e.provider.as_str()),
                Cell::from(e.status.as_str()).style(Style::default().fg(status_color)),
            ])
        })
        .collect();

    let login_hint = if app.agent.auth_login_running { "  logging in…" } else { "  [l] login  [r] refresh" };
    let provider_title = format!(" Providers ({}) ", app.agent.auth_entries.len());
    let mut state = app.agent.auth_state.clone();

    if app.agent.auth_entries.is_empty() {
        let empty = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  No providers in ~/.pi/auth.json", Style::default().fg(Color::DarkGray))]),
            Line::from(vec![Span::styled("  Select a provider row and press [l] to login.", Style::default().fg(Color::DarkGray))]),
        ];
        f.render_widget(Paragraph::new(empty).block(Block::default().title(provider_title).borders(Borders::ALL)), chunks[1]);
    } else {
        let table = Table::new(rows, [Constraint::Length(20), Constraint::Fill(1)])
            .header(header)
            .block(Block::default().title(provider_title).borders(Borders::ALL))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(table, chunks[1], &mut state);
    }

    // hint bar (replaces the shared hint bar for this tab only)
    if chunks[1].height > 2 {
        let hint = Paragraph::new(Line::from(vec![
            Span::styled(login_hint, Style::default().fg(Color::DarkGray)),
        ]));
        // render inside bottom of provider block — skip if log area takes over
        let _ = hint;
    }

    // ── Login output log ──
    if log_height > 0 {
        let log_lines: Vec<Line> = app
            .agent
            .auth_login_output
            .iter()
            .rev()
            .take(5)
            .rev()
            .map(|l| Line::from(Span::styled(l.as_str(), Style::default().fg(Color::Yellow))))
            .collect();
        let log_color = if app.agent.auth_login_running { Color::Yellow } else { Color::DarkGray };
        f.render_widget(
            Paragraph::new(log_lines).block(
                Block::default().title(" Login Output ").borders(Borders::ALL)
                    .border_style(Style::default().fg(log_color)),
            ),
            chunks[2],
        );
    }
}

// ── Skills tab ────────────────────────────────────────────────────────────

fn render_skills(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    if app.agent.skills.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No skills found in ~/.pi/agent/npm/node_modules/",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p = Paragraph::new(msg).block(Block::default().title(" Skills ").borders(Borders::ALL));
        f.render_widget(p, chunks[0]);
    } else {
        let header = Row::new(vec!["Name", "Description"]).style(
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );
        let rows: Vec<Row> = app
            .agent
            .skills
            .iter()
            .map(|s| {
                Row::new(vec![
                    Cell::from(s.name.as_str()),
                    Cell::from(s.description.as_str()).style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let title = format!(" Skills ({}) ", app.agent.skills.len());
        let widths = [Constraint::Length(24), Constraint::Fill(1)];
        let mut state = app.agent.skills_state.clone();
        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().title(title).borders(Borders::ALL))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(table, chunks[0], &mut state);
    }

    let status_text = app.agent.skills_status.as_deref().unwrap_or("");
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

// ── Library tab ───────────────────────────────────────────────────────────

fn render_library(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    if app.agent.library_skills.is_empty() {
        let msg = vec![
            Line::from(""),
            Line::from(vec![Span::styled(
                "  No skills in postlab library.",
                Style::default().fg(Color::DarkGray),
            )]),
        ];
        let p = Paragraph::new(msg).block(Block::default().title(" Skills Library ").borders(Borders::ALL));
        f.render_widget(p, chunks[0]);
    } else {
        let header = Row::new(vec!["Name", "Status", "Description"]).style(
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );
        let rows: Vec<Row> = app
            .agent
            .library_skills
            .iter()
            .map(|s| {
                let (status, color) = if s.installed {
                    ("installed", Color::Green)
                } else {
                    ("available", Color::Yellow)
                };
                Row::new(vec![
                    Cell::from(s.name.as_str()),
                    Cell::from(status).style(Style::default().fg(color)),
                    Cell::from(s.description.as_str()).style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect();

        let title = format!(" Skills Library ({}) ", app.agent.library_skills.len());
        let widths = [Constraint::Length(22), Constraint::Length(12), Constraint::Fill(1)];
        let mut state = app.agent.library_state.clone();
        let table = Table::new(rows, widths)
            .header(header)
            .block(Block::default().title(title).borders(Borders::ALL))
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("› ");
        f.render_stateful_widget(table, chunks[0], &mut state);
    }

    let status_text = app.agent.library_status.as_deref().unwrap_or("");
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

// ── Logs tab ──────────────────────────────────────────────────────────────

fn render_logs(f: &mut Frame, app: &App, area: Rect) {
    if app.agent.logs.is_empty() {
        let p = Paragraph::new(
            "  No session log found. Run pi to create a session, then refresh.",
        )
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().title(" Session Log ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let follow_indicator = if app.agent.logs_follow { " [following]" } else { "" };
    let title = format!(" Session Log{} ", follow_indicator);

    let lines: Vec<Line> = app
        .agent
        .logs
        .iter()
        .map(|l| Line::from(Span::styled(l.as_str(), Style::default().fg(Color::White))))
        .collect();

    let p = Paragraph::new(Text::from(lines))
        .block(Block::default().title(title).borders(Borders::ALL))
        .scroll((app.agent.logs_scroll, 0));
    f.render_widget(p, area);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = match app.agent.active_tab {
        AgentTab::Chat => {
            if app.agent.input_mode == InputMode::Editing {
                "[Enter] send  [Esc] cancel"
            } else if app.agent.rpc_active {
                "[i/Enter] type  [x] disconnect  [←/→] tabs"
            } else {
                "[s] start session  [←/→] tabs"
            }
        }
        AgentTab::Tools => "[←/→] tabs",
        AgentTab::Tasks => {
            if app.agent.task_form_open {
                "[Tab] next field  [Enter] save  [Esc] cancel"
            } else {
                "[n] new  [d] delete  [t] toggle  [Enter] run now  [←/→] tabs"
            }
        }
        AgentTab::Status => {
            if app.agent.info.installed {
                "[←/→] tabs  [u] update check  [U] update  [r] refresh"
            } else if app.agent.installing {
                "Installing pi…  please wait"
            } else {
                "[←/→] tabs  [I] install pi  [r] refresh"
            }
        }
        AgentTab::Sessions => "[←/→] tabs  [↑↓/jk] select  [r] refresh",
        AgentTab::Config => "[←/→] tabs  [↑↓/jk] scroll  [/] search  [n/N] next/prev  [r] refresh",
        AgentTab::Auth => "[←/→] tabs  [↑↓/jk] select provider  [l] login  [m] edit model  [p] cycle provider  [r] refresh",
        AgentTab::Skills => "[←/→] tabs  [↑↓/jk] select  [d] remove  [r] refresh",
        AgentTab::Library => "[←/→] tabs  [↑↓/jk] select  [i/Enter] install  [r] refresh",
        AgentTab::Logs => "[←/→] tabs  [↑↓/jk] scroll  [f] toggle follow  [R] reload",
    };
    let para = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(para, area);
}
