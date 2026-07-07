mod app;
mod theme;
mod ui;

pub use app::App;

use std::time::{Duration, Instant};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::config::HuayuConfig;

const TICK: Duration = Duration::from_millis(100);

pub fn run(config: HuayuConfig) -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_loop(&mut terminal, config);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    config: HuayuConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(config);

    // Always rewrite tool configs on startup to pick up any format changes.
    if !app.config.api_key.is_empty() {
        let _ = crate::config::write_codex_config(&app.config);
        let _ = crate::config::write_claude_config(&app.config);
    }

    // Open login overlay immediately if not yet logged in
    if app.config.api_key.is_empty() {
        app.open_login_overlay();
    }

    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| ui::render(f, &app))?;

        // Viewport height for Page Up/Down: terminal height minus fixed chrome rows.
        // Chrome: 1 status + 2 main borders + 4 input + 1 hints = 8
        let viewport_h = terminal
            .size()
            .map(|s| s.height.saturating_sub(8) as usize)
            .unwrap_or(20)
            .max(1);

        let timeout = TICK.saturating_sub(last_tick.elapsed());

        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(&mut app, key.code, key.modifiers, viewport_h);
                }
                Event::Mouse(mouse) => {
                    handle_mouse(&mut app, mouse.kind);
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= TICK {
            app.drain_tool_events();
            app.drain_update();
            app.poll_login();
            last_tick = Instant::now();
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}

fn handle_mouse(app: &mut App, kind: MouseEventKind) {
    match kind {
        MouseEventKind::ScrollUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(3);
            app.auto_scroll = false;
        }
        MouseEventKind::ScrollDown => {
            if app.scroll_offset > 3 {
                app.scroll_offset -= 3;
            } else {
                app.scroll_offset = 0;
                app.auto_scroll = true;
            }
        }
        _ => {}
    }
}

fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers, viewport_h: usize) {
    // Alt+Q quits from any state
    if code == KeyCode::Char('q') && mods.contains(KeyModifiers::ALT) {
        app.should_quit = true;
        return;
    }

    // ── Login overlay keys ────────────────────────────────────────────────
    if app.login_overlay.is_some() {
        match code {
            KeyCode::Esc => {
                app.login_overlay = None;
            }
            KeyCode::Char('r') => {
                app.open_login_overlay();
            }
            _ => {}
        }
        return;
    }

    // ── Settings overlay keys ─────────────────────────────────────────────
    if app.show_settings {
        match code {
            KeyCode::Esc => app.show_settings = false,
            KeyCode::Enter => app.apply_settings(),
            KeyCode::Tab => {
                app.settings_focus_field = 1 - app.settings_focus_field;
            }
            KeyCode::Backspace => {
                if app.settings_focus_field == 0 {
                    app.settings_model_input.pop();
                } else {
                    app.settings_url_input.pop();
                }
            }
            KeyCode::Char(c) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                if app.settings_focus_field == 0 {
                    app.settings_model_input.push(c);
                } else {
                    app.settings_url_input.push(c);
                }
            }
            _ => {}
        }
        return;
    }

    // ── Main view keys ────────────────────────────────────────────────────
    match code {
        KeyCode::Enter => {
            if mods.contains(KeyModifiers::SHIFT) {
                app.input_newline();
            } else {
                app.submit();
            }
        }

        KeyCode::Esc => {
            if let Some(proc) = &mut app.tool_process {
                proc.kill();
                app.push_progress("[Esc] 任务已取消");
                app.tool_process = None;
                app.task_start = None;
            }
        }

        KeyCode::Backspace => app.input_backspace(),

        // Input history navigation (always, regardless of input content)
        KeyCode::Up => app.history_up(),
        KeyCode::Down => app.history_down(),

        // Main panel scrolling
        KeyCode::PageUp => {
            app.scroll_offset = app.scroll_offset.saturating_add(viewport_h);
            app.auto_scroll = false;
        }
        KeyCode::PageDown => {
            app.scroll_offset = app.scroll_offset.saturating_sub(viewport_h);
            if app.scroll_offset == 0 {
                app.auto_scroll = true;
            }
        }

        // Tab cycles tools when no overlay is open
        KeyCode::Tab if mods == KeyModifiers::NONE => {
            let next = match app.tool_type {
                crate::tool::ToolType::Codex => crate::tool::ToolType::Claude,
                crate::tool::ToolType::Claude => crate::tool::ToolType::Codex,
            };
            let name = next.as_str().to_string();
            app.switch_tool(next);
            app.push_progress(format!("✓ 已切换到 {}", name));
        }

        // Settings overlay when input is empty
        KeyCode::Char('s') if mods == KeyModifiers::NONE && app.input.is_empty() => {
            app.show_settings = true;
        }

        // Space toggles auto-scroll when input is empty
        KeyCode::Char(' ') if app.input.is_empty() => {
            app.auto_scroll = !app.auto_scroll;
            if app.auto_scroll {
                app.scroll_offset = 0;
            }
        }

        KeyCode::Char(c) => app.input_push(c),

        _ => {}
    }
}
