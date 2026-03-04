use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};
use crate::tui::app::App;

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let header_cells = ["Username", "UID", "GID", "Home", "Shell", "Groups"]
        .iter()
        .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray))
        .height(1)
        .bottom_margin(1);

    let rows = app.users.users.iter().map(|u| {
        let cells = vec![
            Cell::from(u.username.clone()),
            Cell::from(u.uid.to_string()),
            Cell::from(u.gid.to_string()),
            Cell::from(u.home.clone()),
            Cell::from(u.shell.clone()),
            Cell::from(u.groups.join(", ")),
        ];
        Row::new(cells).height(1)
    });

    let t = Table::new(rows, [
        Constraint::Percentage(15),
        Constraint::Percentage(5),
        Constraint::Percentage(5),
        Constraint::Percentage(20),
        Constraint::Percentage(15),
        Constraint::Percentage(40),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Unix Users "))
    .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.users.table_state);

    if app.users.loading {
        let loading_block = Block::default()
            .borders(Borders::ALL)
            .title(" Loading... ")
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
}
