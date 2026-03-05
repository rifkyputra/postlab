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
            Constraint::Length(5), // Backbone health + hints
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

    // ── Backbone Health + Footer ──
    let mut footer_text = vec![];

    // NATS health row — always visible so the user can provision before wash is installed
    let nats_indicator = if app.wasm_cloud.nats_running {
        Span::styled("🟢 NATS Up (Port 4222)  ", Style::default().fg(Color::Green))
    } else {
        Span::styled("🔴 NATS Down  ", Style::default().fg(Color::Red))
    };
    let js_indicator = if let Some(usage) = app.wasm_cloud.nats_storage_usage {
        let mb = usage / 1_000_000;
        Span::styled(
            format!("🟢 JetStream Active ({}MB / 2048MB)  ", mb),
            Style::default().fg(Color::Green),
        )
    } else {
        Span::styled("🔴 JetStream Inactive  ", Style::default().fg(Color::Red))
    };
    let lattice_indicator = if app.wasm_cloud.nats_synced {
        Span::styled(
            format!("🟡 Lattice synced ({} host(s) connected)", app.wasm_cloud.hosts.len()),
            Style::default().fg(Color::Yellow),
        )
    } else {
        Span::styled("🔴 Lattice not synced", Style::default().fg(Color::Red))
    };
    footer_text.push(Line::from(vec![nats_indicator, js_indicator, lattice_indicator]));

    // Key-hint row
    if app.wasm_cloud.installed {
        footer_text.push(Line::from(format!(
            " wash v{}  |  [←/→] Switch tab  [r] Refresh  [n] Provision NATS  [N] Poll NATS  [i] Reinstall wash",
            app.wasm_cloud.version.as_deref().unwrap_or("unknown")
        )));
    } else {
        footer_text.push(Line::from(vec![
            Span::raw(" wash not found  |  "),
            Span::styled("[i]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" Install wash  "),
            Span::styled("[n]", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" Provision NATS Backbone  "),
            Span::styled("[←/→]", Style::default().fg(Color::DarkGray)),
            Span::raw(" Switch tab"),
        ]));
    }

    let footer = Paragraph::new(footer_text)
        .block(Block::default().borders(Borders::ALL).title(" 🐛 NATS Backbone "));
    f.render_widget(footer, chunks[2]);
}

fn render_not_installed(f: &mut Frame, _app: &App, area: Rect) {
    let text = vec![
        Line::from(Span::styled("wasmCloud (wash) is not installed", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from("To use wasmCloud features, please install the wash CLI."),
        Line::from(""),
        Line::from(vec![
            Span::raw("Press "),
            Span::styled("i", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" to install automatically"),
        ]),
        Line::from(""),
        Line::from("Alternatively, run these commands:"),
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
    use crate::tui::app::InputMode;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Input
            Constraint::Min(0),    // Output
        ])
        .split(area);

    let (style, title) = match app.wasm_cloud.input_mode {
        InputMode::Editing => (Style::default().fg(Color::Yellow), " [ EDITING ] Wasm Component Path / URL (Esc to exit) "),
        _ => (Style::default(), " Wasm Component Path / URL (Press Enter/i to edit) "),
    };

    let input = Paragraph::new(app.wasm_cloud.inspect_target.as_str())
        .block(Block::default().borders(Borders::ALL).title(title).style(style));
    f.render_widget(input, chunks[0]);

    let output = Paragraph::new(app.wasm_cloud.inspect_output.as_deref().unwrap_or("Enter a path and press Enter to inspect component capabilities..."))
        .block(Block::default().borders(Borders::ALL).title(" Capability Inspector Output "))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(output, chunks[1]);
}
