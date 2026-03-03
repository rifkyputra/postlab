use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Span,
    widgets::{Block, Borders, Paragraph, Sparkline},
    Frame,
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // CPU sparklines
            Constraint::Percentage(30), // Memory
            Constraint::Percentage(30), // Network
        ])
        .split(area);

    render_cpu_sparklines(f, app, chunks[0]);
    render_mem_sparkline(f, app, chunks[1]);
    render_net_sparkline(f, app, chunks[2]);
}

fn render_cpu_sparklines(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" CPU History (60s) ").borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.resources.cpu_history.is_empty() {
        let p = Paragraph::new(Span::styled("Loading…", Style::default().fg(Color::DarkGray)));
        f.render_widget(p, inner);
        return;
    }

    let n = app.resources.cpu_history.len().min(4);
    let heights: Vec<Constraint> = (0..n).map(|_| Constraint::Min(1)).collect();
    let rows = Layout::default().direction(Direction::Vertical).constraints(heights).split(inner);

    for (i, history) in app.resources.cpu_history.iter().take(n).enumerate() {
        if i >= rows.len() { break; }
        let current = history.last().copied().unwrap_or(0);
        let color = if current > 85 { Color::Red } else if current > 65 { Color::Yellow } else { Color::Green };
        let sparkline = Sparkline::default()
            .block(Block::default())
            .data(history)
            .style(Style::default().fg(color))
            .max(100);
        f.render_widget(sparkline, rows[i]);
    }
}

fn render_mem_sparkline(f: &mut Frame, app: &App, area: Rect) {
    let current = app.resources.mem_history.last().copied().unwrap_or(0);
    let color = if current > 85 { Color::Red } else if current > 65 { Color::Yellow } else { Color::Green };

    let label = format!(" Memory History (60s) — current {}% ", current);
    let sparkline = Sparkline::default()
        .block(Block::default().title(label).borders(Borders::ALL))
        .data(&app.resources.mem_history)
        .style(Style::default().fg(color))
        .max(100);
    f.render_widget(sparkline, area);
}

fn render_net_sparkline(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let rx_current = app.resources.net_rx_history.last().copied().unwrap_or(0);
    let tx_current = app.resources.net_tx_history.last().copied().unwrap_or(0);

    let max_rx = app.resources.net_rx_history.iter().max().copied().unwrap_or(1).max(1);
    let max_tx = app.resources.net_tx_history.iter().max().copied().unwrap_or(1).max(1);

    let rx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(format!(" RX — {}KB/s ", rx_current))
            .borders(Borders::ALL))
        .data(&app.resources.net_rx_history)
        .style(Style::default().fg(Color::Cyan))
        .max(max_rx);
    f.render_widget(rx_sparkline, chunks[0]);

    let tx_sparkline = Sparkline::default()
        .block(Block::default()
            .title(format!(" TX — {}KB/s ", tx_current))
            .borders(Borders::ALL))
        .data(&app.resources.net_tx_history)
        .style(Style::default().fg(Color::Magenta))
        .max(max_tx);
    f.render_widget(tx_sparkline, chunks[1]);
}
