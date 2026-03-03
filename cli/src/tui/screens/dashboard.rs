use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, LineGauge, Paragraph, Tabs},
    Frame,
};

use crate::tui::app::{App, DashboardTab};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // tab bar
            Constraint::Min(0),    // content
        ])
        .split(area);

    render_tabs(f, app, chunks[0]);

    match app.dashboard.active_tab {
        DashboardTab::Overview => render_overview(f, app, chunks[1]),
        DashboardTab::Processes => super::processes::render(f, app, chunks[1]),
        DashboardTab::Resources => super::resources::render(f, app, chunks[1]),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = DashboardTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(app.dashboard.active_tab.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
        .divider(Span::raw(" │ "));
    f.render_widget(tabs, area);
}

fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),  // system info
            Constraint::Length(3),  // memory gauge
            Constraint::Min(4),     // cpu + disk
        ])
        .split(area);

    render_sysinfo(f, app, chunks[0]);
    render_memory(f, app, chunks[1]);

    let bottom = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[2]);

    render_cpu(f, app, bottom[0]);
    render_disks(f, app, bottom[1]);
}

fn render_sysinfo(f: &mut Frame, app: &App, area: Rect) {
    let text = if let Some(info) = &app.dashboard.os_info {
        let uptime = format_uptime(info.uptime_secs);
        vec![
            Line::from(vec![
                Span::styled("Hostname:  ", Style::default().fg(Color::DarkGray)),
                Span::styled(&info.hostname, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("OS:        ", Style::default().fg(Color::DarkGray)),
                Span::raw(&info.distro),
            ]),
            Line::from(vec![
                Span::styled("Kernel:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(&info.kernel_version),
            ]),
            Line::from(vec![
                Span::styled("Arch:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(&info.arch),
            ]),
            Line::from(vec![
                Span::styled("CPUs:      ", Style::default().fg(Color::DarkGray)),
                Span::raw(format!("{} cores", info.cpu_count)),
            ]),
            Line::from(vec![
                Span::styled("Uptime:    ", Style::default().fg(Color::DarkGray)),
                Span::raw(uptime),
            ]),
        ]
    } else {
        vec![Line::from(Span::styled("Loading…", Style::default().fg(Color::DarkGray)))]
    };

    let block = Block::default().title(" System ").borders(Borders::ALL);
    let p = Paragraph::new(text).block(block);
    f.render_widget(p, area);
}

fn render_memory(f: &mut Frame, app: &App, area: Rect) {
    let (used, total, pct) = if let Some(mem) = &app.dashboard.mem {
        let pct = if mem.total > 0 { (mem.used * 100 / mem.total) as u16 } else { 0 };
        (mem.used, mem.total, pct)
    } else {
        (0, 0, 0)
    };

    let label = format!(
        " {:.1} / {:.1} GB  {}%",
        used as f64 / 1_073_741_824.0,
        total as f64 / 1_073_741_824.0,
        pct
    );
    let color = if pct > 85 { Color::Red } else if pct > 65 { Color::Yellow } else { Color::Green };
    let gauge = Gauge::default()
        .block(Block::default().title(" Memory ").borders(Borders::ALL))
        .gauge_style(Style::default().fg(color))
        .label(label)
        .percent(pct);
    f.render_widget(gauge, area);
}

fn render_cpu(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" CPU (per core) ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.dashboard.cpu_pct.is_empty() {
        let p = Paragraph::new(Span::styled("Loading…", Style::default().fg(Color::DarkGray)));
        f.render_widget(p, inner);
        return;
    }

    let n = app.dashboard.cpu_pct.len();
    let heights: Vec<Constraint> = (0..n).map(|_| Constraint::Length(1)).collect();
    let rows = Layout::default().direction(Direction::Vertical).constraints(heights).split(inner);

    for (i, &pct) in app.dashboard.cpu_pct.iter().enumerate() {
        if i >= rows.len() { break; }
        let color = if pct > 85.0 { Color::Red } else if pct > 65.0 { Color::Yellow } else { Color::Green };
        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(color))
            .label(format!("{:3}%", pct as u32))
            .ratio((pct / 100.0).clamp(0.0, 1.0) as f64);
        f.render_widget(gauge, rows[i]);
    }
}

fn render_disks(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Disk ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.dashboard.disks.is_empty() {
        let p = Paragraph::new(Span::styled("Loading…", Style::default().fg(Color::DarkGray)));
        f.render_widget(p, inner);
        return;
    }

    let n = app.dashboard.disks.len();
    let heights: Vec<Constraint> = (0..n).map(|_| Constraint::Length(2)).collect();
    let rows = Layout::default().direction(Direction::Vertical).constraints(heights).split(inner);

    for (i, disk) in app.dashboard.disks.iter().enumerate() {
        if i >= rows.len() { break; }
        let pct = if disk.total > 0 { (disk.used * 100 / disk.total) as u16 } else { 0 };
        let color = if pct > 85 { Color::Red } else if pct > 65 { Color::Yellow } else { Color::Green };
        let label = format!(
            "{} {:.0}/{:.0}G {}%",
            disk.mount,
            disk.used as f64 / 1_073_741_824.0,
            disk.total as f64 / 1_073_741_824.0,
            pct
        );
        let gauge = LineGauge::default()
            .filled_style(Style::default().fg(color))
            .label(label)
            .ratio((pct as f64 / 100.0).clamp(0.0, 1.0));
        f.render_widget(gauge, rows[i]);
    }
}

fn format_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{}d {}h {}m", days, hours, mins)
    } else {
        format!("{}h {}m", hours, mins)
    }
}
