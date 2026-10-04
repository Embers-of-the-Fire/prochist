use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};

use crate::app::{App, FIELD_LABELS, Focus, HELP_LINES, fields};

pub fn draw(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(7),
        Constraint::Length(1),
    ])
    .split(frame.area());
    draw_tree(frame, app, chunks[0]);
    draw_details(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
    if app.show_help {
        draw_help(frame, app, frame.area());
    }
}

fn draw_tree(frame: &mut Frame, app: &mut App, area: Rect) {
    let height = area.height.saturating_sub(2);
    app.view_height = height;
    app.ensure_visible(height);

    let focused = app.focus == Focus::Tree && !app.show_help;
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

fn draw_details(frame: &mut Frame, app: &mut App, area: Rect) {
    let focused = app.focus == Focus::Detail && !app.show_help;
    let values = fields(&app.rows[app.selected].info);
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
    } else {
        match app.focus {
            Focus::Tree => "TREE",
            Focus::Detail => "DETAIL",
        }
    };
    let mode_tag = Span::styled(
        format!(" {mode} "),
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    );
    let position = format!(" {}/{}", app.selected + 1, app.rows.len());
    let message = app
        .status
        .clone()
        .unwrap_or_else(|| " ?: help  q: quit".to_string());
    let status = Line::from(vec![
        mode_tag,
        Span::raw(position),
        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
        Span::raw(message),
    ]);
    frame.render_widget(status, area);
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
    fn renders_help_overlay() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        let mut app = App::new(&sample_tree());
        app.show_help = true;
        terminal.draw(|f| draw(f, &mut app)).unwrap();
        let text = buffer_text(&terminal);
        assert!(text.contains("Help (? to close)"));
        assert!(text.contains("half page down"));
    }
}
