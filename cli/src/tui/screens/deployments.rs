use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};
use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(area);

    let main_area = chunks[0];

    // Build the table
    let header_cells = ["Repository", "Deploy Type", "Status", "Last Updated"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray))
        .height(1)
        .bottom_margin(1);

    let rows = app.deployments.list.iter().map(|d| {
        let status_color = match d.status {
            crate::core::models::DeploymentStatus::Running => Color::Green,
            crate::core::models::DeploymentStatus::Stopped => Color::DarkGray,
            crate::core::models::DeploymentStatus::Failed(_) => Color::Red,
            _ => Color::Yellow,
        };

        let cells = vec![
            Cell::from(d.repo_url.clone()),
            Cell::from(d.deploy_type.label().to_string()),
            Cell::from(d.status.label().to_string()).style(Style::default().fg(status_color)),
            Cell::from(d.last_updated.clone()),
        ];
        Row::new(cells).height(1)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(40),
        Constraint::Percentage(15),
        Constraint::Percentage(15),
        Constraint::Percentage(30),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Deployments [a] Add [r] Reload [u] Update [t] Teardown [d] Delete "))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, main_area, &mut app.deployments.table_state);

    if app.deployments.loading {
        let loading_block = Block::default()
            .borders(Borders::ALL)
            .title(" Processing... ")
            .style(Style::default().fg(Color::Yellow));
        let loading_area = Rect {
            x: area.x + area.width / 4,
            y: area.y + area.height / 2 - 1,
            width: area.width / 2,
            height: 3,
        };
        f.render_widget(ratatui::widgets::Clear, loading_area);
        f.render_widget(loading_block, loading_area);
    }

    if app.deployments.input_mode == InputMode::AddingDomain {
        let pop_area = ratatui::layout::Rect {
            x: area.x + (area.width.saturating_sub(60)) / 2,
            y: area.y + (area.height.saturating_sub(3)) / 2,
            width: 60,
            height: 3,
        };
        f.render_widget(Clear, pop_area);
        let block = Block::default().title(" Clone URL (e.g. https://github.com/user/repo.git) ").borders(Borders::ALL);
        
        let mut text = app.deployments.input_url.clone();
        if text.is_empty() {
            text = "Enter repo URL...".to_string();
        }

        let paragraph = Paragraph::new(text)
            .style(Style::default().fg(ratatui::style::Color::Yellow))
            .block(block);

        f.render_widget(paragraph, pop_area);
    }

}
