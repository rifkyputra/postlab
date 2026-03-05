pub mod app;
pub mod events;
pub mod screens;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Borders, Clear, Paragraph, Tabs},
    Terminal,
};
use sqlx::SqlitePool;
use std::{io, time::Duration};

use crate::core::Platform;
use app::{App, ConfirmDialog, Screen};

pub async fn run(platform: Platform, pool: SqlitePool) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(platform, pool);
    // Kick off initial dashboard load
    app.set_screen(Screen::Dashboard);

    let tick = Duration::from_millis(250);
    let result = run_loop(&mut terminal, &mut app, tick).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }
    Ok(())
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    tick: Duration,
) -> Result<()> {
    loop {
        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // nav bar
                    Constraint::Min(0),    // content
                    Constraint::Length(1), // status bar
                ])
                .split(area);

            render_nav(f, app, chunks[0]);

            match app.screen {
                Screen::Dashboard => screens::dashboard::render(f, app, chunks[1]),
                Screen::Packages  => screens::packages::render(f, app, chunks[1]),
                Screen::Security  => screens::security::render(f, app, chunks[1]),
                Screen::Gateway   => screens::gateway::render(f, app, chunks[1]),
                Screen::Tunnel    => screens::tunnel::render(f, app, chunks[1]),
                Screen::Docker      => screens::docker::render(f, app, chunks[1]),
                Screen::WasmCloud => screens::wasmcloud::render(f, app, chunks[1]),
                Screen::Ghosts    => screens::ghost::render(f, app, chunks[1]),
                Screen::Users     => screens::users::render(f, app, chunks[1]),
                Screen::Services  => screens::services::render(f, app, chunks[1]),
                Screen::Maintenance => screens::maintenance::render(f, app, chunks[1]),
            }

            render_status_bar(f, app, chunks[2]);

            if let Some(confirm) = &app.confirm {
                render_confirm_dialog(f, confirm, area);
            }
        })?;

        // Poll for events
        if event::poll(tick)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if events::handle_key(app, key).await {
                        break;
                    }
                }
            }
        }

        // Suspend TUI, run `cloudflared tunnel login` in the foreground, then resume.
        if app.needs_login {
            app.needs_login = false;
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

            let status = tokio::process::Command::new("cloudflared")
                .args(["tunnel", "login"])
                .status()
                .await;

            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;

            match status {
                Ok(s) if s.success() => {
                    app.status_msg = Some("Login successful — loading tunnels…".to_string());
                    app.spawn_load_tunnels();
                }
                Ok(_) => {
                    app.status_msg = Some("Login cancelled or failed".to_string());
                }
                Err(e) => {
                    app.status_msg = Some(format!("cloudflared not found: {}", e));
                }
            }
        }

        // Async tick — refreshes live data
        app.tick().await;
    }
    Ok(())
}

fn render_nav(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let titles: Vec<&str> = Screen::all()
        .iter()
        .map(|s| s.title())
        .collect();
    let tabs = Tabs::new(titles)
        .block(Block::default().borders(Borders::ALL).title(" postlab "))
        .select(app.screen.index())
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));
    f.render_widget(tabs, area);
}

fn render_status_bar(f: &mut ratatui::Frame, app: &App, area: ratatui::layout::Rect) {
    let msg = app.status_msg.as_deref().unwrap_or("[q] quit  [1-8] screens  [Tab] next  [←/→] switch tabs");
    let style = if app.status_msg.is_some() {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let p = Paragraph::new(Span::styled(msg, style));
    f.render_widget(p, area);
}

fn render_confirm_dialog(f: &mut ratatui::Frame, dialog: &ConfirmDialog, area: ratatui::layout::Rect) {
    let w = (dialog.message.len() as u16 + 4).min(area.width.saturating_sub(4));
    let h = 3u16;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    let popup = ratatui::layout::Rect { x, y, width: w, height: h };

    f.render_widget(Clear, popup);
    let p = Paragraph::new(dialog.message.as_str())
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().borders(Borders::ALL).title(" Confirm "));
    f.render_widget(p, popup);
}
