use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let has_physical = !app.storage.physical.is_empty();

    let (fs_height, disk_height) = if has_physical {
        let mid = area.height.saturating_sub(3) / 2;
        (Constraint::Min(mid.min(12)), Constraint::Min(6))
    } else {
        (Constraint::Min(10), Constraint::Length(0))
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            fs_height,
            disk_height,
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_filesystems(f, app, chunks[0]);
    if has_physical || app.storage.smart_loading {
        render_physical(f, app, chunks[1]);
    }
    render_hints(f, app, chunks[2]);

    if app.storage.input_mode == InputMode::Editing {
        render_mount_form(f, app, area);
    }
    if app.storage.show_fstab {
        render_fstab_popup(f, app, area);
    }
}

fn render_filesystems(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Filesystems ").borders(Borders::ALL);

    let devices = &app.storage.devices;

    if devices.is_empty() {
        let msg = if app.storage.loading {
            "Loading…"
        } else {
            "No mounted filesystems — press R to reload"
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
        return;
    }

    let header = Row::new(vec![
        "Device", "Mount", "Type", "Size", "Used", "Avail", "Use%",
    ])
    .style(
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
    .height(1);

    let rows: Vec<Row> = devices
        .iter()
        .map(|d| {
            let use_pct = d.total_bytes.checked_div(100).and_then(|_| d.used_bytes.checked_mul(100)).and_then(|n| n.checked_div(d.total_bytes)).unwrap_or(0);
            let color = if use_pct > 90 {
                Color::Red
            } else if use_pct > 75 {
                Color::Yellow
            } else {
                Color::Green
            };
            Row::new(vec![
                Cell::from(d.device.as_str()),
                Cell::from(d.mount.as_str()),
                Cell::from(d.fs_type.as_str()),
                Cell::from(fmt_bytes(d.total_bytes)),
                Cell::from(Span::styled(fmt_bytes(d.used_bytes), Style::default().fg(color))),
                Cell::from(fmt_bytes(d.avail_bytes)),
                Cell::from(Span::styled(format!("{}%", use_pct), Style::default().fg(color))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(14),
        Constraint::Min(12),
        Constraint::Length(7),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(6),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(block)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = app.storage.table_state.clone();
    f.render_stateful_widget(table, area, &mut state);
}

fn render_physical(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Physical Disks ").borders(Borders::ALL);

    if app.storage.physical.is_empty() && app.storage.smart_loading {
        f.render_widget(
            Paragraph::new(Span::styled(
                "Scanning disk health…",
                Style::default().fg(Color::DarkGray),
            ))
            .block(block),
            area,
        );
        return;
    }

    let lines: Vec<Line> = app
        .storage
        .physical
        .iter()
        .map(|d| {
            let health_span = if d.power_on_hours == 0 && d.temp_celsius == 0 {
                Span::styled("HEALTH: N/A", Style::default().fg(Color::DarkGray))
            } else if d.healthy {
                Span::styled("HEALTH: PASSED", Style::default().fg(Color::Green))
            } else {
                Span::styled("HEALTH: FAILED", Style::default().fg(Color::Red))
            };

            let mut parts = vec![
                Span::styled(format!("{}  ", d.device), Style::default().fg(Color::Cyan)),
                Span::styled(format!("{}   ", d.model), Style::default().fg(Color::White)),
                health_span,
            ];
            if d.temp_celsius > 0 {
                let temp_color = if d.temp_celsius > 55 {
                    Color::Red
                } else if d.temp_celsius > 45 {
                    Color::Yellow
                } else {
                    Color::Green
                };
                parts.push(Span::styled(
                    format!("  {}°C", d.temp_celsius),
                    Style::default().fg(temp_color),
                ));
            }
            if d.power_on_hours > 0 {
                parts.push(Span::styled(
                    format!("  {}h", d.power_on_hours),
                    Style::default().fg(Color::DarkGray),
                ));
            }
            Line::from(parts)
        })
        .collect();

    let p = Paragraph::new(lines).block(block);
    f.render_widget(p, area);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = if app.storage.show_fstab {
        "[q] close"
    } else if app.storage.input_mode == InputMode::Editing {
        "[Tab] switch field  [Enter] confirm  [Esc] cancel"
    } else {
        "[↑/↓] navigate  [m] mount  [u] unmount  [f] view fstab  [R] reload"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray))),
        area,
    );
}

fn render_mount_form(f: &mut Frame, app: &App, area: Rect) {
    let w = 55u16.min(area.width.saturating_sub(4));
    let h = 8u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" Mount Filesystem ")
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
            Constraint::Length(2),
            Constraint::Length(1),
        ])
        .split(inner);

    let dev_style = if app.storage.input_focus == 0 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let mnt_style = if app.storage.input_focus == 1 {
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Device:     ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.storage.input_device, dev_style),
            if app.storage.input_focus == 0 {
                Span::styled("▌", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ])),
        rows[0],
    );

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Mountpoint: ", Style::default().fg(Color::DarkGray)),
            Span::styled(&app.storage.input_mountpoint, mnt_style),
            if app.storage.input_focus == 1 {
                Span::styled("▌", Style::default().fg(Color::Yellow))
            } else {
                Span::raw("")
            },
        ])),
        rows[1],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "e.g. /dev/sdb1  /mnt/data",
            Style::default().fg(Color::DarkGray),
        )),
        rows[3],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            "[Tab] switch field  [Enter] confirm  [Esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
        rows[4],
    );
}

fn render_fstab_popup(f: &mut Frame, app: &App, area: Rect) {
    let w = area.width.saturating_sub(4).min(70);
    let h = area.height.saturating_sub(4).min(30);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);

    let block = Block::default()
        .title(" /etc/fstab ")
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(block, popup);

    let inner = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(popup);

    let content = &app.storage.fstab_content;
    let text = if content.is_empty() {
        vec![Line::from(Span::styled(
            "Loading…",
            Style::default().fg(Color::DarkGray),
        ))]
    } else {
        content
            .lines()
            .skip(app.storage.fstab_scroll as usize)
            .map(|l| Line::from(Span::raw(l)))
            .collect::<Vec<_>>()
    };

    f.render_widget(Paragraph::new(text), inner[0]);

    f.render_widget(
        Paragraph::new(Span::styled(
            "[↑/↓] scroll  [q] close",
            Style::default().fg(Color::DarkGray),
        )),
        inner[1],
    );
}

pub fn fmt_bytes(bytes: u64) -> String {
    if bytes == 0 {
        return "0 B".to_string();
    }
    const UNITS: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
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
