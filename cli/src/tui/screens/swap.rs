use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Gauge, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // summary / gauges
            Constraint::Min(0),    // entries table
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_summary(f, app, chunks[0]);
    render_entries(f, app, chunks[1]);
    render_hints(f, app, chunks[2]);

    if app.swap.input_mode == InputMode::Editing {
        render_form(f, app, area);
    }
}

fn render_summary(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Swap Overview ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.swap.loading && app.swap.status.is_none() {
        let p = Paragraph::new(Span::styled("Loading…", Style::default().fg(Color::DarkGray)));
        f.render_widget(p, inner);
        return;
    }

    let Some(status) = &app.swap.status else {
        let p = Paragraph::new(Span::styled(
            "No swap data — press R to reload",
            Style::default().fg(Color::DarkGray),
        ));
        f.render_widget(p, inner);
        return;
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // gauge
            Constraint::Percentage(60), // numbers
        ])
        .split(inner);

    let pct = (status.used * 100).checked_div(status.total).unwrap_or(0) as u16;
    let gauge_color = if pct > 85 {
        Color::Red
    } else if pct > 65 {
        Color::Yellow
    } else {
        Color::Green
    };
    let gauge = Gauge::default()
        .block(Block::default())
        .gauge_style(Style::default().fg(gauge_color))
        .percent(pct)
        .label(format!("{}%", pct));
    f.render_widget(gauge, cols[0]);

    let text = vec![
        Line::from(vec![
            Span::styled("Total:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_bytes(status.total), Style::default().fg(Color::White)),
        ]),
        Line::from(vec![
            Span::styled("Used:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_bytes(status.used), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("Free:   ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_bytes(status.free), Style::default().fg(Color::Green)),
        ]),
    ];
    let p = Paragraph::new(text);
    f.render_widget(p, cols[1]);
}

fn render_entries(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Swap Entries ").borders(Borders::ALL);

    let entries = app
        .swap
        .status
        .as_ref()
        .map(|s| s.entries.as_slice())
        .unwrap_or(&[]);

    if entries.is_empty() {
        let msg = if app.swap.loading {
            "Loading…"
        } else {
            "No active swap — press n to create one"
        };
        let p = Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray)))
            .block(block);
        f.render_widget(p, area);
        return;
    }

    let header = Row::new(vec!["Path", "Type", "Size", "Used", "Priority"])
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = entries
        .iter()
        .map(|e| {
            let used_pct = (e.used_bytes * 100).checked_div(e.size_bytes).unwrap_or(0);
            let used_color = if used_pct > 85 {
                Color::Red
            } else if used_pct > 65 {
                Color::Yellow
            } else {
                Color::Green
            };
            Row::new(vec![
                Cell::from(e.path.as_str()),
                Cell::from(e.kind.as_str()),
                Cell::from(fmt_bytes(e.size_bytes)),
                Cell::from(Span::styled(
                    fmt_bytes(e.used_bytes),
                    Style::default().fg(used_color),
                )),
                Cell::from(e.priority.to_string()),
            ])
        })
        .collect();

    let widths = [
        Constraint::Min(20),
        Constraint::Length(12),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(9),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = app.swap.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = if app.swap.input_mode == InputMode::Editing {
        "[Tab] switch field  [Enter] confirm  [Esc] cancel"
    } else {
        "[n] new  [d] delete  [e] enable  [x] disable  [r] resize  [R] reload  [↑/↓] navigate"
    };
    let p = Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray)));
    f.render_widget(p, area);
}

fn render_form(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.swap.resize_mode {
        " Resize Swap "
    } else {
        " New Swap File "
    };

    let w = 50u16.min(area.width.saturating_sub(4));
    let h = 7u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Yellow));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let path_style = if app.swap.input_focus == 0 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let size_style = if app.swap.input_focus == 1 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Path:  ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.swap.input_path, path_style),
            if app.swap.input_focus == 0 {
                Span::styled("▌", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ])),
        rows[0],
    );

    if !app.swap.resize_mode {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Size:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(&app.swap.input_size, size_style),
                Span::styled(" MiB", Style::default().fg(Color::DarkGray)),
                if app.swap.input_focus == 1 {
                    Span::styled("▌", Style::default().fg(Color::Yellow))
                } else {
                    Span::raw("")
                },
            ])),
            rows[1],
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("New size: ", Style::default().fg(Color::DarkGray)),
                Span::styled(&app.swap.input_size, size_style),
                Span::styled(" MiB", Style::default().fg(Color::DarkGray)),
                if app.swap.input_focus == 1 {
                    Span::styled("▌", Style::default().fg(Color::Yellow))
                } else {
                    Span::raw("")
                },
            ])),
            rows[1],
        );
    }

    f.render_widget(
        Paragraph::new(Span::styled(
            "[Tab] switch  [Enter] confirm  [Esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
        rows[4],
    );
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB"];
    let mut val = bytes as f64;
    let mut unit = 0;
    while val >= 1024.0 && unit + 1 < UNITS.len() {
        val /= 1024.0;
        unit += 1;
    }
    if val < 10.0 {
        format!("{:.1} {}", val, UNITS[unit])
    } else {
        format!("{:.0} {}", val, UNITS[unit])
    }
}
