use crossterm::event::{KeyCode, KeyEvent};

use crate::core::models::{Route, TunnelRoute};

use super::app::{App, ConfirmAction, ConfirmDialog, DashboardTab, InputMode, PackageTab, ProcessSort, Screen, SecurityTab, TunnelPanel, PROTOS, ACTIONS};

/// Returns true if the app should quit.
pub async fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    // 1. Confirm dialog takes priority over everything
    if app.confirm.is_some() {
        return handle_confirm(app, key);
    }

    // 2. Text-entry (input mode) consumes all keys — global shortcuts are blocked
    //    so the user can type freely without triggering screen switches or quit.
    if app.screen == Screen::WasmCloud && app.wasm_cloud.input_mode == InputMode::Editing {
        handle_wasm_cloud_inspector_input(app, key);
        return false;
    }
    if app.screen == Screen::Packages
        && (app.packages.filter_mode == InputMode::Editing
            || app.packages.search_mode == InputMode::Editing)
    {
        handle_packages_key(app, key).await;
        return false;
    }
    if app.screen == Screen::Gateway && app.gateway.input_mode == InputMode::Editing {
        handle_gateway_input(app, key);
        return false;
    }
    if app.screen == Screen::Tunnel && matches!(app.tunnel.input_mode, InputMode::Editing | InputMode::AddingDomain | InputMode::EditingIngress) {
        handle_tunnel_input(app, key);
        return false;
    }
    if app.screen == Screen::Services && app.services.filter_mode == InputMode::Editing {
        handle_services_key(app, key);
        return false;
    }
    if app.screen == Screen::Security {
        match app.security.active_tab {
            SecurityTab::Firewall if app.firewall.input_mode == InputMode::Editing => {
                handle_firewall_input(app, key);
                return false;
            }
            SecurityTab::Ports if app.portchecker.input_mode == InputMode::Editing => {
                handle_portchecker_input(app, key);
                return false;
            }
            SecurityTab::Ssh if app.ssh.input_mode == InputMode::Editing => {
                handle_ssh_input(app, key);
                return false;
            }
            _ => {}
        }
    }

    // 3. Global keys — always reachable when not in text-entry mode
    match key.code {
        KeyCode::Char('q') => return true,
        KeyCode::Char('1') => { app.set_screen_by_index(0); return false; }
        KeyCode::Char('2') => { app.set_screen_by_index(1); return false; }
        KeyCode::Char('3') => { app.set_screen_by_index(2); return false; }
        KeyCode::Char('4') => { app.set_screen_by_index(3); return false; }
        KeyCode::Char('5') => { app.set_screen_by_index(4); return false; }
        KeyCode::Char('6') => { app.set_screen_by_index(5); return false; }
        KeyCode::Char('7') => { app.set_screen_by_index(6); return false; }
        KeyCode::Char('8') => { app.set_screen_by_index(7); return false; }
        KeyCode::Char('9') => { app.set_screen_by_index(8); return false; }
        KeyCode::Char('0') => { app.set_screen_by_index(9); return false; }
        KeyCode::Char('m') | KeyCode::Char('M') => { app.set_screen_by_index(10); return false; }
        KeyCode::Tab => { app.next_screen(); return false; }
        KeyCode::BackTab => { app.prev_screen(); return false; }
        _ => {}
    }

    // 4. Screen-specific keys
    match app.screen.clone() {
        Screen::Dashboard   => handle_dashboard_key(app, key),
        Screen::Packages    => { handle_packages_key(app, key).await; }
        Screen::Security    => handle_security_key(app, key),
        Screen::Gateway     => handle_gateway_key(app, key),
        Screen::Tunnel      => handle_tunnel_key(app, key),
        Screen::Docker      => handle_docker_key(app, key),
        Screen::WasmCloud   => handle_wasm_cloud_key(app, key),
        Screen::Ghosts      => handle_ghost_key(app, key),
        Screen::Users       => handle_users_key(app, key),
        Screen::Services    => handle_services_key(app, key),
        Screen::Maintenance => handle_maintenance_key(app, key),
    }
    false
}

fn handle_confirm(app: &mut App, key: KeyEvent) -> bool {
    let action = match app.confirm.take() {
        Some(d) => d.action,
        None => return false,
    };
    match key.code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            execute_confirmed(app, action);
        }
        _ => {} // any other key cancels
    }
    false
}

fn execute_confirmed(app: &mut App, action: ConfirmAction) {
    match action {
        ConfirmAction::KillProcess { pid, name: _ } => {
            let platform = std::sync::Arc::clone(&app.platform);
            let tx = app.task_tx.clone();
            tokio::spawn(async move {
                let result = platform.processes.kill(pid).await;
                let success = result.is_ok();
                let output = if success {
                    "killed".to_string()
                } else {
                    result.unwrap_err().to_string()
                };
                let _ = tx.send(crate::tui::app::TaskResult::OpDone {
                    op: "kill".to_string(),
                    target: pid.to_string(),
                    output,
                    success,
                });
            });
        }
        ConfirmAction::RemovePackage { name } => app.spawn_remove(name),
        ConfirmAction::ApplySecurityFix { id, title: _ } => app.spawn_security_apply(id),
        ConfirmAction::DeleteRoute { domain } => {
            let platform = std::sync::Arc::clone(&app.platform);
            let tx = app.task_tx.clone();
            tokio::spawn(async move {
                let _ = platform.gateway.remove_route(&domain).await;
                match platform.gateway.list_routes().await {
                    Ok(routes) => { let _ = tx.send(crate::tui::app::TaskResult::RouteList(routes)); }
                    Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                }
            });
        }
        ConfirmAction::DeleteTunnel { name: _ } => {
            app.status_msg = Some("Tunnel deletion not yet implemented via cloudflared CLI".to_string());
        }
        ConfirmAction::DeleteIngress { tunnel_id, hostname } => {
            let platform = std::sync::Arc::clone(&app.platform);
            let tx = app.task_tx.clone();
            tokio::spawn(async move {
                match platform.tunnel.remove_ingress(&tunnel_id, &hostname).await {
                    Ok(_) => {
                        if let Ok(c) = platform.tunnel.config_content(&tunnel_id).await {
                            let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(c));
                        }
                        let _ = tx.send(crate::tui::app::TaskResult::Status(
                            format!("Removed ingress entry: {}", hostname),
                        ));
                    }
                    Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                }
            });
        }
        ConfirmAction::StopContainer { id, name: _ } => app.spawn_docker_container_action("stop", id),
        ConfirmAction::RemoveContainer { id, name: _ } => app.spawn_docker_container_action("remove", id),
        ConfirmAction::RemoveImage { id, tag: _ } => app.spawn_docker_image_remove(id),
        ConfirmAction::DeleteFirewallRule { num } => app.spawn_firewall_delete_rule(num),
        ConfirmAction::Fail2BanForgive { ip, jail } => app.spawn_fail2ban_unban(jail, ip),
        ConfirmAction::Fail2BanBanish { ip, jail } => app.spawn_fail2ban_banish(jail, ip),
        ConfirmAction::DeauthorizeKey { fingerprint, name: _ } => app.spawn_deauthorize_key(fingerprint),
        ConfirmAction::AuthorizeLocalKey { content, name: _ } => app.spawn_authorize_key(content),
        ConfirmAction::KillGhost { pid, name: _ } => {
            let tx = app.task_tx.clone();
            tokio::spawn(async move {
                let result = tokio::process::Command::new("kill")
                    .args(["-15", &pid.to_string()])
                    .output()
                    .await;
                let success = result.map(|o| o.status.success()).unwrap_or(false);
                let _ = tx.send(crate::tui::app::TaskResult::Status(if success {
                    format!("Killed ghost process {}", pid)
                } else {
                    format!("Failed to kill PID {} — try as root?", pid)
                }));
            });
        }
        ConfirmAction::ServiceAction { name, op } => app.spawn_service_action(name, op),
        ConfirmAction::MaintenanceAction { op } => app.spawn_maintenance_action(op),
    }
}

// ── Dashboard ────────────────────────────────────────────────────────────

fn handle_dashboard_key(app: &mut App, key: KeyEvent) {
    // Left/Right switch between Overview / Processes / Resources tabs
    match key.code {
        KeyCode::Right | KeyCode::Char('L') => {
            let idx = (app.dashboard.active_tab.index() + 1) % DashboardTab::all().len();
            app.dashboard.active_tab = DashboardTab::all()[idx].clone();
            // Trigger immediate load when switching to Processes
            if app.dashboard.active_tab == DashboardTab::Processes {
                app.spawn_load_processes();
            }
            return;
        }
        KeyCode::Left | KeyCode::Char('H') => {
            let idx = app.dashboard.active_tab.index();
            let prev = if idx == 0 { DashboardTab::all().len() - 1 } else { idx - 1 };
            app.dashboard.active_tab = DashboardTab::all()[prev].clone();
            if app.dashboard.active_tab == DashboardTab::Processes {
                app.spawn_load_processes();
            }
            return;
        }
        _ => {}
    }

    // Dispatch tab-specific keys
    match app.dashboard.active_tab {
        DashboardTab::Overview => {
            if key.code == KeyCode::Char('r') {
                app.dashboard.os_info = None;
                app.dashboard.disks.clear();
            }
        }
        DashboardTab::Processes => handle_processes_key(app, key),
        DashboardTab::Resources => {} // read-only sparklines, no keys needed
    }
}

// ── Packages ─────────────────────────────────────────────────────────────

async fn handle_packages_key(app: &mut App, key: KeyEvent) -> bool {
    match app.packages.active_tab {
        PackageTab::Installed => handle_installed_tab(app, key),
        PackageTab::Search => handle_search_tab(app, key).await,
        PackageTab::QuickInstall => handle_quick_install_tab(app, key),
        PackageTab::Queue => handle_queue_tab(app, key),
    }
    false
}

fn handle_installed_tab(app: &mut App, key: KeyEvent) {
    if app.packages.filter_mode == InputMode::Editing {
        match key.code {
            KeyCode::Esc => app.packages.filter_mode = InputMode::Normal,
            KeyCode::Backspace => { app.packages.filter.pop(); }
            KeyCode::Char(c) => app.packages.filter.push(c),
            _ => {}
        }
        return;
    }
    match key.code {
        KeyCode::Char('r') => app.spawn_load_packages(),
        KeyCode::Char('/') => app.packages.filter_mode = InputMode::Editing,
        KeyCode::Esc => app.packages.filter.clear(),
        KeyCode::Down => {
            let count = visible_installed_count(app);
            list_next(&mut app.packages.installed_state, count);
        }
        KeyCode::Up => list_prev(&mut app.packages.installed_state),
        KeyCode::Char(' ') => toggle_installed_selected(app),
        KeyCode::Char('d') => remove_selected_packages(app),
        KeyCode::Right | KeyCode::Char('l') => {
            let idx = (app.packages.active_tab.index() + 1) % PackageTab::all().len();
            app.packages.active_tab = PackageTab::all()[idx].clone();
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let idx = app.packages.active_tab.index();
            let prev = if idx == 0 { PackageTab::all().len() - 1 } else { idx - 1 };
            app.packages.active_tab = PackageTab::all()[prev].clone();
        }
        _ => {}
    }
}

async fn handle_search_tab(app: &mut App, key: KeyEvent) {
    if app.packages.search_mode == InputMode::Editing {
        match key.code {
            KeyCode::Esc => app.packages.search_mode = InputMode::Normal,
            KeyCode::Enter => {
                let q = app.packages.search_query.clone();
                if !q.is_empty() {
                    app.spawn_search(q);
                    app.packages.search_mode = InputMode::Normal;
                }
            }
            KeyCode::Backspace => { app.packages.search_query.pop(); }
            KeyCode::Char(c) => app.packages.search_query.push(c),
            _ => {}
        }
        return;
    }
    match key.code {
        KeyCode::Char('/') | KeyCode::Char('s') => app.packages.search_mode = InputMode::Editing,
        KeyCode::Down => list_next(&mut app.packages.search_state, app.packages.search_results.len()),
        KeyCode::Up => list_prev(&mut app.packages.search_state),
        KeyCode::Char(' ') => toggle_search_selected(app),
        KeyCode::Char('i') => install_search_selected(app),
        KeyCode::Right => {
            let idx = (app.packages.active_tab.index() + 1) % PackageTab::all().len();
            app.packages.active_tab = PackageTab::all()[idx].clone();
        }
        KeyCode::Left => {
            let idx = app.packages.active_tab.index();
            let prev = if idx == 0 { PackageTab::all().len() - 1 } else { idx - 1 };
            app.packages.active_tab = PackageTab::all()[prev].clone();
        }
        _ => {}
    }
}

fn handle_quick_install_tab(app: &mut App, key: KeyEvent) {
    use crate::core::packages::CURATED;
    let flat: Vec<&'static str> = CURATED.iter().flat_map(|c| c.packages.iter().copied()).collect();

    // Build installed name set for O(1) lookup
    let installed: std::collections::HashSet<&str> = app.packages.installed
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    match key.code {
        KeyCode::Down => {
            app.packages.curated_cursor = (app.packages.curated_cursor + 1).min(flat.len().saturating_sub(1));
        }
        KeyCode::Up => {
            app.packages.curated_cursor = app.packages.curated_cursor.saturating_sub(1);
        }
        KeyCode::Char(' ') => {
            if let Some(&pkg) = flat.get(app.packages.curated_cursor) {
                let name = pkg.to_string();
                if installed.contains(pkg) {
                    // Toggle uninstall selection for already-installed packages
                    if app.packages.curated_uninstall.contains(&name) {
                        app.packages.curated_uninstall.remove(&name);
                    } else {
                        app.packages.curated_uninstall.insert(name);
                        // Make sure it's not in the install set
                        app.packages.curated_selected.remove(pkg);
                    }
                } else {
                    // Toggle install selection for not-installed packages
                    if app.packages.curated_selected.contains(&name) {
                        app.packages.curated_selected.remove(&name);
                    } else {
                        app.packages.curated_selected.insert(name);
                        app.packages.curated_uninstall.remove(pkg);
                    }
                }
            }
        }
        KeyCode::Enter => {
            let to_install: Vec<String> = app.packages.curated_selected.drain().collect();
            let to_remove: Vec<String> = app.packages.curated_uninstall.drain().collect();
            let has_ops = !to_install.is_empty() || !to_remove.is_empty();
            for pkg in to_install {
                app.spawn_install(pkg);
            }
            for pkg in to_remove {
                app.spawn_remove(pkg);
            }
            if has_ops {
                app.packages.active_tab = PackageTab::Queue;
            }
        }
        KeyCode::Right => {
            let idx = (app.packages.active_tab.index() + 1) % PackageTab::all().len();
            app.packages.active_tab = PackageTab::all()[idx].clone();
        }
        KeyCode::Left => {
            let idx = app.packages.active_tab.index();
            let prev = if idx == 0 { PackageTab::all().len() - 1 } else { idx - 1 };
            app.packages.active_tab = PackageTab::all()[prev].clone();
        }
        _ => {}
    }
}

fn handle_queue_tab(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.spawn_load_packages(),
        KeyCode::Down => {
            let len = app.packages.queue.len();
            if len > 0 {
                app.packages.queue_selected = Some(
                    app.packages.queue_selected.map(|i| (i + 1).min(len - 1)).unwrap_or(0),
                );
                app.packages.output_scroll = 0;
            }
        }
        KeyCode::Up => {
            if !app.packages.queue.is_empty() {
                app.packages.queue_selected = Some(
                    app.packages.queue_selected.map(|i| i.saturating_sub(1)).unwrap_or(0),
                );
                app.packages.output_scroll = 0;
            }
        }
        KeyCode::PageDown => {
            app.packages.output_scroll = app.packages.output_scroll.saturating_add(10);
        }
        KeyCode::PageUp => {
            app.packages.output_scroll = app.packages.output_scroll.saturating_sub(10);
        }
        KeyCode::Right => {
            let idx = (app.packages.active_tab.index() + 1) % PackageTab::all().len();
            app.packages.active_tab = PackageTab::all()[idx].clone();
        }
        KeyCode::Left => {
            let idx = app.packages.active_tab.index();
            let prev = if idx == 0 { PackageTab::all().len() - 1 } else { idx - 1 };
            app.packages.active_tab = PackageTab::all()[prev].clone();
        }
        _ => {}
    }
}

fn toggle_installed_selected(app: &mut App) {
    if let Some(idx) = app.packages.installed_state.selected() {
        let visible = filtered_installed(app);
        if let Some(pkg) = visible.get(idx) {
            let name = pkg.name.clone();
            if app.packages.selected.contains(&name) {
                app.packages.selected.remove(&name);
            } else {
                app.packages.selected.insert(name);
            }
        }
    }
}

fn toggle_search_selected(app: &mut App) {
    if let Some(idx) = app.packages.search_state.selected() {
        if let Some(pkg) = app.packages.search_results.get(idx) {
            let name = pkg.name.clone();
            if app.packages.search_selected.contains(&name) {
                app.packages.search_selected.remove(&name);
            } else {
                app.packages.search_selected.insert(name);
            }
        }
    }
}

fn install_search_selected(app: &mut App) {
    let selected: Vec<String> = app.packages.search_selected.drain().collect();
    for pkg in selected {
        app.spawn_install(pkg);
    }
    app.packages.active_tab = PackageTab::Queue;
}

fn remove_selected_packages(app: &mut App) {
    let selected: Vec<String> = app.packages.selected.iter().cloned().collect();
    if selected.is_empty() {
        if let Some(idx) = app.packages.installed_state.selected() {
            let visible = filtered_installed(app);
            if let Some(pkg) = visible.get(idx) {
                let name = pkg.name.clone();
                app.confirm = Some(ConfirmDialog {
                    message: format!("Remove {}? (y/N)", name),
                    action: ConfirmAction::RemovePackage { name },
                });
            }
        }
        return;
    }
    for pkg in selected {
        app.spawn_remove(pkg);
    }
    app.packages.active_tab = PackageTab::Queue;
}

pub fn filtered_installed(app: &App) -> Vec<&Package> {
    let filter = app.packages.filter.to_lowercase();
    app.packages.installed
        .iter()
        .filter(|p| filter.is_empty() || p.name.to_lowercase().contains(&filter))
        .collect()
}

fn visible_installed_count(app: &App) -> usize {
    filtered_installed(app).len()
}

// ── Processes ────────────────────────────────────────────────────────────

fn handle_processes_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down => {
            let n = app.processes.list.len();
            table_next(&mut app.processes.table_state, n);
        }
        KeyCode::Up => table_prev(&mut app.processes.table_state),
        KeyCode::Char('r') => app.spawn_load_processes(),
        KeyCode::Char('c') => {
            app.processes.sort = ProcessSort::Cpu;
            sort_processes(app);
        }
        KeyCode::Char('m') => {
            app.processes.sort = ProcessSort::Memory;
            sort_processes(app);
        }
        KeyCode::Char('p') => {
            app.processes.sort = ProcessSort::Pid;
            sort_processes(app);
        }
        KeyCode::Char('k') => {
            if let Some(idx) = app.processes.table_state.selected() {
                if let Some(proc) = app.processes.list.get(idx) {
                    let pid = proc.pid;
                    let name = proc.name.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Kill {} (pid {})? (y/N)", name, pid),
                        action: ConfirmAction::KillProcess { pid, name },
                    });
                }
            }
        }
        _ => {}
    }
}

fn sort_processes(app: &mut App) {
    match app.processes.sort {
        ProcessSort::Cpu => app.processes.list.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap_or(std::cmp::Ordering::Equal)),
        ProcessSort::Memory => app.processes.list.sort_by(|a, b| b.mem_bytes.cmp(&a.mem_bytes)),
        ProcessSort::Pid => app.processes.list.sort_by_key(|p| p.pid),
    }
}

// ── Security ─────────────────────────────────────────────────────────────

fn handle_security_key(app: &mut App, key: KeyEvent) {
    // Left/Right switch tabs
    match key.code {
        KeyCode::Right | KeyCode::Char('L') => {
            let idx = (app.security.active_tab.index() + 1) % SecurityTab::all().len();
            let tab = SecurityTab::all()[idx].clone();
            app.security.active_tab = tab.clone();
            app.spawn_load_security_tab(tab);
            return;
        }
        KeyCode::Left | KeyCode::Char('H') => {
            let idx = app.security.active_tab.index();
            let prev = if idx == 0 { SecurityTab::all().len() - 1 } else { idx - 1 };
            let tab = SecurityTab::all()[prev].clone();
            app.security.active_tab = tab.clone();
            app.spawn_load_security_tab(tab);
            return;
        }
        _ => {}
    }

    match app.security.active_tab.clone() {
        SecurityTab::Findings => handle_security_findings_key(app, key),
        SecurityTab::Firewall => handle_firewall_key(app, key),
        SecurityTab::Ports    => handle_portchecker_key(app, key),
        SecurityTab::Ssh      => handle_ssh_key(app, key),
        SecurityTab::Fail2Ban => handle_fail2ban_key(app, key),
    }
}

fn handle_security_findings_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('s') => app.spawn_security_scan(),
        KeyCode::Down => list_next(&mut app.security.list_state, app.security.findings.len()),
        KeyCode::Up => list_prev(&mut app.security.list_state),
        KeyCode::Char(' ') => {
            if let Some(idx) = app.security.list_state.selected() {
                if let Some(f) = app.security.findings.get(idx) {
                    let id = f.id.clone();
                    if app.security.selected.contains(&id) {
                        app.security.selected.remove(&id);
                    } else {
                        app.security.selected.insert(id);
                    }
                }
            }
        }
        KeyCode::Enter => {
            let selected: Vec<String> = app.security.selected.drain().collect();
            if !selected.is_empty() {
                for id in selected {
                    app.spawn_security_apply(id);
                }
            } else if let Some(idx) = app.security.list_state.selected() {
                if let Some(f) = app.security.findings.get(idx) {
                    let id = f.id.clone();
                    let title = f.title.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Apply fix: {}? (y/N)", title),
                        action: ConfirmAction::ApplySecurityFix { id, title },
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_fail2ban_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.spawn_fail2ban_list(),
        KeyCode::Down => list_next(&mut app.security.jailed_state, app.security.jailed.len()),
        KeyCode::Up => list_prev(&mut app.security.jailed_state),
        // Forgive (unban) — confirmed
        KeyCode::Char('f') => {
            if let Some(idx) = app.security.jailed_state.selected() {
                if let Some(entry) = app.security.jailed.get(idx) {
                    let ip = entry.ip.clone();
                    let jail = entry.jail.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Forgive (unban) {} from {}? (y/N)", ip, jail),
                        action: ConfirmAction::Fail2BanForgive { ip, jail },
                    });
                }
            }
        }
        // Banish (permanent block) — confirmed
        KeyCode::Char('b') => {
            if let Some(idx) = app.security.jailed_state.selected() {
                if let Some(entry) = app.security.jailed.get(idx) {
                    let ip = entry.ip.clone();
                    let jail = entry.jail.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Banish {} permanently? Adds firewall DROP rule. (y/N)", ip),
                        action: ConfirmAction::Fail2BanBanish { ip, jail },
                    });
                }
            }
        }
        _ => {}
    }
}

// ── Gateway ───────────────────────────────────────────────────────────────

fn handle_gateway_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.spawn_load_gateway(),
        KeyCode::Char('i') => app.spawn_install_caddy(),
        KeyCode::Char('a') => {
            app.gateway.input_mode = InputMode::Editing;
            app.gateway.input_domain.clear();
            app.gateway.input_port.clear();
            app.gateway.input_focus = 0;
        }
        KeyCode::Down => table_next(&mut app.gateway.table_state, app.gateway.routes.len()),
        KeyCode::Up => table_prev(&mut app.gateway.table_state),
        KeyCode::Char('D') => {
            if let Some(idx) = app.gateway.table_state.selected() {
                if let Some(route) = app.gateway.routes.get(idx) {
                    let domain = route.domain.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Delete route {}? (y/N)", domain),
                        action: ConfirmAction::DeleteRoute { domain },
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_gateway_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => app.gateway.input_mode = InputMode::Normal,
        KeyCode::Tab => app.gateway.input_focus = (app.gateway.input_focus + 1) % 2,
        KeyCode::Enter if app.gateway.input_focus == 1 => {
            let domain = app.gateway.input_domain.trim().to_string();
            let port: u16 = app.gateway.input_port.trim().parse().unwrap_or(0);
            if !domain.is_empty() && port > 0 {
                let platform = std::sync::Arc::clone(&app.platform);
                let tx = app.task_tx.clone();
                let route = Route { domain, port, tls: true };
                tokio::spawn(async move {
                    match platform.gateway.add_route(route).await {
                        Ok(_) => match platform.gateway.list_routes().await {
                            Ok(routes) => { let _ = tx.send(crate::tui::app::TaskResult::RouteList(routes)); }
                            Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                        },
                        Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                    }
                });
            }
            app.gateway.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if app.gateway.input_focus == 0 { app.gateway.input_domain.pop(); }
            else { app.gateway.input_port.pop(); }
        }
        KeyCode::Char(c) => {
            if app.gateway.input_focus == 0 { app.gateway.input_domain.push(c); }
            else if c.is_ascii_digit() { app.gateway.input_port.push(c); }
        }
        _ => {}
    }
}

// ── Services ─────────────────────────────────────────────────────────────

fn handle_services_key(app: &mut App, key: KeyEvent) {
    if app.services.filter_mode == InputMode::Editing {
        match key.code {
            KeyCode::Esc | KeyCode::Enter => app.services.filter_mode = InputMode::Normal,
            KeyCode::Backspace => { app.services.filter.pop(); }
            KeyCode::Char(c) => app.services.filter.push(c),
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Char('/') => {
            app.services.filter_mode = InputMode::Editing;
        }
        KeyCode::Char('R') => app.spawn_load_services(),
        KeyCode::Down => {
            let filter = app.services.filter.to_lowercase();
            let n = app.services.list.iter()
                .filter(|s| filter.is_empty() || s.name.to_lowercase().contains(&filter) || s.description.to_lowercase().contains(&filter))
                .count();
            table_next(&mut app.services.table_state, n);
        }
        KeyCode::Up => table_prev(&mut app.services.table_state),
        KeyCode::Char('s') => service_action(app, "start"),
        KeyCode::Char('k') => service_action(app, "stop"),
        KeyCode::Char('r') => service_action(app, "restart"),
        KeyCode::Char('e') => service_action(app, "enable"),
        KeyCode::Char('d') => service_action(app, "disable"),
        _ => {}
    }
}

fn service_action(app: &mut App, op: &'static str) {
    let filter = app.services.filter.to_lowercase();
    let visible: Vec<_> = app.services.list.iter()
        .filter(|s| filter.is_empty() || s.name.to_lowercase().contains(&filter) || s.description.to_lowercase().contains(&filter))
        .collect();

    if let Some(idx) = app.services.table_state.selected() {
        if let Some(svc) = visible.get(idx) {
            let name = svc.name.clone();
            if matches!(op, "stop" | "restart" | "disable") {
                app.confirm = Some(ConfirmDialog {
                    message: format!("{} service {}? (y/N)", op.to_uppercase(), name),
                    action: ConfirmAction::ServiceAction { name, op: op.to_string() },
                });
            } else {
                app.spawn_service_action(name, op.to_string());
            }
        }
    }
}

// ── Maintenance ──────────────────────────────────────────────────────────

fn handle_maintenance_key(app: &mut App, key: KeyEvent) {
    if app.maintenance.running_op.is_some() {
        return; // Block input while running
    }

    match key.code {
        KeyCode::Char('c') => {
            app.confirm = Some(ConfirmDialog {
                message: "Clean all package manager caches? (y/N)".to_string(),
                action: ConfirmAction::MaintenanceAction { op: "clean_pkg_cache".to_string() },
            });
        }
        _ => {}
    }
}

// ── Tunnel ────────────────────────────────────────────────────────────────

fn handle_tunnel_key(app: &mut App, key: KeyEvent) {
    // [f] toggles focus between Tunnels panel (left) and Ingress panel (right)
    if key.code == KeyCode::Char('f') {
        app.tunnel.panel_focus = match app.tunnel.panel_focus {
            TunnelPanel::Tunnels => TunnelPanel::Ingress,
            TunnelPanel::Ingress => TunnelPanel::Tunnels,
        };
        return;
    }

    // Ingress-panel-specific keys when the right panel has focus
    if app.tunnel.panel_focus == TunnelPanel::Ingress {
        match key.code {
            KeyCode::Down => {
                list_next(&mut app.tunnel.ingress_state, app.tunnel.ingress_entries.len());
                return;
            }
            KeyCode::Up => {
                list_prev(&mut app.tunnel.ingress_state);
                return;
            }
            // [e] edit selected ingress entry
            KeyCode::Char('e') => {
                if let Some(idx) = app.tunnel.ingress_state.selected() {
                    if let Some((host, svc)) = app.tunnel.ingress_entries.get(idx) {
                        app.tunnel.input_original_host = host.clone();
                        app.tunnel.input_host = host.clone();
                        app.tunnel.input_service = svc.clone();
                        app.tunnel.input_focus = 0;
                        app.tunnel.input_mode = InputMode::EditingIngress;
                    }
                }
                return;
            }
            // [D] delete selected ingress entry (with confirm dialog)
            KeyCode::Char('D') => {
                if let Some(idx) = app.tunnel.ingress_state.selected() {
                    if let Some((host, _)) = app.tunnel.ingress_entries.get(idx) {
                        if let Some(tid) = app.tunnel.active_tunnel_id.clone() {
                            let hostname = host.clone();
                            app.confirm = Some(ConfirmDialog {
                                message: format!("Remove ingress {}? (y/N)", hostname),
                                action: ConfirmAction::DeleteIngress { tunnel_id: tid, hostname },
                            });
                        } else {
                            app.status_msg = Some("Select a tunnel first (Enter)".to_string());
                        }
                    }
                }
                return;
            }
            // [Esc] return focus to tunnel list
            KeyCode::Esc => {
                app.tunnel.panel_focus = TunnelPanel::Tunnels;
                return;
            }
            _ => {}
        }
        // Fall through to shared keys (service controls etc.) when no ingress key matched
    }

    match key.code {
        KeyCode::Char('r') => app.spawn_load_tunnels(),
        KeyCode::Char('i') => app.spawn_install_cloudflared(),
        KeyCode::Char('l') => {
            app.needs_login = true;
        }
        KeyCode::Char('a') => {
            app.tunnel.input_mode = InputMode::Editing;
            app.tunnel.input_name.clear();
            app.tunnel.input_host.clear();
            app.tunnel.input_service.clear();
            app.tunnel.input_focus = 0;
        }
        KeyCode::Char('d') => {
            if app.tunnel.table_state.selected().is_some() {
                app.tunnel.input_mode = InputMode::AddingDomain;
                app.tunnel.input_host.clear();
                app.tunnel.input_service.clear();
                app.tunnel.input_focus = 0;
            }
        }
        KeyCode::Down => table_next(&mut app.tunnel.table_state, app.tunnel.tunnels.len()),
        KeyCode::Up => table_prev(&mut app.tunnel.table_state),
        // [Enter] select this tunnel as active (config points to it)
        KeyCode::Enter => {
            if let Some(idx) = app.tunnel.table_state.selected() {
                if let Some(t) = app.tunnel.tunnels.get(idx) {
                    let id = t.id.clone();
                    let name = t.name.clone();
                    app.tunnel.active_tunnel_id = Some(id.clone());
                    app.tunnel.ingress_entries.clear();
                    app.tunnel.ingress_state.select(None);
                    app.status_msg = Some(format!("Active tunnel: {}", name));
                    let platform = std::sync::Arc::clone(&app.platform);
                    let tx = app.task_tx.clone();
                    tokio::spawn(async move {
                        match platform.tunnel.config_content(&id).await {
                            Ok(c) => { let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(c)); }
                            Err(_) => { let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(String::new())); }
                        }
                        if let Ok((active, enabled)) = platform.tunnel.service_status().await {
                            let _ = tx.send(crate::tui::app::TaskResult::TunnelServiceStatus { active, enabled });
                        }
                    });
                }
            }
        }
        // [c] reload config content + service status for active tunnel
        KeyCode::Char('c') => {
            let id = app.tunnel.active_tunnel_id.clone();
            app.spawn_tunnel_extras(id);
        }
        // [s] install service pointing at the active tunnel's per-tunnel config
        KeyCode::Char('s') => {
            let tunnel_id = app.tunnel.active_tunnel_id.clone()
                .or_else(|| app.tunnel.table_state.selected()
                    .and_then(|i| app.tunnel.tunnels.get(i))
                    .map(|t| t.id.clone()));
            if let Some(tid) = tunnel_id {
                let platform = std::sync::Arc::clone(&app.platform);
                let tx = app.task_tx.clone();
                tokio::spawn(async move {
                    match platform.tunnel.install_service(&tid).await {
                        Ok(_) => { let _ = tx.send(crate::tui::app::TaskResult::Status("Service installed".to_string())); }
                        Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                    }
                });
            } else {
                app.status_msg = Some("Select a tunnel first (Enter)".to_string());
            }
        }
        // [u] sync config → /etc/cloudflared + restart (use after editing config)
        KeyCode::Char('u') => {
            app.status_msg = Some("Syncing config…".to_string());
            spawn_service_action(app, |p| Box::pin(async move { p.tunnel.sync_config().await }), "Config synced & service restarted");
        }
        // [T] start  [X] stop  [R] restart
        KeyCode::Char('T') => {
            app.status_msg = Some("Starting cloudflared…".to_string());
            spawn_service_action(app, |p| Box::pin(async move { p.tunnel.service_start().await }), "Service started");
        }
        KeyCode::Char('X') => {
            app.status_msg = Some("Stopping cloudflared…".to_string());
            spawn_service_action(app, |p| Box::pin(async move { p.tunnel.service_stop().await }), "Service stopped");
        }
        KeyCode::Char('R') => {
            app.status_msg = Some("Restarting cloudflared…".to_string());
            spawn_service_action(app, |p| Box::pin(async move { p.tunnel.service_restart().await }), "Service restarted");
        }
        _ => {}
    }
}

fn spawn_service_action<F>(app: &mut App, f: F, ok_msg: &'static str)
where
    F: FnOnce(std::sync::Arc<crate::core::Platform>) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<()>> + Send>> + Send + 'static,
{
    let platform = std::sync::Arc::clone(&app.platform);
    let tx = app.task_tx.clone();
    tokio::spawn(async move {
        match f(platform.clone()).await {
            Ok(_) => {
                let _ = tx.send(crate::tui::app::TaskResult::Status(ok_msg.to_string()));
                // Refresh service status after action
                if let Ok((active, enabled)) = platform.tunnel.service_status().await {
                    let _ = tx.send(crate::tui::app::TaskResult::TunnelServiceStatus { active, enabled });
                }
            }
            Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
        }
    });
}

/// Normalise a user-supplied service string to a full URL.
///   "9999"           → "http://localhost:9999"
///   "8080"           → "http://localhost:8080"
///   "localhost:3000" → "http://localhost:3000"
///   "http://…"       → unchanged
fn normalize_service(s: &str) -> String {
    let s = s.trim();
    if s.is_empty() {
        return "http://localhost:3000".to_string();
    }
    if s.starts_with("http://") || s.starts_with("https://") || s.starts_with("tcp://") || s.starts_with("ssh://") {
        return s.to_string();
    }
    // bare port number
    if s.chars().all(|c| c.is_ascii_digit()) {
        return format!("http://localhost:{}", s);
    }
    // host:port without scheme
    format!("http://{}", s)
}

fn handle_tunnel_input(app: &mut App, key: KeyEvent) {
    let editing_ingress = app.tunnel.input_mode == InputMode::EditingIngress;
    let adding_domain = app.tunnel.input_mode == InputMode::AddingDomain;
    // EditingIngress has 2 fields (hostname, service) like AddingDomain
    let tab_fields = if adding_domain || editing_ingress { 2 } else { 3 };
    let submit_focus = tab_fields - 1;

    match key.code {
        KeyCode::Esc => app.tunnel.input_mode = InputMode::Normal,
        KeyCode::Tab => app.tunnel.input_focus = (app.tunnel.input_focus + 1) % tab_fields,
        KeyCode::Enter if app.tunnel.input_focus == submit_focus => {
            if editing_ingress {
                // Update ingress: remove old hostname entry, add updated one
                let original_host = app.tunnel.input_original_host.clone();
                let new_host = app.tunnel.input_host.trim().to_string();
                let service = app.tunnel.input_service.trim().to_string();
                if let Some(tid) = app.tunnel.active_tunnel_id.clone() {
                    if !new_host.is_empty() {
                        let platform = std::sync::Arc::clone(&app.platform);
                        let tx = app.task_tx.clone();
                        tokio::spawn(async move {
                            // Remove old entry first (no-op if hostname unchanged, add_domain_to_config handles update)
                            if original_host != new_host {
                                let _ = platform.tunnel.remove_ingress(&tid, &original_host).await;
                            }
                            let route = TunnelRoute {
                                tunnel_id: tid.clone(),
                                tunnel_name: String::new(),
                                hostname: new_host,
                                service: normalize_service(&service),
                            };
                            match platform.tunnel.add_domain_to_config(route).await {
                                Ok(_) => {
                                    if let Ok(c) = platform.tunnel.config_content(&tid).await {
                                        let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(c));
                                    }
                                    let _ = tx.send(crate::tui::app::TaskResult::Status("Ingress entry updated".to_string()));
                                }
                                Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                            }
                        });
                    }
                }
                app.tunnel.input_mode = InputMode::Normal;
                return;
            }
            if adding_domain {
                // Add domain to selected existing tunnel (config + DNS)
                let selected = app.tunnel.table_state.selected()
                    .and_then(|i| app.tunnel.tunnels.get(i))
                    .cloned();
                let host = app.tunnel.input_host.trim().to_string();
                let service = app.tunnel.input_service.trim().to_string();
                if let Some(tunnel) = selected {
                    if !host.is_empty() {
                        let platform = std::sync::Arc::clone(&app.platform);
                        let tx = app.task_tx.clone();
                        let tid = tunnel.id.clone();
                        tokio::spawn(async move {
                            let route = TunnelRoute {
                                tunnel_id: tunnel.id,
                                tunnel_name: tunnel.name,
                                hostname: host,
                                service: normalize_service(&service),
                            };
                            match platform.tunnel.add_route(route).await {
                                Ok(_) => {
                                    if let Ok(c) = platform.tunnel.config_content(&tid).await {
                                        let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(c));
                                    }
                                    let (active, enabled) = platform.tunnel.service_status().await.unwrap_or((false, false));
                                    let _ = tx.send(crate::tui::app::TaskResult::TunnelServiceStatus { active, enabled });
                                    let msg = if active { "Domain added" } else { "Domain added — press [s] to install service, [T] to start" };
                                    let _ = tx.send(crate::tui::app::TaskResult::Status(msg.to_string()));
                                }
                                Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                            }
                        });
                    }
                }
            } else {
                // Create new tunnel (+ optional route)
                let name = app.tunnel.input_name.trim().to_string();
                let host = app.tunnel.input_host.trim().to_string();
                let service = app.tunnel.input_service.trim().to_string();
                if !name.is_empty() {
                    let platform = std::sync::Arc::clone(&app.platform);
                    let tx = app.task_tx.clone();
                    tokio::spawn(async move {
                        match platform.tunnel.create(&name).await {
                            Ok(t) => {
                                if !host.is_empty() {
                                    let tid = t.id.clone();
                                    let route = TunnelRoute {
                                        tunnel_id: t.id.clone(),
                                        tunnel_name: t.name.clone(),
                                        hostname: host,
                                        service: normalize_service(&service),
                                    };
                                    match platform.tunnel.add_route(route).await {
                                        Ok(_) => {
                                            if let Ok(c) = platform.tunnel.config_content(&tid).await {
                                                let _ = tx.send(crate::tui::app::TaskResult::TunnelConfigContent(c));
                                            }
                                            let (active, enabled) = platform.tunnel.service_status().await.unwrap_or((false, false));
                                            let _ = tx.send(crate::tui::app::TaskResult::TunnelServiceStatus { active, enabled });
                                        }
                                        Err(e) => {
                                            let _ = tx.send(crate::tui::app::TaskResult::Error(
                                                format!("Tunnel created but route failed: {}", e)
                                            ));
                                        }
                                    }
                                }
                                let _ = tx.send(crate::tui::app::TaskResult::TunnelCreated(t));
                            }
                            Err(e) => { let _ = tx.send(crate::tui::app::TaskResult::Error(e.to_string())); }
                        }
                    });
                }
            }
            app.tunnel.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if adding_domain || editing_ingress {
                match app.tunnel.input_focus {
                    0 => { app.tunnel.input_host.pop(); }
                    _ => { app.tunnel.input_service.pop(); }
                }
            } else {
                match app.tunnel.input_focus {
                    0 => { app.tunnel.input_name.pop(); }
                    1 => { app.tunnel.input_host.pop(); }
                    _ => { app.tunnel.input_service.pop(); }
                }
            }
        }
        KeyCode::Char(c) => {
            if adding_domain || editing_ingress {
                match app.tunnel.input_focus {
                    0 => app.tunnel.input_host.push(c),
                    _ => app.tunnel.input_service.push(c),
                }
            } else {
                match app.tunnel.input_focus {
                    0 => app.tunnel.input_name.push(c),
                    1 => app.tunnel.input_host.push(c),
                    _ => app.tunnel.input_service.push(c),
                }
            }
        }
        _ => {}
    }
}

// ── List / Table navigation helpers ──────────────────────────────────────

fn list_next(state: &mut ratatui::widgets::ListState, len: usize) {
    if len == 0 { return; }
    let next = state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
    state.select(Some(next));
}

fn list_prev(state: &mut ratatui::widgets::ListState) {
    let prev = state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
    state.select(Some(prev));
}

fn table_next(state: &mut ratatui::widgets::TableState, len: usize) {
    if len == 0 { return; }
    let next = state.selected().map(|i| (i + 1).min(len - 1)).unwrap_or(0);
    state.select(Some(next));
}

fn table_prev(state: &mut ratatui::widgets::TableState) {
    let prev = state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
    state.select(Some(prev));
}

use crate::core::models::Package;

// ── Firewall ──────────────────────────────────────────────────────────────

fn handle_firewall_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.spawn_load_firewall(),
        KeyCode::Char('e') => {
            // Enable firewall
            if app.firewall.enabled != Some(true) {
                app.spawn_firewall_set_enabled(true);
            }
        }
        KeyCode::Char('d') => {
            // Disable firewall
            if app.firewall.enabled == Some(true) {
                app.spawn_firewall_set_enabled(false);
            }
        }
        KeyCode::Char('a') => {
            // Open add-rule popup
            app.firewall.input_mode = InputMode::Editing;
            app.firewall.input_focus = 0;
            app.firewall.input_port.clear();
            app.firewall.input_proto = 0;
            app.firewall.input_from.clear();
            app.firewall.input_action = 0;
        }
        KeyCode::Down => table_next(&mut app.firewall.table_state, app.firewall.rules.len()),
        KeyCode::Up => table_prev(&mut app.firewall.table_state),
        KeyCode::Char('D') => {
            if let Some(idx) = app.firewall.table_state.selected() {
                if let Some(rule) = app.firewall.rules.get(idx) {
                    let num = rule.num;
                    let desc = format!("rule {} ({} {})", num, rule.action, rule.to);
                    app.confirm = Some(ConfirmDialog {
                        message: format!("Delete {}? (y/N)", desc),
                        action: ConfirmAction::DeleteFirewallRule { num },
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_firewall_input(app: &mut App, key: KeyEvent) {
    const NUM_FIELDS: usize = 4; // port, proto, from, action

    match key.code {
        KeyCode::Esc => {
            app.firewall.input_mode = InputMode::Normal;
        }
        KeyCode::Tab => {
            app.firewall.input_focus = (app.firewall.input_focus + 1) % NUM_FIELDS;
        }
        KeyCode::Enter => {
            if app.firewall.input_focus == NUM_FIELDS - 1 {
                // Submit
                let port = app.firewall.input_port.trim().to_string();
                let proto = PROTOS[app.firewall.input_proto].to_string();
                let from = app.firewall.input_from.trim().to_string();
                let action = ACTIONS[app.firewall.input_action].to_string();
                if !port.is_empty() {
                    app.spawn_firewall_add_rule(port, proto, from, action);
                } else {
                    app.status_msg = Some("Port is required".to_string());
                }
                app.firewall.input_mode = InputMode::Normal;
            } else {
                // Advance to next field
                app.firewall.input_focus = (app.firewall.input_focus + 1) % NUM_FIELDS;
            }
        }
        KeyCode::Char(' ') | KeyCode::Right => {
            // Cycle selector fields
            match app.firewall.input_focus {
                1 => app.firewall.input_proto = (app.firewall.input_proto + 1) % PROTOS.len(),
                3 => app.firewall.input_action = (app.firewall.input_action + 1) % ACTIONS.len(),
                _ => {}
            }
        }
        KeyCode::Left => {
            // Cycle selector fields backwards
            match app.firewall.input_focus {
                1 => {
                    app.firewall.input_proto = if app.firewall.input_proto == 0 {
                        PROTOS.len() - 1
                    } else {
                        app.firewall.input_proto - 1
                    };
                }
                3 => {
                    app.firewall.input_action = if app.firewall.input_action == 0 {
                        ACTIONS.len() - 1
                    } else {
                        app.firewall.input_action - 1
                    };
                }
                _ => {}
            }
        }
        KeyCode::Backspace => match app.firewall.input_focus {
            0 => { app.firewall.input_port.pop(); }
            2 => { app.firewall.input_from.pop(); }
            _ => {}
        },
        KeyCode::Char(c) => match app.firewall.input_focus {
            0 => {
                // Only digits and range syntax
                if c.is_ascii_digit() || c == ':' {
                    app.firewall.input_port.push(c);
                }
            }
            2 => app.firewall.input_from.push(c),
            _ => {}
        },
        _ => {}
    }
}

// ── Docker ────────────────────────────────────────────────────────────────

fn handle_docker_key(app: &mut App, key: KeyEvent) {
    use crate::tui::app::DockerTab;

    // Tab switching
    match key.code {
        KeyCode::Right | KeyCode::Char('l') => {
            let idx = (app.docker.active_tab.index() + 1) % DockerTab::all().len();
            app.docker.active_tab = DockerTab::all()[idx].clone();
            if app.docker.active_tab == DockerTab::Compose {
                app.spawn_load_compose();
            }
            return;
        }
        KeyCode::Left | KeyCode::Char('h') => {
            let idx = app.docker.active_tab.index();
            let prev = if idx == 0 { DockerTab::all().len() - 1 } else { idx - 1 };
            app.docker.active_tab = DockerTab::all()[prev].clone();
            if app.docker.active_tab == DockerTab::Compose {
                app.spawn_load_compose();
            }
            return;
        }
        _ => {}
    }

    match app.docker.active_tab.clone() {
        DockerTab::Containers => handle_docker_containers_key(app, key),
        DockerTab::Images     => handle_docker_images_key(app, key),
        DockerTab::Compose    => handle_docker_compose_key(app, key),
    }
}

fn handle_docker_containers_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down => table_next(&mut app.docker.containers_state, app.docker.containers.len()),
        KeyCode::Up   => table_prev(&mut app.docker.containers_state),
        KeyCode::Char('r') => app.spawn_load_docker(),
        KeyCode::Char('s') => {
            if let Some(idx) = app.docker.containers_state.selected() {
                if let Some(c) = app.docker.containers.get(idx) {
                    app.spawn_docker_container_action("start", c.id.clone());
                }
            }
        }
        KeyCode::Char('x') => {
            if let Some(idx) = app.docker.containers_state.selected() {
                if let Some(c) = app.docker.containers.get(idx) {
                    let id = c.id.clone();
                    let name = c.name.clone();
                    app.confirm = Some(crate::tui::app::ConfirmDialog {
                        message: format!("Stop container {}? (y/N)", name),
                        action: crate::tui::app::ConfirmAction::StopContainer { id, name },
                    });
                }
            }
        }
        KeyCode::Char('R') => {
            if let Some(idx) = app.docker.containers_state.selected() {
                if let Some(c) = app.docker.containers.get(idx) {
                    app.spawn_docker_container_action("restart", c.id.clone());
                }
            }
        }
        KeyCode::Char('D') => {
            if let Some(idx) = app.docker.containers_state.selected() {
                if let Some(c) = app.docker.containers.get(idx) {
                    let id = c.id.clone();
                    let name = c.name.clone();
                    app.confirm = Some(crate::tui::app::ConfirmDialog {
                        message: format!("Remove container {}? (y/N)", name),
                        action: crate::tui::app::ConfirmAction::RemoveContainer { id, name },
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_docker_images_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down      => table_next(&mut app.docker.images_state, app.docker.images.len()),
        KeyCode::Up        => table_prev(&mut app.docker.images_state),
        KeyCode::Char('r') => app.spawn_load_docker(),
        KeyCode::Char('D') => {
            if let Some(idx) = app.docker.images_state.selected() {
                if let Some(img) = app.docker.images.get(idx) {
                    let id = img.id.clone();
                    let tag = format!("{}:{}", img.repository, img.tag);
                    app.confirm = Some(crate::tui::app::ConfirmDialog {
                        message: format!("Remove image {}? (y/N)", tag),
                        action: crate::tui::app::ConfirmAction::RemoveImage { id, tag },
                    });
                }
            }
        }
        _ => {}
    }
}

fn handle_docker_compose_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Down      => table_next(&mut app.docker.compose_state, app.docker.compose_services.len()),
        KeyCode::Up        => table_prev(&mut app.docker.compose_state),
        KeyCode::Char('r') => app.spawn_load_compose(),
        KeyCode::Char('u') => app.spawn_compose_action("up"),
        KeyCode::Char('d') => app.spawn_compose_action("down"),
        KeyCode::Char('R') => app.spawn_compose_action("restart"),
        _ => {}
    }
}

// ── Port Checker ──────────────────────────────────────────────────────────

fn handle_portchecker_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => {
            app.portchecker.public_ip = None;
            app.spawn_fetch_public_ip();
        }
        KeyCode::Char('c') => {
            if !app.portchecker.checking {
                app.spawn_check_ports();
            }
        }
        KeyCode::Char('a') => {
            app.portchecker.input_mode = InputMode::Editing;
            app.portchecker.input_port.clear();
            app.portchecker.input_label.clear();
            app.portchecker.input_focus = 0;
        }
        KeyCode::Char('d') => {
            if let Some(idx) = app.portchecker.list_state.selected() {
                if idx < app.portchecker.entries.len() {
                    app.portchecker.entries.remove(idx);
                    // clamp cursor
                    let new_len = app.portchecker.entries.len();
                    if new_len == 0 {
                        app.portchecker.list_state.select(None);
                    } else {
                        app.portchecker.list_state.select(Some(idx.min(new_len - 1)));
                    }
                }
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            let len = app.portchecker.entries.len();
            if len > 0 {
                let next = match app.portchecker.list_state.selected() {
                    Some(i) => (i + 1).min(len - 1),
                    None    => 0,
                };
                app.portchecker.list_state.select(Some(next));
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if let Some(i) = app.portchecker.list_state.selected() {
                if i > 0 {
                    app.portchecker.list_state.select(Some(i - 1));
                }
            }
        }
        _ => {}
    }
}

/// Handles typing inside the "add port" popup.
fn handle_portchecker_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.portchecker.input_mode = InputMode::Normal;
        }
        KeyCode::Tab => {
            // cycle focus: 0 (port) → 1 (label) → 0
            app.portchecker.input_focus = (app.portchecker.input_focus + 1) % 2;
        }
        KeyCode::Enter => {
            // Parse port number and commit
            if let Ok(port) = app.portchecker.input_port.trim().parse::<u16>() {
                if port > 0 {
                    let label = if app.portchecker.input_label.trim().is_empty() {
                        format!("Port {}", port)
                    } else {
                        app.portchecker.input_label.trim().to_string()
                    };
                    // Avoid duplicates
                    if !app.portchecker.entries.iter().any(|e| e.port == port) {
                        use crate::core::portcheck::PortEntry;
                        app.portchecker.entries.push(PortEntry::new(port, label));
                        let last = app.portchecker.entries.len() - 1;
                        app.portchecker.list_state.select(Some(last));
                    }
                }
            }
            app.portchecker.input_mode = InputMode::Normal;
        }
        KeyCode::Backspace => {
            if app.portchecker.input_focus == 0 {
                app.portchecker.input_port.pop();
            } else {
                app.portchecker.input_label.pop();
            }
        }
        KeyCode::Char(c) => {
            if app.portchecker.input_focus == 0 {
                if c.is_ascii_digit() {
                    app.portchecker.input_port.push(c);
                }
            } else {
                app.portchecker.input_label.push(c);
            }
        }
        _ => {}
    }
}

// ── Ghost Services Hunter ─────────────────────────────────────────────────

fn handle_ghost_key(app: &mut App, key: KeyEvent) {
    match key.code {
        // Rescan
        KeyCode::Char('r') => {
            if !app.ghost.scanning {
                app.spawn_ghost_scan();
            }
        }
        // Navigate (arrow keys only — 'k' is reserved for kill)
        KeyCode::Down => {
            let n = app.ghost.ghosts.len();
            if n > 0 {
                let next = app.ghost.table_state.selected()
                    .map(|i| (i + 1).min(n - 1))
                    .unwrap_or(0);
                app.ghost.table_state.select(Some(next));
            }
        }
        KeyCode::Up => {
            let prev = app.ghost.table_state.selected()
                .map(|i| i.saturating_sub(1))
                .unwrap_or(0);
            app.ghost.table_state.select(Some(prev));
        }
        // Kill selected ghost process (with confirmation)
        KeyCode::Char('k') => {
            if let Some(idx) = app.ghost.table_state.selected() {
                if let Some(g) = app.ghost.ghosts.get(idx) {
                    let pid = g.pid;
                    let name = g.name.clone();
                    app.confirm = Some(ConfirmDialog {
                        message: format!(
                            "Kill {} (PID {}, {})? (y/N)",
                            name, pid, g.reason.label()
                        ),
                        action: ConfirmAction::KillGhost { pid, name },
                    });
                }
            }
        }
        _ => {}
    }
}

// ── SSH ───────────────────────────────────────────────────────────────────

fn handle_ssh_key(app: &mut App, key: KeyEvent) {
    use super::app::InputMode;
    match key.code {
        KeyCode::Char('r') => app.spawn_load_ssh(),
        KeyCode::Char('g') => {
            app.ssh.input_mode = InputMode::Editing;
            app.ssh.input_name.clear();
            app.ssh.input_type = "ed25519".to_string();
        }
        KeyCode::Left => app.ssh.focus = 0,
        KeyCode::Right => app.ssh.focus = 1,
        KeyCode::Char('a') => {
            if app.ssh.focus == 0 {
                if let Some(idx) = app.ssh.local_state.selected() {
                    if let Some(k) = app.ssh.local_keys.get(idx) {
                        let content = k.content.clone();
                        let name = k.name.clone();
                        app.confirm = Some(ConfirmDialog {
                            message: format!("Authorize key '{}'? (y/N)", name),
                            action: ConfirmAction::AuthorizeLocalKey { content, name },
                        });
                    }
                }
            }
        }
        KeyCode::Char('D') => {
            if app.ssh.focus == 1 {
                if let Some(idx) = app.ssh.authorized_state.selected() {
                    if let Some(k) = app.ssh.authorized_keys.get(idx) {
                        let fingerprint = k.fingerprint.clone();
                        let name = k.name.clone();
                        app.confirm = Some(ConfirmDialog {
                            message: format!("Remove authorized key '{}'? (y/N)", name),
                            action: ConfirmAction::DeauthorizeKey { fingerprint, name },
                        });
                    }
                }
            }
        }
        KeyCode::Down => {
            if app.ssh.focus == 0 {
                let n = app.ssh.local_keys.len();
                if n > 0 {
                    let next = app.ssh.local_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                    app.ssh.local_state.select(Some(next));
                }
            } else {
                let n = app.ssh.authorized_keys.len();
                if n > 0 {
                    let next = app.ssh.authorized_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                    app.ssh.authorized_state.select(Some(next));
                }
            }
        }
        KeyCode::Up => {
            if app.ssh.focus == 0 {
                let prev = app.ssh.local_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                app.ssh.local_state.select(Some(prev));
            } else {
                let prev = app.ssh.authorized_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                app.ssh.authorized_state.select(Some(prev));
            }
        }
        _ => {}
    }
}

fn handle_ssh_input(app: &mut App, key: KeyEvent) {
    use super::app::InputMode;
    match key.code {
        KeyCode::Esc => {
            app.ssh.input_mode = InputMode::Normal;
            app.ssh.input_name.clear();
        }
        KeyCode::Enter => {
            if !app.ssh.input_name.trim().is_empty() {
                let name = app.ssh.input_name.trim().to_string();
                let key_type = app.ssh.input_type.clone();
                app.spawn_generate_key(name, key_type);
                app.ssh.input_mode = InputMode::Normal;
                app.ssh.input_name.clear();
            }
        }
        KeyCode::Char(c) => {
            app.ssh.input_name.push(c);
        }
        KeyCode::Backspace => {
            app.ssh.input_name.pop();
        }
        KeyCode::Tab => {
            // Toggle key type between ed25519 and rsa
            app.ssh.input_type = if app.ssh.input_type == "ed25519" {
                "rsa".to_string()
            } else {
                "ed25519".to_string()
            };
        }
        _ => {}
    }
}

// ── WasmCloud ─────────────────────────────────────────────────────────────

fn handle_users_key(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('r') => app.spawn_load_users(),
        KeyCode::Down => {
            let n = app.users.users.len();
            table_next(&mut app.users.table_state, n);
        }
        KeyCode::Up => table_prev(&mut app.users.table_state),
        KeyCode::Char('d') => {
            if let Some(idx) = app.users.table_state.selected() {
                if let Some(u) = app.users.users.get(idx) {
                    let username = u.username.clone();
                    app.spawn_user_action("delete".to_string(), username);
                }
            }
        }
        _ => {}
    }
}

fn handle_wasm_cloud_key(app: &mut App, key: KeyEvent) {
    use super::app::WasmCloudTab;

    // NATS provisioning is available regardless of wash install state
    if key.code == KeyCode::Char('n') {
        app.spawn_nats_provision();
        return;
    }
    if key.code == KeyCode::Char('N') {
        app.spawn_poll_nats_status();
        return;
    }

    if !app.wasm_cloud.installed {
        if key.code == KeyCode::Char('i') {
            app.spawn_install_wash();
        }
        return;
    }

    match key.code {
        KeyCode::Char('r') => app.spawn_load_wasm_cloud(),
        KeyCode::Left => {
            let idx = app.wasm_cloud.active_tab.index();
            let all = WasmCloudTab::all();
            if idx > 0 {
                app.wasm_cloud.active_tab = all[idx - 1].clone();
            }
        }
        KeyCode::Right => {
            let idx = app.wasm_cloud.active_tab.index();
            let all = WasmCloudTab::all();
            if idx + 1 < all.len() {
                app.wasm_cloud.active_tab = all[idx + 1].clone();
            }
        }
        KeyCode::Down => match app.wasm_cloud.active_tab {
            WasmCloudTab::Hosts => {
                let n = app.wasm_cloud.hosts.len();
                if n > 0 {
                    let next = app.wasm_cloud.hosts_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                    app.wasm_cloud.hosts_state.select(Some(next));
                }
            }
            WasmCloudTab::Components => {
                let n = app.wasm_cloud.components.len();
                if n > 0 {
                    let next = app.wasm_cloud.components_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                    app.wasm_cloud.components_state.select(Some(next));
                }
            }
            WasmCloudTab::Apps => {
                let n = app.wasm_cloud.apps.len();
                if n > 0 {
                    let next = app.wasm_cloud.apps_state.selected().map(|i| (i + 1).min(n - 1)).unwrap_or(0);
                    app.wasm_cloud.apps_state.select(Some(next));
                }
            }
            WasmCloudTab::Inspector => {}
        },
        KeyCode::Up => match app.wasm_cloud.active_tab {
            WasmCloudTab::Hosts => {
                let prev = app.wasm_cloud.hosts_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                app.wasm_cloud.hosts_state.select(Some(prev));
            }
            WasmCloudTab::Components => {
                let prev = app.wasm_cloud.components_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                app.wasm_cloud.components_state.select(Some(prev));
            }
            WasmCloudTab::Apps => {
                let prev = app.wasm_cloud.apps_state.selected().map(|i| i.saturating_sub(1)).unwrap_or(0);
                app.wasm_cloud.apps_state.select(Some(prev));
            }
            WasmCloudTab::Inspector => {}
        },
        KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char('e') if app.wasm_cloud.active_tab == WasmCloudTab::Inspector => {
            app.wasm_cloud.input_mode = InputMode::Editing;
        }
        _ => {}
    }
}

fn handle_wasm_cloud_inspector_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if !app.wasm_cloud.inspect_target.trim().is_empty() {
                let target = app.wasm_cloud.inspect_target.trim().to_string();
                app.wasm_cloud.inspect_output = Some("Inspecting...".to_string());
                app.spawn_inspect_component(target);
            }
            app.wasm_cloud.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => {
            app.wasm_cloud.inspect_target.push(c);
        }
        KeyCode::Backspace => {
            app.wasm_cloud.inspect_target.pop();
        }
        KeyCode::Esc => {
            app.wasm_cloud.input_mode = InputMode::Normal;
        }
        _ => {}
    }
}
