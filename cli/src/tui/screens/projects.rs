use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::tui::app::{
    App, InputMode, ProjectsTab, BTS_ADDONS, BTS_APIS, BTS_AUTHS, BTS_BACKENDS, BTS_DATABASES,
    BTS_EXAMPLES, BTS_FRONTENDS, BTS_GIT, BTS_ORMS, BTS_PACKAGE_MANAGERS, BTS_PAYMENTS,
    BTS_RUNTIMES, BTS_SERVER_DEPLOY, BTS_WEB_DEPLOY,
};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_tabs(f, app, chunks[0]);

    match app.projects.active_tab {
        ProjectsTab::Projects => render_list(f, app, chunks[1]),
        ProjectsTab::New => render_new(f, app, chunks[1]),
        ProjectsTab::Clone => render_clone(f, app, chunks[1]),
        ProjectsTab::Settings => render_settings(f, app, chunks[1]),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = ProjectsTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" Projects "))
        .select(app.projects.active_tab.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_list(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let dir_label = if app.projects.dir.is_empty() {
        "loading…".to_string()
    } else {
        app.projects.dir.clone()
    };

    let title = if app.projects.loading {
        format!(" Projects — {} (loading…) ", dir_label)
    } else {
        format!(" Projects — {} ({}) ", dir_label, app.projects.list.len())
    };

    let items: Vec<ListItem> = app
        .projects
        .list
        .iter()
        .map(|p| {
            let modified = p
                .modified
                .map(format_age)
                .unwrap_or_else(|| "—".to_string());
            let stack_color = match p.stack.as_str() {
                "Node" => Color::Green,
                "Rust" => Color::Red,
                "Go" => Color::Cyan,
                "Python" => Color::Blue,
                "Docker" => Color::LightBlue,
                "WasmCloud" => Color::Magenta,
                _ => Color::DarkGray,
            };
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<30}", truncate(&p.name, 28)),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    format!("{:<10}", &p.stack),
                    Style::default().fg(stack_color),
                ),
                Span::raw("  "),
                Span::styled(modified, Style::default().fg(Color::DarkGray)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let mut list_state = app.projects.list_state.clone();
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, chunks[0], &mut list_state);

    let hint = if app.projects.list.is_empty() && !app.projects.loading {
        "[r] refresh  (no projects found — check Settings tab for directory)"
    } else {
        "[↑/↓] navigate  [Enter] show path  [p] git pull  [r] refresh"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[1]);
}

fn render_new(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // name input
            Constraint::Length(4), // stack summary (2 lines + 2 borders)
            Constraint::Min(0),    // output
            Constraint::Length(1), // hint
        ])
        .split(area);

    let editing = app.projects.new_input_mode == InputMode::Editing;
    let exists = app.projects.new_name_exists;
    let name_border = if exists {
        Style::default().fg(Color::Red)
    } else if editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if editing { "█" } else { "" };
    let name_text = format!("{}{}", app.projects.new_name, cursor);
    let name_title = if exists {
        " Project name — already exists "
    } else {
        " Project name "
    };
    let name_para = Paragraph::new(name_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(name_border)
            .title(name_title),
    );
    f.render_widget(name_para, chunks[0]);

    // Compact summary of the current stack; the full editor opens in a popup on [s].
    let addons_count = app.projects.new_addons_selected.iter().filter(|&&s| s).count();
    let cyan = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
    let dim = Style::default().fg(Color::DarkGray);
    let summary_lines = vec![
        Line::from(vec![
            Span::styled("frontend ", dim),
            Span::styled(BTS_FRONTENDS[app.projects.new_frontend_idx], cyan),
            Span::styled("  backend ", dim),
            Span::styled(BTS_BACKENDS[app.projects.new_backend_idx], cyan),
            Span::styled("  db ", dim),
            Span::styled(BTS_DATABASES[app.projects.new_database_idx], cyan),
            Span::styled("/", dim),
            Span::styled(BTS_ORMS[app.projects.new_orm_idx], cyan),
            Span::styled("  api ", dim),
            Span::styled(BTS_APIS[app.projects.new_api_idx], cyan),
            Span::styled("  runtime ", dim),
            Span::styled(BTS_RUNTIMES[app.projects.new_runtime_idx], cyan),
        ]),
        Line::from(vec![
            Span::styled("auth ", dim),
            Span::styled(BTS_AUTHS[app.projects.new_auth_idx], cyan),
            Span::styled("  payments ", dim),
            Span::styled(BTS_PAYMENTS[app.projects.new_payments_idx], cyan),
            Span::styled("  pm ", dim),
            Span::styled(BTS_PACKAGE_MANAGERS[app.projects.new_package_manager_idx], cyan),
            Span::styled("  git ", dim),
            Span::styled(BTS_GIT[app.projects.new_git_idx], cyan),
            Span::styled("  deploy ", dim),
            Span::styled(BTS_WEB_DEPLOY[app.projects.new_web_deploy_idx], cyan),
            Span::styled("/", dim),
            Span::styled(BTS_SERVER_DEPLOY[app.projects.new_server_deploy_idx], cyan),
            Span::styled("  addons ", dim),
            Span::styled(addons_count.to_string(), cyan),
        ]),
    ];
    let summary_para = Paragraph::new(summary_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Stack  [c] configure "),
    );
    f.render_widget(summary_para, chunks[1]);

    let running_label = if app.projects.new_running { " (running…)" } else { "" };
    let total = app.projects.new_output.len();
    let visible_h = chunks[2].height.saturating_sub(2) as usize;
    let scroll = app.projects.new_output_scroll.min(total.saturating_sub(visible_h));
    let bottom = total.saturating_sub(scroll);
    let top = bottom.saturating_sub(visible_h);
    let out_lines: Vec<Line> = app.projects.new_output[top..bottom]
        .iter()
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();
    let scroll_label = if scroll > 0 { format!(" ↑{} ", scroll) } else { String::new() };
    let out_para = Paragraph::new(out_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Output{}{} ", running_label, scroll_label)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(out_para, chunks[2]);

    let hint = if editing {
        "[Esc] cancel  [Enter] confirm name"
    } else if app.projects.new_running {
        "scaffolding in progress…  [PgUp/PgDn] scroll"
    } else {
        "[i] name  [c] configure stack  [Enter] scaffold"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[3]);

    // Overlay popups last so they sit above the form and output panels.
    // Stack first, then addons on top of it (addons opens from within the stack popup).
    if app.projects.new_stack_popup {
        render_stack_popup(f, app, area);
    }
    if app.projects.new_addons_popup {
        render_addons_popup(f, app, area);
    }
}

fn render_stack_popup(f: &mut Frame, app: &App, area: Rect) {
    let fields: &[(&str, &[&str], usize)] = &[
        ("Frontend",   BTS_FRONTENDS,     app.projects.new_frontend_idx),
        ("Database",   BTS_DATABASES,     app.projects.new_database_idx),
        ("ORM",        BTS_ORMS,          app.projects.new_orm_idx),
        ("Auth",       BTS_AUTHS,         app.projects.new_auth_idx),
        ("Backend",    BTS_BACKENDS,      app.projects.new_backend_idx),
        ("API",        BTS_APIS,          app.projects.new_api_idx),
        ("Runtime",    BTS_RUNTIMES,      app.projects.new_runtime_idx),
        ("Payments",   BTS_PAYMENTS,      app.projects.new_payments_idx),
        ("Examples",   BTS_EXAMPLES,      app.projects.new_examples_idx),
        ("Git",        BTS_GIT,           app.projects.new_git_idx),
        ("Web Deploy", BTS_WEB_DEPLOY,    app.projects.new_web_deploy_idx),
        ("Srv Deploy", BTS_SERVER_DEPLOY, app.projects.new_server_deploy_idx),
        ("Pkg Mgr",    BTS_PACKAGE_MANAGERS, app.projects.new_package_manager_idx),
    ];
    const ADDONS_FIELD: usize = 13;

    let height = (fields.len() as u16 + 4).min(area.height.saturating_sub(2));
    let popup = centered_rect(54, height, area);
    f.render_widget(Clear, popup);

    let mut lines: Vec<Line> = fields
        .iter()
        .enumerate()
        .map(|(i, (label, opts, sel))| {
            let focused = i == app.projects.new_form_focus;
            let prefix = Span::styled(
                if focused { "> " } else { "  " },
                Style::default().fg(Color::Yellow),
            );
            let label_span = Span::styled(
                format!("{:<11}", label),
                if focused {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                },
            );
            let value = Span::styled(
                opts[*sel],
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            );
            let nav = if focused {
                Span::styled("  ‹ h/l ›", Style::default().fg(Color::DarkGray))
            } else {
                Span::raw("")
            };
            Line::from(vec![prefix, label_span, value, nav])
        })
        .collect();

    // Addons row — opens the nested multi-select popup.
    {
        let focused = app.projects.new_form_focus == ADDONS_FIELD;
        let count = app.projects.new_addons_selected.iter().filter(|&&s| s).count();
        let prefix = Span::styled(
            if focused { "> " } else { "  " },
            Style::default().fg(Color::Yellow),
        );
        let label_span = Span::styled(
            format!("{:<11}", "Addons"),
            if focused {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            },
        );
        let value = Span::styled(
            format!("{} selected", count),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        );
        let nav = if focused {
            Span::styled("  ‹ Enter ›", Style::default().fg(Color::DarkGray))
        } else {
            Span::raw("")
        };
        lines.push(Line::from(vec![prefix, label_span, value, nav]));
    }

    lines.push(Line::from(Span::styled(
        "[j/k ↑/↓] field  [h/l ←/→] change  [Enter] addons/done  [Esc] close",
        Style::default().fg(Color::DarkGray),
    )));

    let para = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow))
            .title(" Configure stack "),
    );
    f.render_widget(para, popup);
}

fn render_addons_popup(f: &mut Frame, app: &App, area: Rect) {
    let height = (BTS_ADDONS.len() as u16 + 3).min(area.height.saturating_sub(2));
    let popup = centered_rect(46, height, area);
    f.render_widget(Clear, popup);

    let selected_count = app.projects.new_addons_selected.iter().filter(|&&s| s).count();
    let items: Vec<ListItem> = BTS_ADDONS
        .iter()
        .enumerate()
        .map(|(i, &addon)| {
            let checked = app.projects.new_addons_selected[i];
            let mark = if checked { "[x]" } else { "[ ]" };
            let style = if checked {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", mark), style),
                Span::styled(addon, style),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.projects.new_addons_cursor));
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow))
                .title(format!(" Addons ({} selected) ", selected_count)),
        )
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("> ");
    f.render_stateful_widget(list, popup, &mut state);

    let hint_area = Rect {
        x: popup.x + 1,
        y: popup.y + popup.height.saturating_sub(2),
        width: popup.width.saturating_sub(2),
        height: 1,
    };
    let hint = Paragraph::new(Span::styled(
        "[j/k] move  [Space] toggle  [Enter/Esc] done",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hint, hint_area);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let w = r.width * percent_x / 100;
    let x = r.x + (r.width.saturating_sub(w)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}

fn render_clone(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let editing = app.projects.clone_input_mode == InputMode::Editing;
    let url_border = if editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if editing { "█" } else { "" };
    let url_text = format!("{}{}", app.projects.clone_url, cursor);
    let url_para = Paragraph::new(url_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(url_border)
            .title(" URL or user/repo shorthand (GitHub) "),
    );
    f.render_widget(url_para, chunks[0]);

    let running_label = if app.projects.clone_running { " (running…)" } else { "" };
    let total = app.projects.clone_output.len();
    let visible_h = chunks[1].height.saturating_sub(2) as usize;
    let scroll = app.projects.clone_output_scroll.min(total.saturating_sub(visible_h));
    let bottom = total.saturating_sub(scroll);
    let top = bottom.saturating_sub(visible_h);
    let out_lines: Vec<Line> = app.projects.clone_output[top..bottom]
        .iter()
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();
    let scroll_label = if scroll > 0 { format!(" ↑{} ", scroll) } else { String::new() };
    let out_para = Paragraph::new(out_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Output{}{} ", running_label, scroll_label)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(out_para, chunks[1]);

    let hint = if editing {
        "[Esc] cancel  [Enter] clone"
    } else if app.projects.clone_running {
        "[↑/↓] scroll output  cloning in progress…"
    } else {
        "[i] edit URL  [Enter] clone  [↑/↓] scroll  (user/repo → github.com)"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[2]);
}

fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // dir input
            Constraint::Length(4), // git status
            Constraint::Length(3), // git name
            Constraint::Length(3), // git email
            Constraint::Length(3), // github token
            Constraint::Min(0),    // editor info
            Constraint::Length(1), // hint
        ])
        .split(area);

    let editing = app.projects.settings_edit_mode == InputMode::Editing;
    let focus = app.projects.settings_focus;

    let field = |f: &mut Frame, area: Rect, idx: usize, title: &str, value: &str, mask: bool| {
        let focused = focus == idx;
        let active = focused && editing;
        let border = if focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };
        let shown = if mask && !value.is_empty() {
            "•".repeat(value.chars().count())
        } else {
            value.to_string()
        };
        let cursor = if active { "█" } else { "" };
        let marker = if focused { "› " } else { "  " };
        let para = Paragraph::new(format!("{}{}{}", marker, shown, cursor)).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border)
                .title(format!(" {} ", title)),
        );
        f.render_widget(para, area);
    };

    field(f, chunks[0], 0, "Projects directory", &app.projects.dir_input, false);

    let git = &app.projects.git;
    let status_line = if git.installed {
        let helper = if git.credential_helper.is_empty() {
            "none".to_string()
        } else {
            git.credential_helper.clone()
        };
        format!(
            "git {}\nidentity: {} <{}>\ncredential.helper: {}",
            if git.version.is_empty() { "(installed)" } else { &git.version },
            if git.name.is_empty() { "(unset)" } else { &git.name },
            if git.email.is_empty() { "(unset)" } else { &git.email },
            helper,
        )
    } else {
        "git not found — install git to clone/scaffold".to_string()
    };
    let status_para = Paragraph::new(Span::styled(
        status_line,
        Style::default().fg(if git.installed { Color::DarkGray } else { Color::Red }),
    ))
    .block(Block::default().borders(Borders::ALL).title(" Git status "));
    f.render_widget(status_para, chunks[1]);

    field(f, chunks[2], 1, "Git user.name", &app.projects.git_name_input, false);
    field(f, chunks[3], 2, "Git user.email", &app.projects.git_email_input, false);
    field(f, chunks[4], 3, "GitHub token (HTTPS)", &app.projects.git_token_input, true);

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "(not set)".to_string());
    let editor_para = Paragraph::new(Span::styled(
        format!("$EDITOR = {}", editor),
        Style::default().fg(Color::DarkGray),
    ))
    .block(Block::default().borders(Borders::ALL).title(" Editor "));
    f.render_widget(editor_para, chunks[5]);

    let hint = if editing {
        "[Esc] cancel  [Enter] save field"
    } else {
        "[j/k] field  [i] edit  [Enter] save/apply"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[6]);
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

fn format_age(ts: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let secs = now.saturating_sub(ts);
    if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}
