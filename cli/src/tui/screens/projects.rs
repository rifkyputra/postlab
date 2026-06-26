use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs, Wrap},
    Frame,
};

use crate::tui::app::{App, InputMode, ProjectsTab};

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
            let line = Line::from(vec![
                Span::styled(
                    format!("{:<30}", truncate(&p.name, 28)),
                    Style::default().fg(Color::Cyan),
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
        "[↑/↓] navigate  [Enter] show path  [r] refresh"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[1]);
}

fn render_new(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // name input
            Constraint::Min(0),    // output
            Constraint::Length(1), // hint
        ])
        .split(area);

    let editing = app.projects.new_input_mode == InputMode::Editing;
    let name_border = if editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if editing { "█" } else { "" };
    let name_text = format!("{}{}", app.projects.new_name, cursor);
    let name_para = Paragraph::new(name_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(name_border)
            .title(" Project name (better-t-stack) "),
    );
    f.render_widget(name_para, chunks[0]);

    let running_label = if app.projects.new_running { " (running…)" } else { "" };
    let out_lines: Vec<Line> = app
        .projects
        .new_output
        .iter()
        .rev()
        .take(chunks[1].height.saturating_sub(2) as usize)
        .rev()
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();
    let out_para = Paragraph::new(out_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Output{} ", running_label)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(out_para, chunks[1]);

    let hint = if editing {
        "[Esc] cancel  [Enter] run scaffold"
    } else if app.projects.new_running {
        "scaffolding in progress…"
    } else {
        "[i] edit name  [Enter] run scaffold  (requires Node.js / npx)"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[1 + 1]);
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
    let out_lines: Vec<Line> = app
        .projects
        .clone_output
        .iter()
        .rev()
        .take(chunks[1].height.saturating_sub(2) as usize)
        .rev()
        .map(|l| Line::from(Span::raw(l.as_str())))
        .collect();
    let out_para = Paragraph::new(out_lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" Output{} ", running_label)),
        )
        .wrap(Wrap { trim: false });
    f.render_widget(out_para, chunks[1]);

    let hint = if editing {
        "[Esc] cancel  [Enter] clone"
    } else if app.projects.clone_running {
        "cloning in progress…"
    } else {
        "[i] edit URL  [Enter] clone  (shorthand: user/repo → github.com)"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[2]);
}

fn render_settings(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // dir input
            Constraint::Length(3), // editor info
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let editing = app.projects.dir_input_mode == InputMode::Editing;
    let dir_border = if editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let cursor = if editing { "█" } else { "" };
    let dir_text = format!("{}{}", app.projects.dir_input, cursor);
    let dir_para = Paragraph::new(dir_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(dir_border)
            .title(" Projects directory "),
    );
    f.render_widget(dir_para, chunks[0]);

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "(not set)".to_string());
    let editor_para = Paragraph::new(Span::styled(
        format!("$EDITOR = {}", editor),
        Style::default().fg(Color::DarkGray),
    ))
    .block(Block::default().borders(Borders::ALL).title(" Editor "));
    f.render_widget(editor_para, chunks[1]);

    let hint = if editing {
        "[Esc] cancel  [Enter] save"
    } else {
        "[i] edit directory  [Enter] save"
    };
    let hint_p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hint_p, chunks[3]);
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
