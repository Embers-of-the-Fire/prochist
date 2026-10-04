use std::path::Path;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prochist_core::{ProcessInfo, ProcessTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Detail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Copy(String),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub line: String,
    pub info: ProcessInfo,
    pub is_current: bool,
}

pub const FIELD_LABELS: [&str; 5] = ["PID", "PPID", "Name", "Command", "Executable"];

pub fn display_name(p: &ProcessInfo) -> String {
    match &p.exe {
        Some(exe) => Path::new(exe)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.name.clone()),
        None => p.name.clone(),
    }
}

pub fn fields(info: &ProcessInfo) -> Vec<Option<String>> {
    vec![
        Some(info.pid.to_string()),
        Some(info.ppid.to_string()),
        Some(info.name.clone()),
        info.command.clone(),
        info.exe.clone(),
    ]
}

fn build_rows(tree: &ProcessTree) -> Vec<Row> {
    let mut rows = Vec::new();
    for ancestor in &tree.ancestors {
        rows.push(Row {
            line: format!("├── {} ({})", display_name(ancestor), ancestor.pid),
            info: ancestor.clone(),
            is_current: false,
        });
    }
    rows.push(Row {
        line: format!("└── {} ({})", display_name(&tree.current), tree.current.pid),
        info: tree.current.clone(),
        is_current: true,
    });
    let child_count = tree.children.len();
    for (i, child) in tree.children.iter().enumerate() {
        let connector = if i + 1 == child_count {
            "└── "
        } else {
            "├── "
        };
        rows.push(Row {
            line: format!("    {connector}{} ({})", display_name(child), child.pid),
            info: child.clone(),
            is_current: false,
        });
    }
    rows
}

pub const HELP_LINES: [&str; 30] = [
    "phi - interactive process tree",
    "",
    "Navigation",
    "  j / Down        move down one row",
    "  k / Up          move up one row",
    "  Ctrl-d          half page down",
    "  Ctrl-u          half page up",
    "  Ctrl-f          full page down",
    "  Ctrl-b          full page up",
    "  gg              jump to the first row",
    "  G               jump to the last row",
    "",
    "Panes",
    "  Tab / i / Enter focus the details pane",
    "  Esc / q         return to the tree pane",
    "",
    "Yank (copy)",
    "  y  (tree)       yank \"name (pid)\" of the selected row",
    "  Y  (tree)       yank the full command line",
    "  y  (details)    yank the selected field value",
    "  Y  (details)    yank \"name (pid)\"",
    "",
    "Other",
    "  r               refresh the process snapshot",
    "  ?               toggle this help",
    "  q               quit (from the tree pane)",
    "",
    "Copying uses the OSC 52 escape sequence; it needs a",
    "terminal that supports it (kitty, alacritty, wezterm,",
    "foot, tmux, iTerm2, Windows Terminal).",
];

pub struct App {
    pub rows: Vec<Row>,
    pub selected: usize,
    pub scroll: u16,
    pub focus: Focus,
    pub detail_selected: usize,
    pub show_help: bool,
    pub help_scroll: u16,
    pub status: Option<String>,
    pub view_height: u16,
    pub should_quit: bool,
    pending_g: bool,
}

impl App {
    pub fn new(tree: &ProcessTree) -> Self {
        let selected = tree.ancestors.len();
        Self {
            rows: build_rows(tree),
            selected,
            scroll: 0,
            focus: Focus::Tree,
            detail_selected: 0,
            show_help: false,
            help_scroll: 0,
            status: None,
            view_height: 1,
            should_quit: false,
            pending_g: false,
        }
    }

    pub fn replace_tree(&mut self, tree: &ProcessTree) {
        let pid = self.rows[self.selected].info.pid;
        self.rows = build_rows(tree);
        self.selected = self
            .rows
            .iter()
            .position(|r| r.info.pid == pid)
            .unwrap_or(tree.ancestors.len())
            .min(self.rows.len() - 1);
        self.detail_selected = 0;
    }

    pub fn note(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    fn selected_row(&self) -> &Row {
        &self.rows[self.selected]
    }

    fn move_by(&mut self, delta: i64) {
        let last = self.rows.len() as i64 - 1;
        self.selected = (self.selected as i64 + delta).clamp(0, last) as usize;
    }

    fn half_page(&self) -> i64 {
        i64::from(self.view_height.max(2)) / 2
    }

    fn page(&self) -> i64 {
        i64::from(self.view_height.max(1))
    }

    pub fn ensure_visible(&mut self, height: u16) {
        let height = height.max(1);
        let selected = self.selected as u16;
        if selected < self.scroll {
            self.scroll = selected;
        } else if selected >= self.scroll + height {
            self.scroll = selected - height + 1;
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Option<Effect> {
        self.status = None;
        if key.code != KeyCode::Char('g') {
            self.pending_g = false;
        }
        if self.show_help {
            self.handle_help_key(key);
            return None;
        }
        match self.focus {
            Focus::Tree => self.handle_tree_key(key),
            Focus::Detail => self.handle_detail_key(key),
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => self.show_help = false,
            KeyCode::Char('j') | KeyCode::Down => {
                let max = HELP_LINES.len().saturating_sub(1) as u16;
                self.help_scroll = (self.help_scroll + 1).min(max);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1);
            }
            _ => {}
        }
    }

    fn handle_tree_key(&mut self, key: KeyEvent) -> Option<Effect> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab | KeyCode::Enter | KeyCode::Char('i') => self.focus = Focus::Detail,
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('d') if ctrl => self.move_by(self.half_page()),
            KeyCode::Char('u') if ctrl => self.move_by(-self.half_page()),
            KeyCode::Char('f') if ctrl => self.move_by(self.page()),
            KeyCode::Char('b') if ctrl => self.move_by(-self.page()),
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.selected = 0;
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') | KeyCode::End => self.selected = self.rows.len() - 1,
            KeyCode::Home => self.selected = 0,
            KeyCode::Char('y') => {
                let row = self.selected_row();
                return Some(Effect::Copy(format!(
                    "{} ({})",
                    display_name(&row.info),
                    row.info.pid
                )));
            }
            KeyCode::Char('Y') => {
                let info = &self.selected_row().info;
                match info.command.clone().or_else(|| info.exe.clone()) {
                    Some(value) => return Some(Effect::Copy(value)),
                    None => self.note("nothing to yank"),
                }
            }
            _ => {}
        }
        None
    }

    fn handle_detail_key(&mut self, key: KeyEvent) -> Option<Effect> {
        match key.code {
            KeyCode::Esc | KeyCode::Tab | KeyCode::Enter | KeyCode::Char('q') => {
                self.focus = Focus::Tree
            }
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.detail_selected = (self.detail_selected + 1).min(FIELD_LABELS.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.detail_selected = self.detail_selected.saturating_sub(1);
            }
            KeyCode::Char('y') => {
                match fields(&self.selected_row().info)[self.detail_selected].clone() {
                    Some(value) => return Some(Effect::Copy(value)),
                    None => self.note("nothing to yank"),
                }
            }
            KeyCode::Char('Y') => {
                let row = self.selected_row();
                return Some(Effect::Copy(format!(
                    "{} ({})",
                    display_name(&row.info),
                    row.info.pid
                )));
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prochist_core::Pid;

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
        current.exe = Some("/usr/local/bin/ph".to_string());
        current.command = Some("/usr/local/bin/ph 300".to_string());
        let mut child = proc(301, 300, "worker");
        child.command = Some("/usr/bin/worker --daemon".to_string());
        ProcessTree {
            ancestors: vec![proc(1, 0, "init"), proc(200, 1, "bash")],
            current,
            children: vec![child, proc(302, 300, "logger")],
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    #[test]
    fn builds_rows_and_selects_current() {
        let app = App::new(&sample_tree());
        let lines: Vec<&str> = app.rows.iter().map(|r| r.line.as_str()).collect();
        assert_eq!(
            lines,
            vec![
                "├── init (1)",
                "├── bash (200)",
                "└── ph (300)",
                "    ├── worker (301)",
                "    └── logger (302)",
            ]
        );
        assert_eq!(app.selected, 2);
        assert!(app.rows[2].is_current);
    }

    #[test]
    fn uses_exe_basename_for_display_name() {
        let app = App::new(&sample_tree());
        assert_eq!(app.rows[2].line, "└── ph (300)");
    }

    #[test]
    fn jk_move_within_bounds() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected, 1);
        app.handle_key(key(KeyCode::Char('k')));
        app.handle_key(key(KeyCode::Char('k')));
        assert_eq!(app.selected, 0);
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 1);
    }

    #[test]
    fn g_g_and_shift_g_jump() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, 2);
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, 0);
        app.handle_key(key(KeyCode::Char('G')));
        assert_eq!(app.selected, app.rows.len() - 1);
    }

    #[test]
    fn pending_g_is_cancelled_by_other_keys() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('g')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected, 3);
        app.handle_key(key(KeyCode::Char('g')));
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn ctrl_d_u_move_half_page() {
        let mut app = App::new(&sample_tree());
        app.view_height = 4;
        app.selected = 0;
        app.handle_key(ctrl(KeyCode::Char('d')));
        assert_eq!(app.selected, 2);
        app.handle_key(ctrl(KeyCode::Char('u')));
        assert_eq!(app.selected, 0);
        app.handle_key(ctrl(KeyCode::Char('f')));
        assert_eq!(app.selected, 4);
        app.handle_key(ctrl(KeyCode::Char('b')));
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn y_in_tree_yanks_name_pid() {
        let mut app = App::new(&sample_tree());
        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("ph (300)".to_string())));
    }

    #[test]
    fn shift_y_in_tree_yanks_command_or_reports_nothing() {
        let mut app = App::new(&sample_tree());
        let effect = app.handle_key(key(KeyCode::Char('Y')));
        assert_eq!(
            effect,
            Some(Effect::Copy("/usr/local/bin/ph 300".to_string()))
        );

        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        let effect = app.handle_key(key(KeyCode::Char('Y')));
        assert_eq!(effect, None);
        assert_eq!(app.status.as_deref(), Some("nothing to yank"));
    }

    #[test]
    fn detail_focus_navigates_and_yanks_fields() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Tab));
        assert_eq!(app.focus, Focus::Detail);

        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("300".to_string())));

        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.detail_selected, 4);
        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("/usr/local/bin/ph".to_string())));

        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::Tree);
    }

    #[test]
    fn detail_yank_missing_field_reports_nothing() {
        let mut tree = sample_tree();
        tree.current.command = None;
        let mut app = App::new(&tree);
        app.handle_key(key(KeyCode::Enter));
        for _ in 0..3 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        assert_eq!(app.detail_selected, 3);
        assert_eq!(app.handle_key(key(KeyCode::Char('y'))), None);
        assert_eq!(app.status.as_deref(), Some("nothing to yank"));
    }

    #[test]
    fn help_toggles_and_scrolls() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('?')));
        assert!(app.show_help);
        for _ in 0..HELP_LINES.len() + 5 {
            app.handle_key(key(KeyCode::Char('j')));
        }
        assert_eq!(app.help_scroll as usize, HELP_LINES.len() - 1);
        app.handle_key(key(KeyCode::Char('?')));
        assert!(!app.show_help);
    }

    #[test]
    fn help_swallows_other_keys() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('?')));
        assert_eq!(app.handle_key(key(KeyCode::Char('y'))), None);
        assert!(!app.should_quit);
        assert!(app.show_help);
    }

    #[test]
    fn q_quits_only_from_tree_focus() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Tab));
        app.handle_key(key(KeyCode::Char('q')));
        assert!(!app.should_quit);
        assert_eq!(app.focus, Focus::Tree);
        app.handle_key(key(KeyCode::Char('q')));
        assert!(app.should_quit);
    }

    #[test]
    fn replace_tree_preserves_selection_by_pid() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.rows[app.selected].info.pid, 301);
        app.replace_tree(&sample_tree());
        assert_eq!(app.rows[app.selected].info.pid, 301);
    }

    #[test]
    fn ensure_visible_scrolls_to_selection() {
        let mut app = App::new(&sample_tree());
        app.selected = 4;
        app.ensure_visible(3);
        assert_eq!(app.scroll, 2);
        app.selected = 0;
        app.ensure_visible(3);
        assert_eq!(app.scroll, 0);
    }
}
