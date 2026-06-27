use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::core::models::{ManagedWorkloadBackend, ManagedWorkloadState};
use crate::tui::app::{App, DockerTab, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // tab bar
            Constraint::Min(0),    // content
            Constraint::Length(3), // header / status
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_tabs(f, app, chunks[0]);

    match app.docker.active_tab {
        DockerTab::Containers => render_containers(f, app, chunks[1]),
        DockerTab::Images => render_images(f, app, chunks[1]),
        DockerTab::Compose => render_compose(f, app, chunks[1]),
        DockerTab::Workloads => render_workloads(f, app, chunks[1]),
        DockerTab::Managed => render_managed(f, app, chunks[1]),
    }

    render_header(f, app, chunks[2]);
    render_hints(f, app, chunks[3]);
    if app.docker.active_tab == DockerTab::Workloads
        && app.docker.workloads.form.input_mode == InputMode::Editing
    {
        render_workload_popup(f, app, area);
    }
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let (dot_color, dot) = if app.docker.installed {
        (Color::Green, "●")
    } else {
        (Color::Red, "○")
    };
    let version = app.docker.version.as_deref().unwrap_or("not installed");
    let loading = if app.docker.loading {
        "  Loading…"
    } else {
        ""
    };
    let text = Line::from(vec![
        Span::styled(dot, Style::default().fg(dot_color)),
        Span::raw(" Docker  "),
        Span::styled(version, Style::default().fg(Color::DarkGray)),
        Span::styled(loading, Style::default().fg(Color::Yellow)),
    ]);
    let p = Paragraph::new(text).block(
        Block::default()
            .title(" Docker Manager ")
            .borders(Borders::ALL),
    );
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

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Name"),
        Cell::from("Image"),
        Cell::from("Status"),
        Cell::from("Ports"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app
        .docker
        .containers
        .iter()
        .map(|c| {
            let status_color = if c.status.contains("Up") || c.status.contains("running") {
                Color::Green
            } else if c.status.contains("Paused") || c.status.contains("paused") {
                Color::Yellow
            } else {
                Color::Red
            };
            Row::new([
                Cell::from(c.name.as_str()).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(c.image.as_str()).style(Style::default().fg(Color::Cyan)),
                Cell::from(c.status.as_str()).style(Style::default().fg(status_color)),
                Cell::from(c.ports.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let count = rows.len();
    let widths = [
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Length(20),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(headers)
        .block(
            Block::default()
                .title(format!(" Containers ({}) ", count))
                .borders(Borders::ALL),
        )
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

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Repository"),
        Cell::from("Tag"),
        Cell::from("ID"),
        Cell::from("Size"),
        Cell::from("Created"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app
        .docker
        .images
        .iter()
        .map(|img| {
            let repo_color = if img.repository == "<none>" {
                Color::DarkGray
            } else {
                Color::White
            };
            Row::new([
                Cell::from(img.repository.as_str())
                    .style(Style::default().fg(repo_color).add_modifier(Modifier::BOLD)),
                Cell::from(img.tag.as_str()).style(Style::default().fg(Color::Cyan)),
                Cell::from(img.id.as_str()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(img.size.as_str()).style(Style::default().fg(Color::Yellow)),
                Cell::from(img.created.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

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
        .block(
            Block::default()
                .title(format!(" Images ({}) ", count))
                .borders(Borders::ALL),
        )
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
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Compose File "),
    );
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

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Service"),
        Cell::from("Status"),
        Cell::from("Image"),
        Cell::from("Ports"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app
        .docker
        .compose_services
        .iter()
        .map(|svc| {
            let status_color = if svc.status.to_lowercase().contains("running") {
                Color::Green
            } else if svc.status.is_empty() || svc.status.to_lowercase().contains("exit") {
                Color::Red
            } else {
                Color::Yellow
            };
            Row::new([
                Cell::from(svc.name.as_str()).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(svc.status.as_str()).style(Style::default().fg(status_color)),
                Cell::from(svc.image.as_str()).style(Style::default().fg(Color::Cyan)),
                Cell::from(svc.ports.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let count = rows.len();
    let widths = [
        Constraint::Fill(1),
        Constraint::Length(24),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ];
    let table = Table::new(rows, widths)
        .header(headers)
        .block(
            Block::default()
                .title(format!(" Services ({}) ", count))
                .borders(Borders::ALL),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.docker.compose_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_workloads(f: &mut Frame, app: &App, area: Rect) {
    let capabilities = app.docker.workloads.capabilities.as_ref();
    if let Some(capabilities) = capabilities {
        if !capabilities.supported {
            let reason = capabilities
                .reason
                .as_deref()
                .unwrap_or("Workloads are unavailable on this host.");
            let p = Paragraph::new(reason)
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().title(" Workloads ").borders(Borders::ALL));
            f.render_widget(p, area);
            return;
        }
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(7), Constraint::Length(5)])
        .split(area);

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Name"),
        Cell::from("Backend"),
        Cell::from("State"),
        Cell::from("Image"),
        Cell::from("Ports"),
        Cell::from("Unit"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app
        .docker
        .workloads
        .workloads
        .iter()
        .map(|workload| {
            let status_color = match workload.status {
                ManagedWorkloadState::Running => Color::Green,
                ManagedWorkloadState::Stopped => Color::Yellow,
                ManagedWorkloadState::Failed => Color::Red,
                ManagedWorkloadState::NotInstalled => Color::DarkGray,
                ManagedWorkloadState::Unknown => Color::Cyan,
            };
            let backend = match workload.backend {
                ManagedWorkloadBackend::PodmanQuadlet => "Podman Quadlet",
                ManagedWorkloadBackend::DockerComposeSystemd => "Docker Compose + systemd",
            };
            Row::new([
                Cell::from(workload.name.as_str()).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(backend).style(Style::default().fg(Color::Cyan)),
                Cell::from(workload.status.label()).style(Style::default().fg(status_color)),
                Cell::from(workload.image.as_str()).style(Style::default().fg(Color::White)),
                Cell::from(workload.ports_summary.as_str())
                    .style(Style::default().fg(Color::DarkGray)),
                Cell::from(workload.unit_name.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(18),
            Constraint::Length(24),
            Constraint::Length(14),
            Constraint::Fill(1),
            Constraint::Length(18),
            Constraint::Length(24),
        ],
    )
    .header(headers)
    .block(Block::default().title(" Workloads ").borders(Borders::ALL))
    .row_highlight_style(Style::default().bg(Color::DarkGray))
    .highlight_symbol("› ");

    let mut state = app.docker.workloads.table_state.clone();
    f.render_stateful_widget(table, chunks[0], &mut state);

    let details = if let Some(idx) = app.docker.workloads.table_state.selected() {
        if let Some(workload) = app.docker.workloads.workloads.get(idx) {
            let compose = workload
                .compose_path
                .as_deref()
                .map(|path| format!("Compose: {}", path))
                .unwrap_or_else(|| "Compose: -".to_string());
            format!(
                "Backend: {}  Engine: {}\nSpec: {}\n{}",
                workload.backend.label(),
                workload.engine,
                workload.spec_path,
                compose,
            )
        } else {
            "No workload selected.".to_string()
        }
    } else {
        "No workload selected.".to_string()
    };

    let detail = Paragraph::new(details)
        .style(Style::default().fg(Color::DarkGray))
        .block(Block::default().title(" Details ").borders(Borders::ALL));
    f.render_widget(detail, chunks[1]);
}

fn render_workload_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 16, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Min(0),
        ])
        .margin(1)
        .split(popup_area);

    let form = &app.docker.workloads.form;
    let title = if form.editing_name.is_some() {
        " Edit Workload "
    } else {
        " Create Workload "
    };
    f.render_widget(
        Block::default().title(title).borders(Borders::ALL),
        popup_area,
    );

    let field_style = |field: usize| {
        if form.input_focus == field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        }
    };
    let render_field = |label: &str, value: &str, focus: usize| {
        Paragraph::new(format!("{}: {}", label, value)).style(field_style(focus))
    };

    f.render_widget(render_field("Name", &form.name, 0), chunks[0]);
    f.render_widget(render_field("Image", &form.image, 1), chunks[1]);
    f.render_widget(render_field("Command (csv)", &form.command, 2), chunks[2]);
    f.render_widget(render_field("Env (KEY=VAL,csv)", &form.env, 3), chunks[3]);
    f.render_widget(render_field("Ports (csv)", &form.ports, 4), chunks[4]);
    f.render_widget(render_field("Volumes (csv)", &form.volumes, 5), chunks[5]);
    f.render_widget(render_field("Restart", &form.restart_policy, 6), chunks[6]);
    f.render_widget(
        Paragraph::new("Tab to move, Enter to submit, Esc to cancel")
            .style(Style::default().fg(Color::DarkGray)),
        chunks[7],
    );
}

// ── Hints ────────────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hint = match app.docker.active_tab {
        DockerTab::Containers => {
            " [←/→] tabs  [↑/↓] select  [s] start  [x] stop  [R] restart  [D] remove  [r] refresh "
        }
        DockerTab::Images => " [←/→] tabs  [↑/↓] select  [D] remove image  [r] refresh ",
        DockerTab::Compose => {
            " [←/→] tabs  [↑/↓] select  [u] up  [d] down  [R] restart  [r] refresh "
        }
        DockerTab::Workloads => {
            " [←/→] tabs  [↑/↓] select  [a] add  [e] edit  [s] start  [x] stop  [R] restart  [n] enable  [d] disable  [D] delete  [r] refresh "
        }
        DockerTab::Managed => {
            " [←/→] tabs  [↑/↓] select  [s] start  [x] stop  [R] restart  [r] refresh "
        }
    };
    let p = Paragraph::new(Span::styled(hint, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

// ── Managed dev services tab ──────────────────────────────────────────────

fn render_managed(f: &mut Frame, app: &App, area: Rect) {
    if !app.docker.installed {
        let p = Paragraph::new(Span::styled(
            "Docker is not installed or not running.",
            Style::default().fg(Color::DarkGray),
        ))
        .block(
            Block::default()
                .title(" Managed Services ")
                .borders(Borders::ALL),
        );
        f.render_widget(p, area);
        return;
    }

    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let headers = Row::new([
        Cell::from("Service"),
        Cell::from("Image"),
        Cell::from("Ports"),
        Cell::from("Status"),
        Cell::from("Description"),
    ])
    .style(header_style);

    let rows: Vec<Row> = app
        .docker
        .managed_services
        .iter()
        .map(|svc| {
            let status_lower = svc.status.to_lowercase();
            let status_color = if status_lower.starts_with("up") || status_lower.contains("running")
            {
                Color::Green
            } else if status_lower == "not found" {
                Color::DarkGray
            } else if status_lower.contains("exit") || status_lower.contains("dead") {
                Color::Red
            } else {
                Color::Yellow
            };

            let status_icon = if status_lower.starts_with("up") || status_lower.contains("running")
            {
                "● "
            } else if status_lower == "not found" {
                "○ "
            } else {
                "◌ "
            };

            Row::new([
                Cell::from(svc.name.as_str()).style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Cell::from(svc.image.as_str()).style(Style::default().fg(Color::Cyan)),
                Cell::from(svc.ports.as_str()).style(Style::default().fg(Color::DarkGray)),
                Cell::from(format!("{}{}", status_icon, svc.status))
                    .style(Style::default().fg(status_color)),
                Cell::from(svc.description.as_str()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Fill(1),
        Constraint::Length(22),
        Constraint::Length(20),
        Constraint::Fill(2),
    ];

    let table = Table::new(rows, widths)
        .header(headers)
        .block(
            Block::default()
                .title(" Managed Dev Services — start/stop common local infrastructure ")
                .borders(Borders::ALL),
        )
        .row_highlight_style(Style::default().bg(Color::DarkGray))
        .highlight_symbol("› ");

    let mut state = app.docker.managed_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let width = r
        .width
        .saturating_mul(percent_x)
        .saturating_div(100)
        .max(20);
    let popup_h = height.min(r.height.saturating_sub(2)).max(3);
    Rect {
        x: r.x + (r.width.saturating_sub(width)) / 2,
        y: r.y + (r.height.saturating_sub(popup_h)) / 2,
        width,
        height: popup_h,
    }
}
