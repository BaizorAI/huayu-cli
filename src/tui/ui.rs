use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::tui::{
    app::{App, ConnectionStatus, LoginState},
    theme,
};

pub fn render(f: &mut Frame, app: &App) {
    let area = f.area();

    // ── Top-level layout: status bar | main panels | input | hints ─────────
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // status bar
            Constraint::Min(5),    // main panels (left output + right help)
            Constraint::Length(4), // input box
            Constraint::Length(1), // key hint bar
        ])
        .split(area);

    render_status_bar(f, app, rows[0]);
    render_main(f, app, rows[1]);
    render_input(f, app, rows[2]);
    render_hints(f, rows[3]);

    // ── Overlays ──────────────────────────────────────────────────────────
    if let Some(ov) = &app.login_overlay {
        render_login_overlay(f, ov, area);
    } else if app.show_settings {
        render_settings_overlay(f, app, area);
    }
}

// ── Status bar ─────────────────────────────────────────────────────────────

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let (status_label, status_color) = match &app.connection_status {
        ConnectionStatus::Connected => (app.connection_status.label(), theme::STATUS_OK),
        ConnectionStatus::NotConfigured => (app.connection_status.label(), theme::STATUS_WARN),
        _ => (app.connection_status.label(), theme::STATUS_ERR),
    };

    let tool_label = format!(" {} ", app.tool_type.as_str());
    let model_label = format!(" {} ", app.config.default_model);

    let line = Line::from(vec![
        Span::styled(
            " 华宇 huayu ",
            Style::default()
                .fg(theme::TITLE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("v{} ", env!("CARGO_PKG_VERSION")),
            Style::default().fg(theme::DIM),
        ),
        Span::styled("│", Style::default().fg(theme::BORDER)),
        Span::styled(tool_label, Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("[Tab切换]", Style::default().fg(theme::DIM)),
        Span::styled("│", Style::default().fg(theme::BORDER)),
        Span::styled(model_label, Style::default().fg(theme::DIM)),
        Span::styled("│", Style::default().fg(theme::BORDER)),
        Span::styled(
            format!(" {} ", status_label),
            Style::default().fg(status_color),
        ),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

// ── Main panels: left output (70%) + right help (30%) ──────────────────────

fn render_main(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(area);

    render_main_panel(f, app, cols[0]);
    render_help_panel(f, app, cols[1]);
}

fn render_main_panel(f: &mut Frame, app: &App, area: Rect) {
    let running = app.tool_process.is_some();
    let border_style = if running {
        Style::default().fg(theme::STATUS_OK)
    } else {
        Style::default().fg(theme::BORDER)
    };
    let title = if running && app.waiting_for_input {
        " 主工作区 [等待输入...] "
    } else if running {
        " 主工作区 [●] "
    } else if !app.auto_scroll {
        " 主工作区 [已暂停] "
    } else {
        " 主工作区 "
    };

    // Inner height (subtract 2 for top/bottom borders)
    let viewport_h = area.height.saturating_sub(2) as usize;
    let total = app.main_lines.len();

    // visible_start = max(0, total - viewport_h - scroll_offset)
    let visible_start = total.saturating_sub(viewport_h + app.scroll_offset);
    let visible_end = (visible_start + viewport_h).min(total);

    let items: Vec<ListItem> = if total == 0 {
        vec![]
    } else {
        app.main_lines[visible_start..visible_end]
            .iter()
            .map(|line| {
                let style = if line.starts_with("✓") || line.contains("[完成]") {
                    Style::default().fg(theme::STATUS_OK)
                } else if line.starts_with("✗") || line.contains("[错误]") {
                    Style::default().fg(theme::STATUS_ERR)
                } else if line.starts_with("[文件]") || line.starts_with("[下载]") {
                    Style::default().fg(theme::FILE_EVENT)
                } else if line.starts_with("───") || line.starts_with("──") {
                    Style::default()
                        .fg(theme::HIGHLIGHT)
                        .add_modifier(Modifier::BOLD)
                } else if line.starts_with("[提示]") || line.starts_with("[更新]") {
                    Style::default().fg(theme::STATUS_WARN)
                } else {
                    Style::default().fg(theme::PROMPT)
                };
                ListItem::new(Span::styled(line.as_str(), style))
            })
            .collect()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, Style::default().fg(theme::TITLE)))
        .border_style(border_style);

    if items.is_empty() {
        let inner = block.inner(area);
        f.render_widget(block, area);
        f.render_widget(
            Paragraph::new("发送消息后，AI 输出将显示在这里")
                .style(Style::default().fg(theme::DIM))
                .alignment(Alignment::Center),
            inner,
        );
    } else {
        f.render_widget(List::new(items).block(block), area);
    }
}

fn render_help_panel(f: &mut Frame, app: &App, area: Rect) {
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".to_string());

    let max_cwd = area.width.saturating_sub(10) as usize;
    let cwd_char_count = cwd.chars().count();
    let cwd_display = if cwd_char_count > max_cwd && max_cwd > 3 {
        let keep = max_cwd - 3;
        let start = cwd
            .char_indices()
            .nth(cwd_char_count - keep)
            .map(|(i, _)| i)
            .unwrap_or(0);
        format!("...{}", &cwd[start..])
    } else {
        cwd
    };

    let tool_avail = if app.tool_type.is_available() {
        "✓"
    } else {
        "✗"
    };
    let running = app.tool_process.is_some();

    let mut content: Vec<Line> = Vec::new();

    // ── Task-in-progress section (shown only when tool is running) ──────────
    if running {
        let (elapsed_secs, spinner_frame) = if let Some(start) = app.task_start {
            let elapsed = start.elapsed();
            let secs = elapsed.as_secs_f32();
            let frame = (elapsed.as_millis() / 200 % 4) as usize;
            (secs, frame)
        } else {
            (0.0, 0)
        };

        const SPINNER: [&str; 4] = ["◐", "◓", "◑", "◒"];
        let spinner = SPINNER[spinner_frame];
        let elapsed_str = if elapsed_secs < 60.0 {
            format!("{:.0}s", elapsed_secs)
        } else {
            format!("{:.0}m{:.0}s", elapsed_secs / 60.0, elapsed_secs % 60.0)
        };

        content.push(Line::from(Span::styled(
            "任务进行中",
            Style::default()
                .fg(theme::STATUS_OK)
                .add_modifier(Modifier::BOLD),
        )));
        content.push(Line::from(vec![
            Span::styled(
                format!("  {} ", spinner),
                Style::default().fg(theme::STATUS_OK),
            ),
            Span::styled(
                app.tool_type.as_str(),
                Style::default().fg(theme::HIGHLIGHT),
            ),
            Span::styled(
                format!("  [{}]", elapsed_str),
                Style::default().fg(theme::DIM),
            ),
        ]));

        // Show last non-empty output lines as a mini preview
        let preview_lines: Vec<&str> = app
            .main_lines
            .iter()
            .rev()
            .filter(|l| !l.is_empty() && !l.starts_with("───"))
            .take(5)
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        if !preview_lines.is_empty() {
            content.push(Line::from(""));
            content.push(Line::from(Span::styled(
                "最近输出",
                Style::default()
                    .fg(theme::TITLE)
                    .add_modifier(Modifier::BOLD),
            )));
            let max_w = area.width.saturating_sub(4) as usize;
            for line in preview_lines {
                let char_count = line.chars().count();
                let truncated = if char_count > max_w && max_w > 3 {
                    let end = line
                        .char_indices()
                        .nth(max_w - 1)
                        .map(|(i, _)| i)
                        .unwrap_or(line.len());
                    format!("{}…", &line[..end])
                } else {
                    line.to_string()
                };
                content.push(Line::from(Span::styled(
                    format!("  {}", truncated),
                    Style::default().fg(theme::DIM),
                )));
            }
        }

        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "────────────────────",
            Style::default().fg(theme::BORDER),
        )));
        content.push(Line::from(""));
    }

    // ── Shortcuts ───────────────────────────────────────────────────────────
    content.push(Line::from(Span::styled(
        "快捷键",
        Style::default()
            .fg(theme::TITLE)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(Line::from(Span::styled(
        "  Enter   发送/确认",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  Esc     取消/关闭",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  ↑/↓    输入历史",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  PgUp   向上翻页",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  PgDn   向下/回底",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  滚轮   上下滚动",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  Alt+Q  退出",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(Span::styled(
        "  /help  命令列表",
        Style::default().fg(theme::DIM),
    )));
    content.push(Line::from(""));

    // ── Status ──────────────────────────────────────────────────────────────
    content.push(Line::from(Span::styled(
        "当前状态",
        Style::default()
            .fg(theme::TITLE)
            .add_modifier(Modifier::BOLD),
    )));
    content.push(Line::from(vec![
        Span::styled("  工具  ", Style::default().fg(theme::DIM)),
        Span::styled(
            format!("{} {}", tool_avail, app.tool_type.as_str()),
            Style::default().fg(theme::HIGHLIGHT),
        ),
    ]));
    content.push(Line::from(vec![
        Span::styled("  模型  ", Style::default().fg(theme::DIM)),
        Span::styled(
            app.config.default_model.as_str(),
            Style::default().fg(theme::PROMPT),
        ),
    ]));
    content.push(Line::from(vec![
        Span::styled("  目录  ", Style::default().fg(theme::DIM)),
        Span::styled(cwd_display, Style::default().fg(theme::DIM)),
    ]));

    if !app.recent_commands.is_empty() {
        content.push(Line::from(""));
        content.push(Line::from(Span::styled(
            "最近命令",
            Style::default()
                .fg(theme::TITLE)
                .add_modifier(Modifier::BOLD),
        )));
        for cmd in &app.recent_commands {
            content.push(Line::from(Span::styled(
                format!("  {}", cmd),
                Style::default().fg(theme::STATUS_OK),
            )));
        }
    }

    let border_style = if running {
        Style::default().fg(theme::STATUS_OK)
    } else {
        Style::default().fg(theme::BORDER)
    };
    let panel_title = if running {
        " 任务状态 "
    } else {
        " 帮助与参考 "
    };

    f.render_widget(
        Paragraph::new(content)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(Span::styled(panel_title, Style::default().fg(theme::TITLE)))
                    .border_style(border_style),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

// ── Input box ──────────────────────────────────────────────────────────────

fn render_input(f: &mut Frame, app: &App, area: Rect) {
    let is_running = app.tool_process.is_some();
    let in_history = app.history_cursor.is_some();
    let title = if is_running {
        " 输入 [Esc取消  Enter转发] "
    } else if in_history {
        " 输入 [↑/↓历史  Enter发送] "
    } else {
        " 输入 [Enter发送  ↑历史] "
    };

    let block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme::TITLE)))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::BORDER));

    let (text, style) = if app.input.is_empty() {
        (
            "输入你的需求，或 /help 查看命令...",
            Style::default().fg(theme::DIM),
        )
    } else if in_history {
        (app.input.as_str(), Style::default().fg(theme::STATUS_WARN))
    } else {
        (app.input.as_str(), Style::default().fg(theme::PROMPT))
    };

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(text, style)))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Key hints bar ──────────────────────────────────────────────────────────

fn render_hints(f: &mut Frame, area: Rect) {
    let line = Line::from(vec![
        Span::styled("[Enter]", Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("发送  ", Style::default().fg(theme::DIM)),
        Span::styled("[Esc]", Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("取消  ", Style::default().fg(theme::DIM)),
        Span::styled("[↑/↓]", Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("历史  ", Style::default().fg(theme::DIM)),
        Span::styled("[PgUp/Dn]", Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("翻页  ", Style::default().fg(theme::DIM)),
        Span::styled("[Alt+Q]", Style::default().fg(theme::HIGHLIGHT)),
        Span::styled("退出  ", Style::default().fg(theme::DIM)),
        Span::styled("/help", Style::default().fg(theme::STATUS_OK)),
        Span::styled(" 查看全部命令", Style::default().fg(theme::DIM)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

// ── Login overlay ──────────────────────────────────────────────────────────

fn render_login_overlay(f: &mut Frame, ov: &crate::tui::app::LoginOverlay, area: Rect) {
    let popup = centered_rect(70, 40, area);
    f.render_widget(Clear, popup);

    let body = match &ov.state {
        LoginState::Waiting => format!(
            "\n在浏览器中打开：\n\n  {}\n\n等待登录中（按 Esc 取消，r 重试）...",
            ov.url
        ),
        LoginState::Error(e) => format!("登录失败: {}\n\n（按 r 重试，Esc 关闭）", e),
    };

    let block = Block::default()
        .title(Span::styled(
            " 华宇 — 登录 baizor.com ",
            Style::default()
                .fg(theme::TITLE)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::HIGHLIGHT));

    f.render_widget(
        Paragraph::new(body)
            .block(block)
            .wrap(Wrap { trim: true })
            .alignment(Alignment::Left),
        popup,
    );
}

// ── Settings overlay ───────────────────────────────────────────────────────

fn render_settings_overlay(f: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(60, 35, area);
    f.render_widget(Clear, popup);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .margin(1)
        .split(popup);

    let outer = Block::default()
        .title(Span::styled(
            " 设置 [Enter保存  Esc关闭  Tab切换字段] ",
            Style::default().fg(theme::TITLE),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::HIGHLIGHT));
    f.render_widget(outer, popup);

    let model_style = if app.settings_focus_field == 0 {
        Style::default().fg(theme::HIGHLIGHT)
    } else {
        Style::default().fg(theme::PROMPT)
    };
    let url_style = if app.settings_focus_field == 1 {
        Style::default().fg(theme::HIGHLIGHT)
    } else {
        Style::default().fg(theme::PROMPT)
    };

    f.render_widget(
        Paragraph::new(app.settings_model_input.as_str()).block(
            Block::default()
                .title("模型")
                .borders(Borders::ALL)
                .border_style(model_style),
        ),
        rows[0],
    );
    f.render_widget(
        Paragraph::new(app.settings_url_input.as_str()).block(
            Block::default()
                .title("Base URL")
                .borders(Borders::ALL)
                .border_style(url_style),
        ),
        rows[1],
    );
}

// ── Helper ─────────────────────────────────────────────────────────────────

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
