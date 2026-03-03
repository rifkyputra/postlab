use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::{App, DockerTab, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // header / status
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_header(f, app, chunks[0]);
    render_tabs(f, app, chunks[1]);

    match app.docker.active_tab {
        DockerTab::Containers => render_containers(f, app, chunks[2]),
        DockerTab::Images     => render_images(f, app, chunks[2]),
        DockerTab::Compose    => render_compose(f, app, chunks[2]),
    }

    render_hints(f, app, chunks[3]);

    // Compose path input popup
    if app.docker.active_tab == DockerTab::Compose
        && app.docker.active_tab == DockerTab::Compose
    {
        // (future: could render a path-edit popup here)
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.docker.installed {
        (Color::Green, "●")
    } else {
        (Color::Red, "○")
    };
    let version = app.docker.version.as_deref().unwrap_or("not installed");
    let loading = if app.docker.loading { "  Loading…" } else { "" };
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" Docker  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::styled(loading, Style::default().fg(Color::Yellow)),
    ]);
    let p = Paragraph::new(text)
        .block(Block::default().title(" Docker Manager ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = DockerTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL))
        .select(app.docker.active_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, area);
}

// ── Containers tab ────────────────────────────────────────────────────────

fn render_containers(f: &mut Frame, app: &App, area: Rect) {
    if !app.docker.installed {
        let p = Paragraph::new(Span::styled(
            "Docker is not installed or not running.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Containers ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Name"),
        Cell::from("Image"),
        Cell::from("Status"),
        Cell::from("Ports"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.docker.containers.iter().map(|c| {
        let status_color = if c.status.contains("Up") || c.status.contains("running") {
            Color::Green
        } else if c.status.contains("Paused") || c.status.contains("paused") {
            Color::Yellow
        } else {
            Color::Red
        };
        Row::new([
            Cell::from(c.name.as_str()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Cell::from(c.image.as_str()).style(Style::default().fg(Color::Cyan)),
            Cell::from(c.status.as_str()).style(Style::default().fg(status_color)),
            Cell::from(c.ports.as_str()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let count = rows.len();
    let widths = [
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Length(20),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Containers ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.docker.containers_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

// ── Images tab ────────────────────────────────────────────────────────────

fn render_images(f: &mut Frame, app: &App, area: Rect) {
    if !app.docker.installed {
        let p = Paragraph::new(Span::styled(
            "Docker is not installed or not running.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Images ").borders(Borders::ALL));
        f.render_widget(p, area);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Repository"),
        Cell::from("Tag"),
        Cell::from("ID"),
        Cell::from("Size"),
        Cell::from("Created"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.docker.images.iter().map(|img| {
        let repo_color = if img.repository == "<none>" { Color::DarkGray } else { Color::White };
        Row::new([
            Cell::from(img.repository.as_str()).style(Style::default().fg(repo_color).add_modifier(Modifier::BOLD)),
            Cell::from(img.tag.as_str()).style(Style::default().fg(Color::Cyan)),
            Cell::from(img.id.as_str()).style(Style::default().fg(Color::DarkGray)),
            Cell::from(img.size.as_str()).style(Style::default().fg(Color::Yellow)),
            Cell::from(img.created.as_str()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let count = rows.len();
    let widths = [
        Constraint::Fill(2),
        Constraint::Length(20),
        Constraint::Length(14),
        Constraint::Length(12),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Images ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.docker.images_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

// ── Compose tab ───────────────────────────────────────────────────────────

fn render_compose(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Compose file path bar
    let path_bar = Paragraph::new(Line::from(vec![
        Span::styled("File: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&app.docker.compose_path, Style::default().fg(Color::Yellow)),
    ]))
    .block(Block::default().borders(Borders::ALL).title(" Compose File "));
    f.render_widget(path_bar, chunks[0]);

    if !app.docker.installed {
        let p = Paragraph::new(Span::styled(
            "Docker is not installed or not running.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(Block::default().title(" Services ").borders(Borders::ALL));
        f.render_widget(p, chunks[1]);
        return;
    }

    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Service"),
        Cell::from("Status"),
        Cell::from("Image"),
        Cell::from("Ports"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app.docker.compose_services.iter().map(|svc| {
        let status_color = if svc.status.to_lowercase().contains("running") {
            Color::Green
        } else if svc.status.is_empty() || svc.status.to_lowercase().contains("exit") {
            Color::Red
        } else {
            Color::Yellow
        };
        Row::new([
            Cell::from(svc.name.as_str()).style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Cell::from(svc.status.as_str()).style(Style::default().fg(status_color)),
            Cell::from(svc.image.as_str()).style(Style::default().fg(Color::Cyan)),
            Cell::from(svc.ports.as_str()).style(Style::default().fg(Color::DarkGray)),
        ])
    }).collect();

    let count = rows.len();
    let widths = [
        Constraint::Fill(1),
        Constraint::Length(24),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(headers)
        .block(Block::default().title(format!(" Services ({}) ", count)).borders(Borders::ALL))
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.docker.compose_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

// ── Hints ────────────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hint = match app.docker.active_tab {
        DockerTab::Containers => {
            " [←/→] tabs  [↑/↓] select  [s] start  [x] stop  [R] restart  [D] remove  [r] refresh "
        }
        DockerTab::Images => {
            " [←/→] tabs  [↑/↓] select  [D] remove image  [r] refresh "
        }
        DockerTab::Compose => {
            " [←/→] tabs  [↑/↓] select  [u] up  [d] down  [R] restart  [r] refresh "
        }
    };
    let p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

pub fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect { x, y, width: w, height }
}
