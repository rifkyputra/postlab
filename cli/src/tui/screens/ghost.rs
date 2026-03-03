use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

use crate::core::models::GhostReason;
use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // table
            Constraint::Length(3), // detail / cmdline
            Constraint::Length(1), // hint bar
        ])
        .split(area);

    render_table(f, app, chunks[0]);
    render_detail(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);
}

// ── Table ─────────────────────────────────────────────────────────────────

fn render_table(f: &mut Frame, app: &App, area: Rect) {
    let header_style = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    let selected_style = Style::default().bg(Color::DarkGray);

    let header = Row::new([
        Cell::from("Reason"),
        Cell::from("PID"),
        Cell::from("PPID"),
        Cell::from("Name"),
        Cell::from("CPU%"),
        Cell::from("Memory"),
        Cell::from("User"),
        Cell::from("Cgroup"),
    ])
    .style(header_style);

    let rows: Vec<Row> = if app.ghost.scanning {
        vec![Row::new([Cell::from(Span::styled(
            "  Scanning…",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::ITALIC),
        ))])]
    } else if app.ghost.ghosts.is_empty() {
        vec![Row::new([Cell::from(Span::styled(
            "  No ghost processes found — press [r] to rescan",
            Style::default().fg(Color::DarkGray),
        ))])]
    } else {
        app.ghost
            .ghosts
            .iter()
            .map(|g| {
                let reason_cell = Cell::from(Span::styled(
                    g.reason.label(),
                    Style::default()
                        .fg(g.reason.color())
                        .add_modifier(Modifier::BOLD),
                ));

                let mem_color = if g.mem_bytes >= 500 * 1024 * 1024 {
                    Color::Red
                } else if g.mem_bytes >= 200 * 1024 * 1024 {
                    Color::Yellow
                } else {
                    Color::White
                };

                Row::new([
                    reason_cell,
                    Cell::from(g.pid.to_string()),
                    Cell::from(g.ppid.to_string()).style(Style::default().fg(Color::DarkGray)),
                    Cell::from(g.name.as_str()).style(Style::default().add_modifier(Modifier::BOLD)),
                    Cell::from(format!("{:.1}%", g.cpu_pct))
                        .style(Style::default().fg(if g.cpu_pct > 20.0 { Color::Yellow } else { Color::White })),
                    Cell::from(format_bytes(g.mem_bytes))
                        .style(Style::default().fg(mem_color)),
                    Cell::from(g.user.as_str()).style(Style::default().fg(Color::DarkGray)),
                    Cell::from(truncate_cgroup(&g.cgroup)).style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect()
    };

    let count = app.ghost.ghosts.len();
    let title = if app.ghost.scanning {
        " Ghost Services Hunter — scanning… ".to_string()
    } else {
        format!(" Ghost Services Hunter ({} found) ", count)
    };

    let widths = [
        Constraint::Length(10), // reason
        Constraint::Length(7),  // pid
        Constraint::Length(7),  // ppid
        Constraint::Fill(1),    // name
        Constraint::Length(7),  // cpu
        Constraint::Length(10), // mem
        Constraint::Length(12), // user
        Constraint::Length(28), // cgroup
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().title(title).borders(Borders::ALL))
        .row_highlight_style(selected_style)
        .highlight_symbol("› ");

    let mut state = app.ghost.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

// ── Detail / cmdline strip ─────────────────────────────────────────────────

fn render_detail(f: &mut Frame, app: &App, area: Rect) {
    let content = if let Some(idx) = app.ghost.table_state.selected() {
        if let Some(g) = app.ghost.ghosts.get(idx) {
            let reason_desc = match g.reason {
                GhostReason::Zombie  => "defunct (zombie) — should have been reaped by its parent",
                GhostReason::Orphan  => "reparented to PID 1 — parent process died",
                GhostReason::MemLeak => "high memory, not tracked by any systemd service",
            };
            Line::from(vec![
                Span::styled("  cmd: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    if g.cmdline.is_empty() { "<no cmdline>" } else { &g.cmdline },
                    Style::default().fg(Color::White),
                ),
                Span::styled("  — ", Style::default().fg(Color::DarkGray)),
                Span::styled(reason_desc, Style::default().fg(g.reason.color())),
            ])
        } else {
            Line::from("")
        }
    } else {
        Line::from(Span::styled(
            "  Select a process with ↑/↓ to see details",
            Style::default().fg(Color::DarkGray),
        ))
    };

    let para = Paragraph::new(content)
        .block(Block::default().borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM));
    f.render_widget(para, area);
}

// ── Hint bar ──────────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let scanning = app.ghost.scanning;
    let text = if scanning {
        " scanning — please wait…"
    } else {
        " [r] rescan  [k] kill selected  [↑/↓] navigate  [q] quit"
    };
    let para = Paragraph::new(Span::styled(
        text,
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(para, area);
}

// ── Formatting helpers ────────────────────────────────────────────────────

fn format_bytes(bytes: u64) -> String {
    const MB: u64 = 1_048_576;
    const GB: u64 = 1_073_741_824;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0} MB", bytes as f64 / MB as f64)
    } else {
        format!("{} KB", bytes / 1024)
    }
}

/// Show only the last component of a long cgroup path to save horizontal space.
fn truncate_cgroup(cgroup: &str) -> &str {
    if cgroup.is_empty() {
        return "-";
    }
    // cgroup v2 lines look like "0::/system.slice/foo.service"
    // Strip the "0::/" prefix, then show the last path segment.
    let stripped = cgroup.trim_start_matches("0::/");
    stripped.rsplit('/').next().unwrap_or(stripped)
}
