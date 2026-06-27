use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Tabs},
    Frame,
};

use crate::tui::app::{App, SystemTab};

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // tab bar
            Constraint::Min(0),    // content
        ])
        .split(area);

    let tab = app.system_tab.clone();
    render_tabs(f, app, chunks[0]);

    match tab {
        SystemTab::Ghosts => super::ghost::render(f, app, chunks[1]),
        SystemTab::Janitor => super::maintenance::render(f, app, chunks[1]),
        SystemTab::Services => super::services::render(f, app, chunks[1]),
        SystemTab::Users => super::users::render(f, app, chunks[1]),
        SystemTab::Swap => super::swap::render(f, app, chunks[1]),
        SystemTab::Storage => super::storage::render(f, app, chunks[1]),
    }
}

fn render_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<&str> = SystemTab::all().iter().map(|t| t.title()).collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::BOTTOM))
        .select(app.system_tab.index())
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));
    f.render_widget(tabs, area);
}
