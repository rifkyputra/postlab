use crate::tui::app::{App, InputMode};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Cell, Clear, Paragraph, Row, Table},
    Frame,
};

fn masked(s: &str) -> String {
    "*".repeat(s.len())
}

pub fn render(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    render_table(f, app, chunks[0]);
    render_hints(f, app, chunks[1]);

    match app.users.input_mode {
        InputMode::Editing => render_add_popup(f, app, area),
        InputMode::SettingPassword => render_pw_popup(f, app, area),
        _ => {}
    }

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
        f.render_widget(Clear, loading_area);
        f.render_widget(loading_block, loading_area);
    }
}

fn render_table(f: &mut Frame, app: &mut App, area: Rect) {
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

    let t = Table::new(
        rows,
        [
            Constraint::Percentage(15),
            Constraint::Percentage(5),
            Constraint::Percentage(5),
            Constraint::Percentage(20),
            Constraint::Percentage(15),
            Constraint::Percentage(40),
        ],
    )
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Unix Users "))
    .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
    .highlight_symbol(">> ");

    f.render_stateful_widget(t, area, &mut app.users.table_state);
}

fn render_hints(f: &mut Frame, _app: &App, area: Rect) {
    let p = Paragraph::new(Span::styled(
        " [a] add  [p] set password  [s] sudoers  [d] delete  [r] refresh ",
        Style::default().fg(Color::DarkGray),
    ));
    f.render_widget(p, area);
}

fn render_add_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 9, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // username
            Constraint::Length(3), // shell
            Constraint::Length(1), // hints
        ])
        .split(popup_area);

    let cursor = "█";
    let focused = |field: usize| {
        if app.users.input_focus == field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        }
    };

    let username_text = if app.users.input_focus == 0 {
        format!("{}{}", app.users.input_username, cursor)
    } else {
        app.users.input_username.clone()
    };
    f.render_widget(
        Paragraph::new(username_text)
            .style(focused(0))
            .block(Block::default().title(" Username ").borders(Borders::ALL)),
        chunks[0],
    );

    let shell_text = if app.users.input_focus == 1 {
        format!("{}{}", app.users.input_shell, cursor)
    } else if app.users.input_shell.is_empty() {
        String::new()
    } else {
        app.users.input_shell.clone()
    };
    f.render_widget(
        Paragraph::new(shell_text).style(focused(1)).block(
            Block::default()
                .title(" Shell (blank=/bin/bash) ")
                .borders(Borders::ALL),
        ),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            " [Tab] next field  [Enter] confirm  [Esc] cancel ",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[2],
    );
}

fn render_pw_popup(f: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(50, 9, area);
    f.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // password
            Constraint::Length(3), // confirm
            Constraint::Length(1), // hints
        ])
        .split(popup_area);

    let cursor = "█";
    let focused = |field: usize| {
        if app.users.pw_focus == field {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        }
    };

    let title = format!(" Set Password — {} ", app.users.pw_target);

    let pw_text = if app.users.pw_focus == 0 {
        format!("{}{}", masked(&app.users.pw_password), cursor)
    } else {
        masked(&app.users.pw_password)
    };
    f.render_widget(
        Paragraph::new(pw_text)
            .style(focused(0))
            .block(Block::default().title(title).borders(Borders::ALL)),
        chunks[0],
    );

    let confirm_text = if app.users.pw_focus == 1 {
        format!("{}{}", masked(&app.users.pw_confirm), cursor)
    } else {
        masked(&app.users.pw_confirm)
    };
    f.render_widget(
        Paragraph::new(confirm_text).style(focused(1)).block(
            Block::default()
                .title(" Confirm Password ")
                .borders(Borders::ALL),
        ),
        chunks[1],
    );

    f.render_widget(
        Paragraph::new(Span::styled(
            " [Tab] next field  [Enter] confirm  [Esc] cancel ",
            Style::default().fg(Color::DarkGray),
        )),
        chunks[2],
    );
}

fn centered_rect(percent_x: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(r.width * percent_x / 100)) / 2;
    let w = r.width * percent_x / 100;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: w,
        height,
    }
}
