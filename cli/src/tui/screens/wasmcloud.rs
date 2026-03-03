use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Tabs},
    Frame,
};

use crate::tui::app::{App, WasmCloudTab};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Tabs
            Constraint::Min(0),    // Content
            Constraint::Length(3), // Status/Footer
        ])
        .split(area);

    // ── Tabs ──
    let titles = WasmCloudTab::all()
        .iter()
        .map(|t| Line::from(t.title()))
        .collect::<Vec<_>>();

    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" wasmCloud "))
        .select(app.wasm_cloud.active_tab.index())
        .style(Style::default().fg(Color::Cyan))
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(tabs, chunks[0]);

    // ── Content ──
    if !app.wasm_cloud.installed {
        render_not_installed(f, app, chunks[1]);
    } else {
        match app.wasm_cloud.active_tab {
            WasmCloudTab::Hosts => render_hosts(f, app, chunks[1]),
            WasmCloudTab::Components => render_components(f, app, chunks[1]),
            WasmCloudTab::Apps => render_apps(f, app, chunks[1]),
            WasmCloudTab::Inspector => render_inspector(f, app, chunks[1]),
        }
    }

    // ── Footer ──
    let info = if app.wasm_cloud.installed {
        format!(
            " wash v{} | [Tab] Next Tab | [r] Refresh | [Enter] Select ",
            app.wasm_cloud.version.as_deref().unwrap_or("unknown")
        )
    } else {
        " wash not found | install wasmCloud to continue ".to_string()
    };
    let footer = Paragraph::new(info)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(footer, chunks[2]);
}

fn render_not_installed(f: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(Span::styled("wasmCloud (wash) is not installed", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("To use wasmCloud features, please install the wash CLI:"),
        Line::from(Span::styled("curl -s https://packagecloud.io/install/repositories/wasmcloud/core/script.deb.sh | sudo bash", Style::default().fg(Color::Yellow))),
        Line::from(Span::styled("sudo apt install wash", Style::default().fg(Color::Yellow))),
    ];
    let p = Paragraph::new(text)
        .block(Block::default().borders(Borders::ALL))
        .alignment(ratatui::layout::Alignment::Center);
    
    // Center it vertically a bit
    let mut center_area = area;
    if area.height > 10 {
        center_area.y += (area.height - 10) / 2;
        center_area.height = 10;
    }
    f.render_widget(p, center_area);
}

fn render_hosts(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Host ID", "Name", "Uptime", "Labels"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.wasm_cloud.hosts.iter().map(|h| {
        let labels = h.labels.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join(", ");
        
        Row::new(vec![
            Cell::from(h.id.chars().take(8).collect::<String>()),
            Cell::from(h.friendly_name.as_str()),
            Cell::from(format!("{}s", h.uptime_secs)),
            Cell::from(labels),
        ])
    });

    let t = Table::new(rows, [
        Constraint::Length(10),
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Min(20),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Hosts "))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.wasm_cloud.hosts_state);
}

fn render_components(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["ID", "Name", "Type", "Image Ref"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.wasm_cloud.components.iter().map(|c| {
        Row::new(vec![
            Cell::from(c.id.chars().take(8).collect::<String>()),
            Cell::from(c.name.as_str()),
            Cell::from(c.component_type.as_str()),
            Cell::from(c.image_ref.as_str()),
        ])
    });

    let t = Table::new(rows, [
        Constraint::Length(10),
        Constraint::Length(20),
        Constraint::Length(12),
        Constraint::Min(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Components "))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.wasm_cloud.components_state);
}

fn render_apps(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Name", "Version", "Status", "Description"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells).height(1).bottom_margin(1);

    let rows = app.wasm_cloud.apps.iter().map(|a| {
        Row::new(vec![
            Cell::from(a.name.as_str()),
            Cell::from(a.version.as_str()),
            Cell::from(a.status.as_str()),
            Cell::from(a.description.as_str()),
        ])
    });

    let t = Table::new(rows, [
        Constraint::Length(20),
        Constraint::Length(10),
        Constraint::Length(15),
        Constraint::Min(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Applications (WADM) "))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.wasm_cloud.apps_state);
}

fn render_inspector(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input
            Constraint::Min(0),    // Output
        ])
        .split(area);

    let input = Paragraph::new(app.wasm_cloud.inspect_target.as_str())
        .block(Block::default().borders(Borders::ALL).title(" Wasm Component Path / URL "));
    f.render_widget(input, chunks[0]);

    let output = Paragraph::new(app.wasm_cloud.inspect_output.as_deref().unwrap_or("Enter a path and press Enter to inspect component capabilities..."))
        .block(Block::default().borders(Borders::ALL).title(" Capability Inspector Output "))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(output, chunks[1]);
}
