use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use prochist_core::{Pid, ProcessInfo, ProcessTree};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tree,
    Holders,
    Processes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FocusEntry {
    Process(Pid, String),
    Holders,
    Processes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    Copy(String),
    Focus(Pid),
    Restore(Pid),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchField {
    Any,
    Pid,
    Name,
    Cmd,
    Exe,
}

fn parse_query(query: &str) -> (SearchField, &str) {
    let prefixes = [
        ("pid:", SearchField::Pid),
        ("name:", SearchField::Name),
        ("cmd:", SearchField::Cmd),
        ("exe:", SearchField::Exe),
    ];
    for (prefix, field) in prefixes {
        if let Some(rest) = query.strip_prefix(prefix) {
            return (field, rest.trim());
        }
    }
    (SearchField::Any, query)
}

fn field_matches(info: &ProcessInfo, field: SearchField, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let contains = |text: &str| text.to_lowercase().contains(&query.to_lowercase());
    match field {
        SearchField::Any => contains(&info.name) || info.command.as_deref().is_some_and(contains),
        SearchField::Pid => info.pid.to_string().starts_with(query),
        SearchField::Name => contains(&info.name),
        SearchField::Cmd => info.command.as_deref().is_some_and(contains),
        SearchField::Exe => info.exe.as_deref().is_some_and(contains),
    }
}

fn filtered<'a>(all: &'a [ProcessInfo], filter: &str) -> impl Iterator<Item = &'a ProcessInfo> {
    let (field, query) = parse_query(filter);
    all.iter()
        .filter(move |info| field_matches(info, field, query))
}

pub struct SearchState {
    pub query: String,
    saved_filter: String,
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

pub struct ActionItem {
    pub key: char,
    pub label: &'static str,
}

pub const ACTION_ITEMS: [ActionItem; 5] = [
    ActionItem {
        key: 'f',
        label: "Focus this process",
    },
    ActionItem {
        key: 'y',
        label: "Yank name (pid)",
    },
    ActionItem {
        key: 'c',
        label: "Yank command line",
    },
    ActionItem {
        key: 'p',
        label: "Yank PID",
    },
    ActionItem {
        key: 'e',
        label: "Yank executable path",
    },
];

pub const HELP_LINES: [&str; 63] = [
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
    "Focus",
    "  Enter / f       focus the selected process (re-root)",
    "  Esc / Backspace return to the previous focus",
    "  a               action menu for the selected row",
    "",
    "Processes view (phi with no arguments)",
    "  /               live-filter all processes",
    "  Enter / f       focus the selected process",
    "  Esc             clear the active filter",
    "",
    "Search",
    "  /               search (filter in processes view,",
    "                  jump-to-match in tree/holders)",
    "  n / N           next / previous match (tree/holders)",
    "  Enter           apply the query, Esc cancel",
    "  field prefixes  pid: name: cmd: exe:",
    "  (no prefix)     match name + command",
    "",
    "Holders view (phi -f PATH)",
    "  Enter / f       focus the selected holder's process",
    "  Esc / Backspace (in the focused tree) back to holders",
    "  r               re-query which processes hold the path",
    "",
    "Action menu (a)",
    "  j / k           move through the actions",
    "  Enter           run the highlighted action",
    "  f               focus this process",
    "  y               yank \"name (pid)\"",
    "  c               yank the command line",
    "  p               yank the PID",
    "  e               yank the executable path",
    "  Esc / a / q     close the menu",
    "",
    "Panes",
    "  Tab / i         focus the details pane",
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

pub struct HoldersState {
    pub path: PathBuf,
    pub holders: Vec<ProcessInfo>,
    pub selected: usize,
    pub scroll: u16,
}

pub struct ProcessesState {
    pub all: Vec<ProcessInfo>,
    pub filter: String,
    pub selected: usize,
    pub scroll: u16,
}

pub struct App {
    pub rows: Vec<Row>,
    pub selected: usize,
    pub scroll: u16,
    pub focus: Focus,
    pub view: View,
    pub holders: Option<HoldersState>,
    pub processes: Option<ProcessesState>,
    pub search: Option<SearchState>,
    pub detail_selected: usize,
    pub show_help: bool,
    pub help_scroll: u16,
    pub show_actions: bool,
    pub action_selected: usize,
    pub status: Option<String>,
    pub view_height: u16,
    pub should_quit: bool,
    focus_stack: Vec<FocusEntry>,
    pending_g: bool,
    matches: Vec<usize>,
    match_idx: usize,
}

impl App {
    pub fn new(tree: &ProcessTree) -> Self {
        let selected = tree.ancestors.len();
        Self {
            rows: build_rows(tree),
            selected,
            scroll: 0,
            focus: Focus::Tree,
            view: View::Tree,
            holders: None,
            processes: None,
            search: None,
            detail_selected: 0,
            show_help: false,
            help_scroll: 0,
            show_actions: false,
            action_selected: 0,
            status: None,
            view_height: 1,
            should_quit: false,
            focus_stack: Vec::new(),
            pending_g: false,
            matches: Vec::new(),
            match_idx: 0,
        }
    }

    pub fn new_holders(path: PathBuf, holders: Vec<ProcessInfo>) -> Self {
        let mut app = Self::new(&ProcessTree {
            ancestors: Vec::new(),
            current: ProcessInfo {
                pid: 0,
                ppid: 0,
                name: String::new(),
                command: None,
                exe: None,
                open_files: Vec::new(),
            },
            children: Vec::new(),
        });
        app.rows = Vec::new();
        app.selected = 0;
        app.view = View::Holders;
        app.holders = Some(HoldersState {
            path,
            holders,
            selected: 0,
            scroll: 0,
        });
        app
    }

    pub fn new_processes(mut processes: Vec<ProcessInfo>) -> Self {
        processes.sort_by_key(|p| p.pid);
        let mut app = Self::new(&ProcessTree {
            ancestors: Vec::new(),
            current: ProcessInfo {
                pid: 0,
                ppid: 0,
                name: String::new(),
                command: None,
                exe: None,
                open_files: Vec::new(),
            },
            children: Vec::new(),
        });
        app.rows = Vec::new();
        app.selected = 0;
        app.view = View::Processes;
        app.processes = Some(ProcessesState {
            all: processes,
            filter: String::new(),
            selected: 0,
            scroll: 0,
        });
        app
    }

    pub fn visible_processes(&self) -> Vec<&ProcessInfo> {
        match &self.processes {
            Some(state) => filtered(&state.all, &state.filter).collect(),
            None => Vec::new(),
        }
    }

    pub fn replace_processes(&mut self, mut all: Vec<ProcessInfo>) {
        let selected_pid = self.selected_info().map(|p| p.pid);
        let Some(state) = &mut self.processes else {
            return;
        };
        all.sort_by_key(|p| p.pid);
        state.all = all;
        state.selected = selected_pid
            .and_then(|pid| filtered(&state.all, &state.filter).position(|p| p.pid == pid))
            .unwrap_or(0);
        state.scroll = 0;
        self.detail_selected = 0;
    }

    pub fn enter_tree_from_processes(&mut self, tree: &ProcessTree) {
        self.focus_stack.push(FocusEntry::Processes);
        self.view = View::Tree;
        self.set_rows(tree);
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
        self.matches.clear();
    }

    pub fn apply_focus(&mut self, tree: &ProcessTree) {
        let entry = {
            let current = self.current_row();
            FocusEntry::Process(current.info.pid, display_name(&current.info))
        };
        self.focus_stack.push(entry);
        self.set_rows(tree);
    }

    pub fn enter_tree_from_holders(&mut self, tree: &ProcessTree) {
        self.focus_stack.push(FocusEntry::Holders);
        self.view = View::Tree;
        self.set_rows(tree);
    }

    pub fn replace_holders(&mut self, holders: Vec<ProcessInfo>) {
        let Some(state) = &mut self.holders else {
            return;
        };
        let pid = state.holders.get(state.selected).map(|p| p.pid);
        state.selected = pid
            .and_then(|pid| holders.iter().position(|p| p.pid == pid))
            .unwrap_or(0)
            .min(holders.len().saturating_sub(1));
        state.holders = holders;
        state.scroll = 0;
        self.detail_selected = 0;
        self.matches.clear();
    }

    pub fn apply_restore(&mut self, tree: &ProcessTree) {
        self.set_rows(tree);
    }

    fn set_rows(&mut self, tree: &ProcessTree) {
        self.rows = build_rows(tree);
        self.selected = tree.ancestors.len();
        self.scroll = 0;
        self.detail_selected = 0;
        self.matches.clear();
    }

    pub fn focused_pid(&self) -> Pid {
        self.current_row().info.pid
    }

    pub fn breadcrumb(&self) -> Option<String> {
        if self.focus_stack.is_empty() {
            return None;
        }
        let mut parts: Vec<String> = self
            .focus_stack
            .iter()
            .map(|entry| match entry {
                FocusEntry::Process(_, name) => name.clone(),
                FocusEntry::Holders => self
                    .holders
                    .as_ref()
                    .map(|h| h.path.display().to_string())
                    .unwrap_or_default(),
                FocusEntry::Processes => "processes".to_string(),
            })
            .collect();
        parts.push(display_name(&self.current_row().info));
        Some(parts.join(" > "))
    }

    pub fn note(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    pub fn selected_info(&self) -> Option<&ProcessInfo> {
        match self.view {
            View::Tree => self.rows.get(self.selected).map(|r| &r.info),
            View::Holders => {
                let state = self.holders.as_ref()?;
                state.holders.get(state.selected)
            }
            View::Processes => {
                let state = self.processes.as_ref()?;
                filtered(&state.all, &state.filter).nth(state.selected)
            }
        }
    }

    pub fn position(&self) -> (usize, usize) {
        match self.view {
            View::Tree => (self.selected + 1, self.rows.len()),
            View::Holders => match &self.holders {
                Some(state) if !state.holders.is_empty() => {
                    (state.selected + 1, state.holders.len())
                }
                _ => (0, 0),
            },
            View::Processes => match &self.processes {
                Some(state) => {
                    let total = filtered(&state.all, &state.filter).count();
                    if total == 0 {
                        (0, 0)
                    } else {
                        (state.selected + 1, total)
                    }
                }
                None => (0, 0),
            },
        }
    }

    fn selected_row(&self) -> &Row {
        &self.rows[self.selected]
    }

    fn current_row(&self) -> &Row {
        self.rows
            .iter()
            .find(|r| r.is_current)
            .unwrap_or(&self.rows[self.selected])
    }

    fn move_by(&mut self, delta: i64) {
        match self.view {
            View::Tree => {
                let last = self.rows.len() as i64 - 1;
                self.selected = (self.selected as i64 + delta).clamp(0, last) as usize;
            }
            View::Holders => {
                let Some(state) = &mut self.holders else {
                    return;
                };
                if state.holders.is_empty() {
                    state.selected = 0;
                    return;
                }
                let last = state.holders.len() as i64 - 1;
                state.selected = (state.selected as i64 + delta).clamp(0, last) as usize;
            }
            View::Processes => {
                let Some(state) = &mut self.processes else {
                    return;
                };
                let count = filtered(&state.all, &state.filter).count();
                if count == 0 {
                    state.selected = 0;
                    return;
                }
                let last = count as i64 - 1;
                state.selected = (state.selected as i64 + delta).clamp(0, last) as usize;
            }
        }
    }

    fn jump_to_start(&mut self) {
        match self.view {
            View::Tree => self.selected = 0,
            View::Holders => {
                if let Some(state) = &mut self.holders {
                    state.selected = 0;
                }
            }
            View::Processes => {
                if let Some(state) = &mut self.processes {
                    state.selected = 0;
                }
            }
        }
    }

    fn jump_to_end(&mut self) {
        match self.view {
            View::Tree => self.selected = self.rows.len() - 1,
            View::Holders => {
                if let Some(state) = &mut self.holders {
                    state.selected = state.holders.len().saturating_sub(1);
                }
            }
            View::Processes => {
                if let Some(state) = &mut self.processes {
                    let count = filtered(&state.all, &state.filter).count();
                    state.selected = count.saturating_sub(1);
                }
            }
        }
    }

    fn half_page(&self) -> i64 {
        i64::from(self.view_height.max(2)) / 2
    }

    fn page(&self) -> i64 {
        i64::from(self.view_height.max(1))
    }

    pub fn ensure_visible(&mut self, height: u16) {
        let height = height.max(1);
        let (selected, scroll) = match self.view {
            View::Tree => (self.selected as u16, &mut self.scroll),
            View::Holders => match &mut self.holders {
                Some(state) => (state.selected as u16, &mut state.scroll),
                None => return,
            },
            View::Processes => match &mut self.processes {
                Some(state) => (state.selected as u16, &mut state.scroll),
                None => return,
            },
        };
        if selected < *scroll {
            *scroll = selected;
        } else if selected >= *scroll + height {
            *scroll = selected - height + 1;
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
        if self.show_actions {
            return self.handle_actions_key(key);
        }
        if self.search.is_some() {
            self.handle_search_key(key);
            return None;
        }
        match (self.view, self.focus) {
            (View::Tree, Focus::Tree) => self.handle_tree_key(key),
            (View::Holders, Focus::Tree) => self.handle_holders_key(key),
            (View::Processes, Focus::Tree) => self.handle_processes_key(key),
            (_, Focus::Detail) => self.handle_detail_key(key),
        }
    }

    fn open_search(&mut self) {
        let filter = match &self.processes {
            Some(state) if self.view == View::Processes => state.filter.clone(),
            _ => String::new(),
        };
        self.search = Some(SearchState {
            query: filter.clone(),
            saved_filter: filter,
        });
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        let filter_mode = self.view == View::Processes;
        match key.code {
            KeyCode::Esc => {
                if filter_mode
                    && let (Some(search), Some(state)) = (&self.search, &mut self.processes)
                {
                    state.filter = search.saved_filter.clone();
                    Self::clamp_processes_selection(state);
                }
                self.search = None;
                self.matches.clear();
            }
            KeyCode::Enter => {
                let Some(search) = self.search.take() else {
                    return;
                };
                if filter_mode {
                    self.note(format!("filter: {}", search.query));
                } else {
                    self.matches = self.find_matches(&search.query);
                    self.match_idx = 0;
                    if self.matches.is_empty() {
                        self.note("no matches");
                    } else {
                        let idx = self.matches[0];
                        self.set_selected_index(idx);
                        self.note(format!("match 1/{}", self.matches.len()));
                    }
                }
            }
            KeyCode::Backspace => {
                if let Some(search) = &mut self.search {
                    search.query.pop();
                }
                if filter_mode {
                    self.sync_filter_from_search();
                }
            }
            KeyCode::Char(c) => {
                if let Some(search) = &mut self.search {
                    search.query.push(c);
                }
                if filter_mode {
                    self.sync_filter_from_search();
                }
            }
            _ => {}
        }
    }

    fn sync_filter_from_search(&mut self) {
        if let (Some(search), Some(state)) = (&self.search, &mut self.processes) {
            state.filter = search.query.clone();
            Self::clamp_processes_selection(state);
        }
    }

    fn clamp_processes_selection(state: &mut ProcessesState) {
        let count = filtered(&state.all, &state.filter).count();
        state.selected = state.selected.min(count.saturating_sub(1));
    }

    fn find_matches(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }
        let (field, query) = parse_query(query);
        match self.view {
            View::Tree => self
                .rows
                .iter()
                .enumerate()
                .filter(|(_, row)| field_matches(&row.info, field, query))
                .map(|(i, _)| i)
                .collect(),
            View::Holders => match &self.holders {
                Some(state) => state
                    .holders
                    .iter()
                    .enumerate()
                    .filter(|(_, info)| field_matches(info, field, query))
                    .map(|(i, _)| i)
                    .collect(),
                None => Vec::new(),
            },
            View::Processes => Vec::new(),
        }
    }

    fn set_selected_index(&mut self, idx: usize) {
        match self.view {
            View::Tree => self.selected = idx.min(self.rows.len().saturating_sub(1)),
            View::Holders => {
                if let Some(state) = &mut self.holders {
                    state.selected = idx.min(state.holders.len().saturating_sub(1));
                }
            }
            View::Processes => {
                if let Some(state) = &mut self.processes {
                    state.selected = idx;
                    Self::clamp_processes_selection(state);
                }
            }
        }
    }

    fn cycle_match(&mut self, delta: i64) {
        if self.matches.is_empty() {
            self.note("no active search");
            return;
        }
        let len = self.matches.len() as i64;
        self.match_idx = (self.match_idx as i64 + delta).rem_euclid(len) as usize;
        let idx = self.matches[self.match_idx];
        self.set_selected_index(idx);
        self.note(format!(
            "match {}/{}",
            self.match_idx + 1,
            self.matches.len()
        ));
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
            KeyCode::Tab | KeyCode::Char('i') => self.focus = Focus::Detail,
            KeyCode::Esc | KeyCode::Backspace => match self.focus_stack.pop() {
                Some(FocusEntry::Process(pid, _)) => return Some(Effect::Restore(pid)),
                Some(FocusEntry::Holders) => {
                    self.view = View::Holders;
                    self.detail_selected = 0;
                }
                Some(FocusEntry::Processes) => {
                    self.view = View::Processes;
                    self.detail_selected = 0;
                }
                None => {}
            },
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('n') => self.cycle_match(1),
            KeyCode::Char('N') => self.cycle_match(-1),
            KeyCode::Char('a') => {
                self.show_actions = true;
                self.action_selected = 0;
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('d') if ctrl => self.move_by(self.half_page()),
            KeyCode::Char('u') if ctrl => self.move_by(-self.half_page()),
            KeyCode::Char('f') if ctrl => self.move_by(self.page()),
            KeyCode::Char('b') if ctrl => self.move_by(-self.page()),
            KeyCode::Enter | KeyCode::Char('f') => {
                let row = self.selected_row();
                if row.is_current {
                    self.note("already focused");
                } else {
                    return Some(Effect::Focus(row.info.pid));
                }
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.jump_to_start();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') | KeyCode::End => self.jump_to_end(),
            KeyCode::Home => self.jump_to_start(),
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

    fn handle_holders_key(&mut self, key: KeyEvent) -> Option<Effect> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab | KeyCode::Char('i') => self.focus = Focus::Detail,
            KeyCode::Char('a') => {
                self.show_actions = true;
                self.action_selected = 0;
            }
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('n') => self.cycle_match(1),
            KeyCode::Char('N') => self.cycle_match(-1),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('d') if ctrl => self.move_by(self.half_page()),
            KeyCode::Char('u') if ctrl => self.move_by(-self.half_page()),
            KeyCode::Char('f') if ctrl => self.move_by(self.page()),
            KeyCode::Char('b') if ctrl => self.move_by(-self.page()),
            KeyCode::Enter | KeyCode::Char('f') => {
                if let Some(info) = self.selected_info() {
                    return Some(Effect::Focus(info.pid));
                }
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.jump_to_start();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') | KeyCode::End => self.jump_to_end(),
            KeyCode::Home => self.jump_to_start(),
            KeyCode::Char('y') => match self.selected_info() {
                Some(info) => {
                    let text = format!("{} ({})", display_name(info), info.pid);
                    return Some(Effect::Copy(text));
                }
                None => self.note("nothing to yank"),
            },
            KeyCode::Char('Y') => {
                let value = self
                    .selected_info()
                    .and_then(|info| info.command.clone().or_else(|| info.exe.clone()));
                match value {
                    Some(value) => return Some(Effect::Copy(value)),
                    None => self.note("nothing to yank"),
                }
            }
            _ => {}
        }
        None
    }

    fn handle_processes_key(&mut self, key: KeyEvent) -> Option<Effect> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Tab | KeyCode::Char('i') => self.focus = Focus::Detail,
            KeyCode::Esc => {
                if let Some(state) = &mut self.processes
                    && !state.filter.is_empty()
                {
                    state.filter.clear();
                    state.selected = 0;
                    state.scroll = 0;
                    self.note("filter cleared");
                }
            }
            KeyCode::Char('a') => {
                self.show_actions = true;
                self.action_selected = 0;
            }
            KeyCode::Char('/') => self.open_search(),
            KeyCode::Char('j') | KeyCode::Down => self.move_by(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_by(-1),
            KeyCode::Char('d') if ctrl => self.move_by(self.half_page()),
            KeyCode::Char('u') if ctrl => self.move_by(-self.half_page()),
            KeyCode::Char('f') if ctrl => self.move_by(self.page()),
            KeyCode::Char('b') if ctrl => self.move_by(-self.page()),
            KeyCode::Enter | KeyCode::Char('f') => {
                if let Some(info) = self.selected_info() {
                    return Some(Effect::Focus(info.pid));
                }
            }
            KeyCode::Char('g') => {
                if self.pending_g {
                    self.jump_to_start();
                    self.pending_g = false;
                } else {
                    self.pending_g = true;
                }
            }
            KeyCode::Char('G') | KeyCode::End => self.jump_to_end(),
            KeyCode::Home => self.jump_to_start(),
            KeyCode::Char('y') => match self.selected_info() {
                Some(info) => {
                    let text = format!("{} ({})", display_name(info), info.pid);
                    return Some(Effect::Copy(text));
                }
                None => self.note("nothing to yank"),
            },
            KeyCode::Char('Y') => {
                let value = self
                    .selected_info()
                    .and_then(|info| info.command.clone().or_else(|| info.exe.clone()));
                match value {
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
                let value = self
                    .selected_info()
                    .and_then(|info| fields(info)[self.detail_selected].clone());
                match value {
                    Some(value) => return Some(Effect::Copy(value)),
                    None => self.note("nothing to yank"),
                }
            }
            KeyCode::Char('Y') => match self.selected_info() {
                Some(info) => {
                    let text = format!("{} ({})", display_name(info), info.pid);
                    return Some(Effect::Copy(text));
                }
                None => self.note("nothing to yank"),
            },
            _ => {}
        }
        None
    }

    fn handle_actions_key(&mut self, key: KeyEvent) -> Option<Effect> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('a') | KeyCode::Char('q') => self.show_actions = false,
            KeyCode::Char('j') | KeyCode::Down => {
                self.action_selected = (self.action_selected + 1).min(ACTION_ITEMS.len() - 1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.action_selected = self.action_selected.saturating_sub(1);
            }
            KeyCode::Enter => return self.run_action(self.action_selected),
            KeyCode::Char(c) => {
                if let Some(idx) = ACTION_ITEMS.iter().position(|item| item.key == c) {
                    return self.run_action(idx);
                }
            }
            _ => {}
        }
        None
    }

    fn run_action(&mut self, idx: usize) -> Option<Effect> {
        let Some(info) = self.selected_info().cloned() else {
            self.note("nothing to act on");
            return None;
        };
        let pid = info.pid;
        let is_current =
            self.view == View::Tree && self.rows.get(self.selected).is_some_and(|r| r.is_current);
        let name_pid = format!("{} ({})", display_name(&info), pid);
        let command = info.command.clone();
        let exe = info.exe.clone();
        match idx {
            0 => {
                self.show_actions = false;
                if is_current {
                    self.note("already focused");
                    None
                } else {
                    Some(Effect::Focus(pid))
                }
            }
            1 => {
                self.show_actions = false;
                Some(Effect::Copy(name_pid))
            }
            2 => match command {
                Some(value) => {
                    self.show_actions = false;
                    Some(Effect::Copy(value))
                }
                None => {
                    self.note("nothing to yank");
                    None
                }
            },
            3 => {
                self.show_actions = false;
                Some(Effect::Copy(pid.to_string()))
            }
            4 => match exe {
                Some(value) => {
                    self.show_actions = false;
                    Some(Effect::Copy(value))
                }
                None => {
                    self.note("nothing to yank");
                    None
                }
            },
            _ => None,
        }
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
            open_files: Vec::new(),
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
        app.handle_key(key(KeyCode::Tab));
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
    fn enter_returns_focus_effect_for_selected_row() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('j')));
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, Some(Effect::Focus(301)));
        let effect = app.handle_key(key(KeyCode::Char('f')));
        assert_eq!(effect, Some(Effect::Focus(301)));
    }

    #[test]
    fn enter_on_current_row_notes_already_focused() {
        let mut app = App::new(&sample_tree());
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, None);
        assert_eq!(app.status.as_deref(), Some("already focused"));
    }

    #[test]
    fn ctrl_f_still_pages_down() {
        let mut app = App::new(&sample_tree());
        app.view_height = 4;
        app.selected = 0;
        app.handle_key(ctrl(KeyCode::Char('f')));
        assert_eq!(app.selected, 4);
    }

    #[test]
    fn apply_focus_pushes_stack_and_reselects_current() {
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
        assert_eq!(app.focused_pid(), 301);
        assert_eq!(app.selected, 3);
        assert_eq!(app.breadcrumb().as_deref(), Some("ph > worker"));
    }

    #[test]
    fn esc_pops_focus_stack_and_restores() {
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
        let effect = app.handle_key(key(KeyCode::Esc));
        assert_eq!(effect, Some(Effect::Restore(300)));
        assert_eq!(app.breadcrumb(), None);
        assert_eq!(app.handle_key(key(KeyCode::Backspace)), None);
    }

    #[test]
    fn actions_popup_navigates_and_runs_highlighted() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('a')));
        assert!(app.show_actions);
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.action_selected, ACTION_ITEMS.len() - 1);
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, None);
        assert_eq!(app.status.as_deref(), Some("nothing to yank"));
        assert!(app.show_actions);
        app.handle_key(key(KeyCode::Esc));
        assert!(!app.show_actions);
    }

    #[test]
    fn actions_popup_hotkeys() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('p')));
        assert_eq!(effect, Some(Effect::Copy("300".to_string())));
        assert!(!app.show_actions);

        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("ph (300)".to_string())));

        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('c')));
        assert_eq!(
            effect,
            Some(Effect::Copy("/usr/local/bin/ph 300".to_string()))
        );

        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('e')));
        assert_eq!(effect, Some(Effect::Copy("/usr/local/bin/ph".to_string())));
        assert!(!app.show_actions);

        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('f')));
        assert_eq!(effect, Some(Effect::Focus(301)));
        assert!(!app.show_actions);
    }

    #[test]
    fn actions_popup_focus_on_current_notes_already_focused() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, None);
        assert_eq!(app.status.as_deref(), Some("already focused"));
        assert!(!app.show_actions);
    }

    #[test]
    fn help_takes_precedence_over_actions() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('a')));
        app.show_help = true;
        assert_eq!(app.handle_key(key(KeyCode::Char('p'))), None);
        assert!(app.show_actions);
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

    fn holders_app() -> App {
        let mut vim = proc(123, 1, "vim");
        vim.command = Some("/usr/bin/vim /var/log/app.log".to_string());
        App::new_holders(
            PathBuf::from("/var/log/app.log"),
            vec![vim, proc(456, 1, "code")],
        )
    }

    #[test]
    fn holders_view_navigates_within_bounds() {
        let mut app = holders_app();
        assert_eq!(app.view, View::Holders);
        assert_eq!(app.position(), (1, 2));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.position(), (2, 2));
        app.handle_key(key(KeyCode::Char('g')));
        app.handle_key(key(KeyCode::Char('g')));
        assert_eq!(app.position(), (1, 2));
        app.handle_key(key(KeyCode::Char('G')));
        assert_eq!(app.position(), (2, 2));
    }

    #[test]
    fn holders_enter_returns_focus_effect() {
        let mut app = holders_app();
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, Some(Effect::Focus(123)));
        app.handle_key(key(KeyCode::Char('j')));
        let effect = app.handle_key(key(KeyCode::Char('f')));
        assert_eq!(effect, Some(Effect::Focus(456)));
    }

    #[test]
    fn enter_tree_from_holders_and_esc_roundtrip() {
        let mut app = holders_app();
        app.enter_tree_from_holders(&sample_tree());
        assert_eq!(app.view, View::Tree);
        assert_eq!(app.breadcrumb().as_deref(), Some("/var/log/app.log > ph"));
        let effect = app.handle_key(key(KeyCode::Esc));
        assert_eq!(effect, None);
        assert_eq!(app.view, View::Holders);
        assert_eq!(app.breadcrumb(), None);
        assert_eq!(app.handle_key(key(KeyCode::Backspace)), None);
        assert_eq!(app.view, View::Holders);
    }

    #[test]
    fn holders_yank_and_detail_pane() {
        let mut app = holders_app();
        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("vim (123)".to_string())));
        let effect = app.handle_key(key(KeyCode::Char('Y')));
        assert_eq!(
            effect,
            Some(Effect::Copy("/usr/bin/vim /var/log/app.log".to_string()))
        );

        app.handle_key(key(KeyCode::Tab));
        let effect = app.handle_key(key(KeyCode::Char('y')));
        assert_eq!(effect, Some(Effect::Copy("123".to_string())));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.focus, Focus::Tree);
        assert_eq!(app.view, View::Holders);
    }

    #[test]
    fn holders_action_menu_focuses_holder() {
        let mut app = holders_app();
        app.handle_key(key(KeyCode::Char('a')));
        let effect = app.handle_key(key(KeyCode::Char('f')));
        assert_eq!(effect, Some(Effect::Focus(123)));
        assert!(!app.show_actions);
    }

    #[test]
    fn replace_holders_preserves_selection_by_pid() {
        let mut app = holders_app();
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(456));
        app.replace_holders(vec![proc(456, 1, "code")]);
        assert_eq!(app.selected_info().map(|p| p.pid), Some(456));
        assert_eq!(app.position(), (1, 1));
    }

    fn type_str(app: &mut App, text: &str) {
        for c in text.chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
    }

    fn processes_app() -> App {
        let mut ph = proc(300, 200, "ph");
        ph.exe = Some("/usr/local/bin/ph".to_string());
        ph.command = Some("/usr/local/bin/ph 300".to_string());
        let mut worker = proc(301, 300, "worker");
        worker.command = Some("/usr/bin/worker --daemon".to_string());
        let mut bash = proc(200, 1, "bash");
        bash.command = Some("/usr/bin/bash -l".to_string());
        App::new_processes(vec![
            ph,
            worker,
            bash,
            proc(1, 0, "init"),
            proc(302, 300, "logger"),
        ])
    }

    #[test]
    fn parse_query_extracts_field_prefix() {
        assert_eq!(parse_query("pid:300"), (SearchField::Pid, "300"));
        assert_eq!(parse_query("name:foo"), (SearchField::Name, "foo"));
        assert_eq!(parse_query("cmd:bar"), (SearchField::Cmd, "bar"));
        assert_eq!(parse_query("exe:baz"), (SearchField::Exe, "baz"));
        assert_eq!(parse_query("plain"), (SearchField::Any, "plain"));
    }

    #[test]
    fn field_matches_are_case_insensitive_substrings() {
        let mut p = proc(300, 200, "ph");
        p.command = Some("/usr/bin/Ph 300".to_string());
        p.exe = Some("/usr/local/bin/ph".to_string());
        assert!(field_matches(&p, SearchField::Any, "PH"));
        assert!(field_matches(&p, SearchField::Name, "ph"));
        assert!(field_matches(&p, SearchField::Cmd, "300"));
        assert!(field_matches(&p, SearchField::Exe, "local"));
        assert!(field_matches(&p, SearchField::Pid, "30"));
        assert!(!field_matches(&p, SearchField::Pid, "031"));
        assert!(!field_matches(&p, SearchField::Name, "zzz"));
    }

    #[test]
    fn processes_view_navigates_and_filters_live() {
        let mut app = processes_app();
        assert_eq!(app.view, View::Processes);
        assert_eq!(app.position(), (1, 5));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(200));

        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "work");
        assert_eq!(app.position(), (1, 1));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(301));
        app.handle_key(key(KeyCode::Enter));
        assert!(app.search.is_none());
        assert_eq!(app.position(), (1, 1));
    }

    #[test]
    fn processes_filter_supports_field_prefixes() {
        let mut app = processes_app();
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "pid:30");
        assert_eq!(app.position(), (1, 3));
        app.handle_key(key(KeyCode::Enter));

        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "name:log");
        assert_eq!(app.position(), (1, 1));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(302));
        app.handle_key(key(KeyCode::Enter));

        app.handle_key(key(KeyCode::Esc));
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "exe:local");
        assert_eq!(app.position(), (1, 1));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(300));
    }

    #[test]
    fn esc_in_filter_search_restores_previous_filter() {
        let mut app = processes_app();
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "work");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.position(), (1, 1));

        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "erzzz");
        assert_eq!(app.position(), (0, 0));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.position(), (1, 1));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(301));
    }

    #[test]
    fn esc_in_processes_view_clears_filter() {
        let mut app = processes_app();
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "work");
        app.handle_key(key(KeyCode::Enter));
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.position(), (1, 5));
        assert_eq!(app.status.as_deref(), Some("filter cleared"));
    }

    #[test]
    fn enter_in_processes_view_returns_focus_effect() {
        let mut app = processes_app();
        let effect = app.handle_key(key(KeyCode::Enter));
        assert_eq!(effect, Some(Effect::Focus(1)));
    }

    #[test]
    fn enter_tree_from_processes_and_esc_roundtrip() {
        let mut app = processes_app();
        app.enter_tree_from_processes(&sample_tree());
        assert_eq!(app.view, View::Tree);
        assert_eq!(app.breadcrumb().as_deref(), Some("processes > ph"));
        let effect = app.handle_key(key(KeyCode::Esc));
        assert_eq!(effect, None);
        assert_eq!(app.view, View::Processes);
        assert_eq!(app.breadcrumb(), None);
    }

    #[test]
    fn replace_processes_preserves_selection_by_pid() {
        let mut app = processes_app();
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.selected_info().map(|p| p.pid), Some(200));
        let mut bash = proc(200, 1, "bash");
        bash.command = Some("/usr/bin/bash -l".to_string());
        app.replace_processes(vec![bash, proc(1, 0, "init")]);
        assert_eq!(app.selected_info().map(|p| p.pid), Some(200));
        assert_eq!(app.position(), (2, 2));
    }

    #[test]
    fn tree_search_jumps_to_matches_and_cycles() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "pid:30");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.selected, 2);
        assert_eq!(app.status.as_deref(), Some("match 1/3"));
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.selected, 3);
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.selected, 4);
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.selected, 2);
        app.handle_key(key(KeyCode::Char('N')));
        assert_eq!(app.selected, 4);
    }

    #[test]
    fn tree_search_with_no_matches_keeps_selection() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "zzz");
        app.handle_key(key(KeyCode::Enter));
        assert_eq!(app.selected, 2);
        assert_eq!(app.status.as_deref(), Some("no matches"));
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.status.as_deref(), Some("no active search"));
    }

    #[test]
    fn esc_cancels_pending_jump_search() {
        let mut app = App::new(&sample_tree());
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "worker");
        app.handle_key(key(KeyCode::Esc));
        assert_eq!(app.selected, 2);
        assert!(app.search.is_none());
        app.handle_key(key(KeyCode::Char('n')));
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn search_input_captures_navigation_keys() {
        let mut app = processes_app();
        app.handle_key(key(KeyCode::Char('/')));
        type_str(&mut app, "log");
        assert_eq!(app.position(), (1, 1));
        app.handle_key(key(KeyCode::Char('j')));
        assert_eq!(app.search.as_ref().map(|s| s.query.as_str()), Some("logj"));
        assert_eq!(app.position(), (0, 0));
    }

    #[test]
    fn replace_holders_with_empty_list_is_safe() {
        let mut app = holders_app();
        app.replace_holders(Vec::new());
        assert_eq!(app.position(), (0, 0));
        assert_eq!(app.selected_info(), None);
        assert_eq!(app.handle_key(key(KeyCode::Char('j'))), None);
        assert_eq!(app.handle_key(key(KeyCode::Enter)), None);
        assert_eq!(app.handle_key(key(KeyCode::Char('y'))), None);
        assert_eq!(app.status.as_deref(), Some("nothing to yank"));
        assert!(!app.should_quit);
    }
}
