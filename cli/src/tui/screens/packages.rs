use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::core::packages::CURATED;
use crate::tui::app::{App, InputMode, OpStatus, PackageTab};
use crate::tui::events::filtered_installed;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    render_tabs(f, app, chunks[0]);

    match app.packages.active_tab {
        PackageTab::Installed => render_installed(f, app, chunks[1]),
        PackageTab::Search => render_search(f, app, chunks[1]),
        PackageTab::QuickInstall => render_quick_install(f, app, chunks[1]),
        PackageTab::Queue => render_queue(f, app, chunks[1]),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = PackageTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .select(app.packages.active_tab.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_installed(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Filter input
    let filter_style = if app.packages.filter_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let filter_text = format!("/{}", app.packages.filter);
    let filter_p = Paragraph::new(filter_text)
        .style(filter_style)
        .block(Block::default().title(" Filter ").borders(Borders::ALL));
    f.render_widget(filter_p, chunks[0]);

    // Package list — show loading hint while async fetch is in progress
    if app.packages.installed.is_empty() {
        let p = Paragraph::new(Span::styled(
            "Loading packages…  press [r] to retry",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Installed ").borders(Borders::ALL));
        f.render_widget(p, chunks[1]);
        return;
    }

    let visible = filtered_installed(app);
    let items: Vec<ListItem> = visible
        .iter()
        .map(|pkg| {
            let selected = app.packages.selected.contains(&pkg.name);
            let prefix = if selected { "[*] " } else { "[ ] " };
            let prefix_color = if selected { Color::Yellow } else { Color::DarkGray };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(prefix_color)),
                Span::styled(&pkg.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(&pkg.version, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(&pkg.description, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .title(format!(" Installed ({}) ", visible.len()))
            .borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.packages.installed_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    // Hints
    let hints = Paragraph::new(Span::styled(
        " [Space] toggle  [d] remove  [/] filter  [r] refresh  [←/→] tabs ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, chunks[2]);
}

fn render_search(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    let search_style = if app.packages.search_mode == InputMode::Editing {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let cursor = if app.packages.search_mode == InputMode::Editing { "█" } else { "" };
    let search_text = format!("{}{}", app.packages.search_query, cursor);
    let search_p = Paragraph::new(search_text)
        .style(search_style)
        .block(Block::default().title(" Search ").borders(Borders::ALL));
    f.render_widget(search_p, chunks[0]);

    let items: Vec<ListItem> = app.packages.search_results
        .iter()
        .map(|pkg| {
            let selected = app.packages.search_selected.contains(&pkg.name);
            let prefix = if selected { "[*] " } else { "[ ] " };
            let prefix_color = if selected { Color::Yellow } else { Color::DarkGray };
            ListItem::new(Line::from(vec![
                Span::styled(prefix, Style::default().fg(prefix_color)),
                Span::styled(&pkg.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(&pkg.description, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let count = items.len();
    let list = List::new(items)
        .block(Block::default()
            .title(format!(" Results ({}) ", count))
            .borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.packages.search_state.clone();
    f.render_stateful_widget(list, chunks[1], &mut state);

    let hints = Paragraph::new(Span::styled(
        " [/] search  [Space] toggle  [i] install selected  [←/→] tabs ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(hints, chunks[2]);
}

fn render_quick_install(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Build installed-name set for O(1) lookup
    let installed_names: std::collections::HashSet<&str> = app.packages.installed
        .iter()
        .map(|p| p.name.as_str())
        .collect();
    let installed_loaded = !app.packages.installed.is_empty();

    let mut items: Vec<ListItem> = Vec::new();
    let mut flat_pkgs: Vec<&'static str> = Vec::new();

    for cat in CURATED {
        items.push(ListItem::new(Line::from(Span::styled(
            cat.name,
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ))));
        flat_pkgs.push(""); // placeholder for category row (non-selectable)

        for &pkg in cat.packages {
            flat_pkgs.push(pkg);
            let is_installed = installed_loaded && installed_names.contains(pkg);
            let in_uninstall = app.packages.curated_uninstall.contains(pkg);
            let in_install   = app.packages.curated_selected.contains(pkg);

            let (prefix, prefix_color, name_color, badge) = if is_installed {
                if in_uninstall {
                    ("[✗] ", Color::Red,   Color::Red,   " ← remove")
                } else {
                    ("[✓] ", Color::Cyan,  Color::White, " installed")
                }
            } else if in_install {
                ("[*] ", Color::Green, Color::White, "")
            } else {
                ("[ ] ", Color::DarkGray, Color::White, "")
            };

            items.push(ListItem::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(prefix, Style::default().fg(prefix_color)),
                Span::styled(pkg, Style::default().fg(name_color).add_modifier(
                    if is_installed { Modifier::BOLD } else { Modifier::empty() }
                )),
                Span::styled(badge, Style::default().fg(prefix_color).add_modifier(Modifier::DIM)),
            ])));
        }
    }

    // Compute visual highlight index — skip category header rows
    let cursor = app.packages.curated_cursor;
    let mut highlight_idx = 0usize;
    let mut pkg_count = 0usize;
    for (i, &p) in flat_pkgs.iter().enumerate() {
        if p.is_empty() { continue; }
        if pkg_count == cursor { highlight_idx = i; break; }
        pkg_count += 1;
    }

    let title = if installed_loaded {
        " Quick Install "
    } else {
        " Quick Install (loading installed list…) "
    };

    let list = List::new(items)
        .block(Block::default().title(title).borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray));

    let mut state = ratatui::widgets::ListState::default();
    state.select(Some(highlight_idx));
    f.render_stateful_widget(list, chunks[0], &mut state);

    // Hint bar
    let hint = if installed_loaded {
        " [Space] toggle install/remove  [Enter] apply  [←/→] tabs  [✓]=installed  [✗]=mark remove"
    } else {
        " [Space] toggle  [Enter] install selected  [←/→] tabs "
    };
    let hints = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints, chunks[1]);
}

fn render_queue(f: &mut Frame, app: &App, area: Rect) {
    let queue_len = app.packages.queue.len();

    // Split area: top = queue list, bottom = output pane
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // ── Queue list ────────────────────────────────────────────────────────
    let items: Vec<ListItem> = app.packages.queue
        .iter()
        .enumerate()
        .map(|(i, op)| {
            let (status_color, status_sym) = match op.status {
                OpStatus::Pending => (Color::DarkGray, "○"),
                OpStatus::Running => (Color::Yellow, "◌"),
                OpStatus::Done => (Color::Green, "●"),
                OpStatus::Failed => (Color::Red, "✗"),
            };
            let selected = app.packages.queue_selected == Some(i);
            let style = if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            };
            ListItem::new(Line::from(vec![
                Span::styled(status_sym, Style::default().fg(status_color)),
                Span::raw(" "),
                Span::styled(&op.kind, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(&op.target, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::raw("  "),
                Span::styled(op.status.label(), Style::default().fg(status_color)),
            ])).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default()
            .title(format!(" Queue ({}) ", queue_len))
            .title_bottom(Span::styled(
                " [↑/↓] select  [PgUp/PgDn] scroll output  [r] reload  [←/→] tabs ",
                Style::default().fg(Color::DarkGray),
            ))
            .borders(Borders::ALL));
    f.render_widget(list, chunks[0]);

    // ── Output pane ───────────────────────────────────────────────────────
    let (title, output_text, title_color) = if let Some(sel) = app.packages.queue_selected {
        if let Some(op) = app.packages.queue.get(sel) {
            let color = match op.status {
                OpStatus::Failed => Color::Red,
                OpStatus::Done => Color::Green,
                OpStatus::Running => Color::Yellow,
                OpStatus::Pending => Color::DarkGray,
            };
            let text = if op.output.is_empty() {
                match op.status {
                    OpStatus::Running => "Waiting for output…".to_string(),
                    OpStatus::Pending => "Queued".to_string(),
                    _ => String::new(),
                }
            } else {
                op.output.clone()
            };
            let t = format!(" {} {} ", op.kind, op.target);
            (t, text, color)
        } else {
            (" Output ".to_string(), String::new(), Color::DarkGray)
        }
    } else {
        (" Output — select an item with ↑/↓ ".to_string(), String::new(), Color::DarkGray)
    };

    let pane_height = chunks[1].height.saturating_sub(2) as usize; // subtract borders
    let all_lines: Vec<&str> = output_text.lines().collect();
    let total_lines = all_lines.len();

    // For Running items: auto-tail (always show latest output at bottom)
    // For Done/Failed: use output_scroll from the top
    let display_lines: Vec<Line> = if matches!(
        app.packages.queue_selected
            .and_then(|i| app.packages.queue.get(i))
            .map(|op| &op.status),
        Some(OpStatus::Running)
    ) {
        // Tail mode: show last pane_height lines
        let skip = total_lines.saturating_sub(pane_height);
        all_lines[skip..].iter().map(|&line| {
            let color = if line.starts_with("E:") || line.starts_with("error") || line.starts_with("Error") {
                Color::Red
            } else if line.starts_with("W:") || line.starts_with("warning") {
                Color::Yellow
            } else {
                Color::White
            };
            Line::from(Span::styled(line, Style::default().fg(color)))
        }).collect()
    } else {
        // Scroll mode: start from output_scroll
        let skip = app.packages.output_scroll.min(total_lines.saturating_sub(1));
        all_lines[skip..].iter().take(pane_height).map(|&line| {
            let color = if line.starts_with("E:") || line.starts_with("error") || line.starts_with("Error") {
                Color::Red
            } else if line.starts_with("W:") || line.starts_with("warning") {
                Color::Yellow
            } else {
                Color::White
            };
            Line::from(Span::styled(line, Style::default().fg(color)))
        }).collect()
    };

    let scroll_hint = if total_lines > pane_height {
        format!(" {} lines  [PgUp/PgDn scroll] ", total_lines)
    } else {
        title.clone()
    };

    let pane = Paragraph::new(display_lines)
        .block(
            Block::default()
                .title(Span::styled(scroll_hint, Style::default().fg(title_color)))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(title_color)),
        );
    f.render_widget(pane, chunks[1]);
}
