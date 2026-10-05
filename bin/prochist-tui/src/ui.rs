use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::{ACTION_ITEMS, App, FIELD_LABELS, Focus, HELP_LINES, View, display_name, fields};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Length(1),
    ])
    .split(frame.area());
    match app.view {
        View::Tree => draw_tree(frame, app, chunks[0]),
        View::Holders => draw_holders(frame, app, chunks[0]),
        View::Processes => draw_processes(frame, app, chunks[0]),
    }
    draw_details(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
    if app.show_actions {
        draw_actions(frame, app, frame.area());
    }
    if app.show_help {
        draw_help(frame, app, frame.area());
    }
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2);
    app.view_height = height;
    app.ensure_visible(height);

    let focused = app.focus == Focus::Tree && !app.show_help && !app.show_actions;
    let lines: Vec<Line> = app
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let mut style = Style::default();
            if row.is_current {
                style = style.fg(Color::Cyan).add_modifier(Modifier::BOLD);
            }
            if i == app.selected && focused {
                style = style.add_modifier(Modifier::REVERSED);
            }
            Line::styled(row.line.clone(), style)
        })
        .collect();

    let tree = Paragraph::new(lines)
        .block(Block::bordered().title(" Processes "))
        .scroll((app.scroll, 0));
    frame.render_widget(tree, area);
}

fn draw_holders(frame: &mut Frame, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2);
    app.view_height = height;
    app.ensure_visible(height);

    let focused = app.focus == Focus::Tree && !app.show_help && !app.show_actions;
    let Some(state) = &app.holders else {
        return;
    };
    let count = state.holders.len();
    let lines: Vec<Line> = state
        .holders
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let connector = if i + 1 == count {
                "└── "
            } else {
                "├── "
            };
            let style = if i == state.selected && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(format!("{connector}{} ({})", display_name(p), p.pid), style)
        })
        .collect();

    let title = format!(" Holders of {} ", state.path.display());
    let list = Paragraph::new(lines)
        .block(Block::bordered().title(title))
        .scroll((state.scroll, 0));
    frame.render_widget(list, area);
}

fn draw_processes(frame: &mut Frame, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2);
    app.view_height = height;
    app.ensure_visible(height);

    let focused = app.focus == Focus::Tree && !app.show_help && !app.show_actions;
    let visible = app.visible_processes();
    let selected = app
        .processes
        .as_ref()
        .map(|s| (s.selected, s.scroll, s.filter.clone()));
    let Some((selected, scroll, filter)) = selected else {
        return;
    };
    let lines: Vec<Line> = visible
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut text = format!("{:>7} {}", p.pid, display_name(p));
            if let Some(command) = &p.command {
                text.push_str(" — ");
                text.push_str(command);
            }
            let style = if i == selected && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(text, style)
        })
        .collect();

    let title = if filter.is_empty() {
        " Processes ".to_string()
    } else {
        format!(" Processes (filter: {filter}) ")
    };
    let list = Paragraph::new(lines)
        .block(Block::bordered().title(title))
        .scroll((scroll, 0));
    frame.render_widget(list, area);
}

fn draw_details(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Detail && !app.show_help && !app.show_actions;
    let values = app
        .selected_info()
        .map(fields)
        .unwrap_or_else(|| vec![None; FIELD_LABELS.len()]);
    let lines: Vec<Line> = FIELD_LABELS
        .iter()
        .zip(values)
        .enumerate()
        .map(|(i, (label, value))| {
            let text = format!("{label:<11}: {}", value.as_deref().unwrap_or("-"));
            let style = if i == app.detail_selected && focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(text, style)
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default()
    };
    let details = Paragraph::new(lines).block(
        Block::bordered()
            .title(" Details ")
            .border_style(border_style),
    );
    frame.render_widget(details, area);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let mode = if app.show_help {
        "HELP"
    } else if app.show_actions {
        "ACTIONS"
    } else if app.search.is_some() {
        "SEARCH"
    } else {
        match (app.view, app.focus) {
            (View::Holders, Focus::Tree) => "HOLDERS",
            (View::Processes, Focus::Tree) => "PROCESSES",
            (_, Focus::Tree) => "TREE",
            (_, Focus::Detail) => "DETAIL",
        }
    };
    let mode_tag = Span::styled(
        format!(" {mode} "),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let (index, total) = app.position();
    let position = format!(" {index}/{total}");
    let message = match (&app.status, &app.search) {
        (Some(message), _) => message.clone(),
        (None, Some(search)) => format!("/{}", search.query),
        (None, None) => " ?: help  q: quit".to_string(),
    };
    let separator = || Span::styled(" │ ", Style::default().fg(Color::DarkGray));
    let mut spans = vec![
        mode_tag,
        Span::raw(position),
        separator(),
        Span::raw(message),
    ];
    if let Some(crumb) = app.breadcrumb() {
        let used: usize = spans.iter().map(|s| s.width()).sum();
        let budget = (area.width as usize).saturating_sub(used + 3);
        let crumb = truncate(&crumb, budget);
        if !crumb.is_empty() {
            spans.push(separator());
            spans.push(Span::styled(crumb, Style::default().fg(Color::Cyan)));
        }
    }
    frame.render_widget(Line::from(spans), area);
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let popup = centered_rect(60, 80, area);
    frame.render_widget(Clear, popup);
    let lines: Vec<Line> = HELP_LINES.iter().map(|l| Line::from(*l)).collect();
    let help = Paragraph::new(lines)
        .block(Block::bordered().title(" Help (? to close) "))
        .wrap(Wrap { trim: false })
        .scroll((app.help_scroll, 0));
    frame.render_widget(help, popup);
}

fn draw_actions(frame: &mut Frame, app: &App, area: Rect) {
    let width = 36u16.min(area.width);
    let height = (ACTION_ITEMS.len() as u16 + 2).min(area.height);
    let popup = Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    };
    frame.render_widget(Clear, popup);
    let lines: Vec<Line> = ACTION_ITEMS
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let text = format!(" {}  {}", item.key, item.label);
            let style = if i == app.action_selected {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            Line::styled(text, style)
        })
        .collect();
    let actions = Paragraph::new(lines).block(
        Block::bordered()
            .title(" Actions ")
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(actions, popup);
}

fn truncate(text: &str, budget: usize) -> String {
    if text.chars().count() <= budget {
        return text.to_string();
    }
    if budget == 0 {
        return String::new();
    }
    let mut out: String = text.chars().take(budget.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use prochist_core::{Pid, ProcessInfo, ProcessTree};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    use crate::app::App;

    fn proc(pid: Pid, ppid: Pid, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            name: name.to_string(),
            command: None,
            exe: None,
            open_files: Vec::new(),
        }
    }

    fn sample_tree() -> ProcessTree {
        let mut current = proc(300, 200, "ph");
        current.command = Some("/usr/local/bin/ph 300".to_string());
        ProcessTree {
            ancestors: vec![proc(1, 0, "init"), proc(200, 1, "bash")],
            current,
            children: vec![proc(301, 300, "worker")],
        }
    }

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = buffer.area;
        let mut text = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                text.push_str(buffer[(x, y)].symbol());
            }
            text.push('\n');
        }
        text
    }

    #[test]
    fn renders_tree_details_and_status() {
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        let mut app = App::new(&sample_tree());
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("bash (200)"));
        assert!(text.contains("ph (300)"));
        assert!(text.contains("worker (301)"));
        assert!(text.contains("PID"));
        assert!(text.contains("/usr/local/bin/ph 300"));
        assert!(text.contains("TREE"));
        assert!(text.contains("3/4"));
    }

    #[test]
    fn renders_actions_popup() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut app = App::new(&sample_tree());
        app.show_actions = true;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("Actions"));
        assert!(text.contains("Focus this process"));
        assert!(text.contains("Yank command line"));
        assert!(text.contains("ACTIONS"));
    }

    #[test]
    fn renders_breadcrumb_after_focus() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut app = App::new(&sample_tree());
        let mut focused = sample_tree();
        focused.current = proc(301, 300, "worker");
        focused.ancestors = vec![
            proc(1, 0, "init"),
            proc(200, 1, "bash"),
            proc(300, 200, "ph"),
        ];
        focused.children = vec![];
        app.apply_focus(&focused);
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("ph > worker"));
    }

    #[test]
    fn status_message_survives_long_breadcrumb() {
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        let mut app = App::new(&sample_tree());
        for _ in 0..10 {
            app.apply_focus(&sample_tree());
        }
        app.note("no such process");
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        let status_line = text.lines().nth(13).unwrap();
        assert!(status_line.contains("no such process"));
        assert!(status_line.chars().count() <= 50);
    }

    #[test]
    fn renders_help_overlay() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut app = App::new(&sample_tree());
        app.show_help = true;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("Help (? to close)"));
        assert!(text.contains("half page down"));
    }

    fn holders_app() -> App {
        use std::path::PathBuf;
        App::new_holders(
            PathBuf::from("/var/log/app.log"),
            vec![proc(123, 1, "vim"), proc(456, 1, "code")],
        )
    }

    #[test]
    fn renders_holders_view() {
        let mut terminal = Terminal::new(TestBackend::new(50, 14)).unwrap();
        let mut app = holders_app();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("Holders of /var/log/app.log"));
        assert!(text.contains("├── vim (123)"));
        assert!(text.contains("└── code (456)"));
        assert!(text.contains("HOLDERS"));
        assert!(text.contains("1/2"));
    }

    #[test]
    fn renders_holders_breadcrumb_after_focus() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut app = holders_app();
        app.enter_tree_from_holders(&sample_tree());
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("/var/log/app.log > ph"));
        assert!(text.contains("TREE"));
    }

    fn processes_app() -> App {
        let mut vim = proc(123, 1, "vim");
        vim.command = Some("/usr/bin/vim /var/log/app.log".to_string());
        App::new_processes(vec![vim, proc(456, 1, "code")])
    }

    #[test]
    fn renders_processes_view() {
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        let mut app = processes_app();
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("123 vim"));
        assert!(text.contains("/usr/bin/vim /var/log/app.log"));
        assert!(text.contains("PROCESSES"));
        assert!(text.contains("1/2"));
    }

    #[test]
    fn renders_search_prompt_and_filter_title() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut terminal = Terminal::new(TestBackend::new(60, 14)).unwrap();
        let mut app = processes_app();
        app.handle_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        app.handle_key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE));
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("SEARCH"));
        assert!(text.contains("/v"));
        assert!(text.contains("filter: v"));
        assert!(text.contains("1/1"));
    }

    #[test]
    fn renders_empty_holders_after_refresh() {
        let mut terminal = Terminal::new(TestBackend::new(70, 14)).unwrap();
        let mut app = holders_app();
        app.replace_holders(Vec::new());
        app.note("no process has /var/log/app.log open");
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("0/0"));
        assert!(text.contains("no process has /var/log/app.log open"));
    }
}
