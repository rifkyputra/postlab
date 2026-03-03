use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

use crate::tui::app::{App, InputMode};

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // content
            Constraint::Length(1), // hints
        ])
        .split(area);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ])
        .split(chunks[0]);

    render_local_keys(f, app, content_chunks[0]);
    render_authorized_keys(f, app, content_chunks[1]);

    render_hints(f, app, chunks[1]);

    if app.ssh.input_mode == InputMode::Editing {
        render_generate_input(f, app, area);
    }
}

fn render_local_keys(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Local Public Keys (~/.ssh/*.pub) ")
        .borders(Borders::ALL)
        .border_style(if app.ssh.focus == 0 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    if app.ssh.loading && app.ssh.local_keys.is_empty() {
        f.render_widget(Paragraph::new(" Loading...").block(block), area);
        return;
    }

    let items: Vec<ListItem> = app.ssh.local_keys.iter().map(|k| {
        ListItem::new(vec![
            Line::from(vec![
                Span::styled(format!(" {} ", k.key_type), Style::default().bg(Color::Blue).fg(Color::White)),
                Span::raw(" "),
                Span::styled(&k.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", k.fingerprint), Style::default().fg(Color::DarkGray)),
            ]),
        ])
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = app.ssh.local_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_authorized_keys(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Authorized Keys (~/.ssh/authorized_keys) ")
        .borders(Borders::ALL)
        .border_style(if app.ssh.focus == 1 {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        });

    if app.ssh.loading && app.ssh.authorized_keys.is_empty() {
        f.render_widget(Paragraph::new(" Loading...").block(block), area);
        return;
    }

    let items: Vec<ListItem> = app.ssh.authorized_keys.iter().map(|k| {
        ListItem::new(vec![
            Line::from(vec![
                Span::styled(format!(" {} ", k.key_type), Style::default().bg(Color::Green).fg(Color::Black)),
                Span::raw(" "),
                Span::styled(&k.name, Style::default().add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled(format!("   {}", k.fingerprint), Style::default().fg(Color::DarkGray)),
            ]),
        ])
    }).collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
        .highlight_symbol("▶ ");

    let mut state = app.ssh.authorized_state.clone();
    f.render_stateful_widget(list, area, &mut state);
}

fn render_hints(f: &mut Frame, app: &App, area: Rect) {
    let common = " [Tab] next screen  [r] refresh  [g] generate key ";
    let specific = if app.ssh.focus == 0 {
        " [a] authorize selected local key  [Right] focus authorized "
    } else {
        " [D] deauthorize selected key  [Left] focus local "
    };
    
    let text = format!("{}{}", common, specific);
    let hints = Paragraph::new(Span::styled(text, Style::default().fg(Color::DarkGray)));
    f.render_widget(hints, area);
}

fn render_generate_input(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Generate New SSH Key ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    
    let popup_area = Rect {
        x: area.width / 4,
        y: area.height / 2 - 3,
        width: area.width / 2,
        height: 6,
    };

    f.render_widget(Clear, popup_area);
    
    let text = vec![
        Line::from(vec![
            Span::raw(" Name: "),
            Span::styled(&app.ssh.input_name, Style::default().fg(Color::Cyan)),
            Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
        ]),
        Line::from(vec![
            Span::raw(" Type: "),
            Span::styled(&app.ssh.input_type, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Enter] ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("generate  "),
            Span::styled(" [Esc] ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("cancel"),
        ]),
    ];

    f.render_widget(Paragraph::new(text).block(block), popup_area);
}
