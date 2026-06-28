use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Sparkline, Table},
    Frame,
};

use crate::core::models::SensorKind;
use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    if !cfg!(target_os = "linux") {
        f.render_widget(
            Paragraph::new(Span::styled(
                "Hardware diagnostics are available on Linux only.",
                Style::default().fg(Color::DarkGray),
            ))
            .block(Block::default().title(" Hardware ").borders(Borders::ALL)),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // sensors
            Constraint::Length(7), // load
            Constraint::Min(7),    // boot
            Constraint::Length(1), // hints
        ])
        .split(area);

    render_sensors(f, app, chunks[0]);
    render_load(f, app, chunks[1]);
    render_boot(f, app, chunks[2]);
    render_hints(f, app, chunks[3]);
}

fn render_sensors(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Sensors ").borders(Borders::ALL);
    let hw = &app.hardware;

    let Some(sensors) = &hw.sensors else {
        let msg = if hw.sensors_loading { "Loading…" } else { "No data — press R" };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
        return;
    };

    if sensors.readings.is_empty() {
        let msg = if sensors.sensors_tool {
            "No sensors exposed by the kernel."
        } else {
            "No sensors found — press [i] to install lm-sensors."
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
        return;
    }

    let header = Row::new(vec!["Chip", "Sensor", "Reading"])
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .height(1);

    let rows: Vec<Row> = sensors
        .readings
        .iter()
        .map(|r| {
            let (value, color) = match r.kind {
                SensorKind::Temp => {
                    let color = if r.value >= 80.0 {
                        Color::Red
                    } else if r.value >= 65.0 {
                        Color::Yellow
                    } else {
                        Color::Green
                    };
                    (format!("{:.1} {}", r.value, r.unit), color)
                }
                SensorKind::Fan => (format!("{:.0} {}", r.value, r.unit), Color::White),
            };
            Row::new(vec![
                Cell::from(r.chip.as_str()),
                Cell::from(r.label.as_str()),
                Cell::from(Span::styled(value, Style::default().fg(color))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(18),
        Constraint::Min(16),
        Constraint::Length(14),
    ];
    f.render_widget(Table::new(rows, widths).header(header).block(block), area);
}

fn render_load(f: &mut Frame, app: &App, area: Rect) {
    let hw = &app.hardware;
    let Some(load) = &hw.load else {
        f.render_widget(
            Paragraph::new(Span::styled("Loading…", Style::default().fg(Color::DarkGray)))
                .block(Block::default().title(" Load Average ").borders(Borders::ALL)),
            area,
        );
        return;
    };

    let title = format!(
        " Load Average — 1m {:.2}  5m {:.2}  15m {:.2}  ({} running / {} total) ",
        load.one, load.five, load.fifteen, load.running, load.total
    );
    let data: Vec<u64> = hw.load_history.iter().map(|v| (v * 100.0) as u64).collect();
    let max = data.iter().copied().max().unwrap_or(100).max(100);
    let color = if load.one >= 8.0 {
        Color::Red
    } else if load.one >= 2.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    let sparkline = Sparkline::default()
        .block(Block::default().title(title).borders(Borders::ALL))
        .data(&data)
        .style(Style::default().fg(color))
        .max(max);
    f.render_widget(sparkline, area);
}

fn render_boot(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Boot Time (systemd-analyze) ")
        .borders(Borders::ALL);
    let hw = &app.hardware;

    let Some(boot) = &hw.boot else {
        let msg = if hw.boot_loading {
            "Loading…"
        } else {
            "systemd-analyze unavailable on this host."
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(Color::DarkGray))).block(block),
            area,
        );
        return;
    };

    let mut lines = Vec::new();
    let mut breakdown = vec![Span::styled(
        format!("Total {:.2}s", boot.total_secs),
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];
    if boot.firmware_secs > 0.0 {
        breakdown.push(Span::raw(format!("   firmware {:.2}s", boot.firmware_secs)));
    }
    if boot.loader_secs > 0.0 {
        breakdown.push(Span::raw(format!("   loader {:.2}s", boot.loader_secs)));
    }
    breakdown.push(Span::raw(format!("   kernel {:.2}s", boot.kernel_secs)));
    breakdown.push(Span::raw(format!("   userspace {:.2}s", boot.userspace_secs)));
    lines.push(Line::from(breakdown));
    lines.push(Line::from(""));

    for u in &boot.units {
        lines.push(Line::from(vec![
            Span::styled(format!("{:>8.2}s  ", u.secs), Style::default().fg(Color::Yellow)),
            Span::raw(u.name.as_str()),
        ]));
    }

    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let hints = if app.hardware.installing {
        "installing lm-sensors…"
    } else {
        "[R] reload  [i] install lm-sensors"
    };
    f.render_widget(
        Paragraph::new(Span::styled(hints, Style::default().fg(Color::DarkGray))),
        area,
    );
}
