use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io;
use std::path::PathBuf;

use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::list::ListItem;
use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::status_bar::StatusBar;
use gpui_kit::component::table::{DataTable, TableDelegate as _, TableEvent, TableState};
use gpui_kit::component::theme::Theme;
use gpui_kit::component::tree::{TreeEvent, TreeItem, TreeState, tree};
use gpui_kit::component::{ActiveTheme as _, WindowExt as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{TestSupportExt as _, *};
use prochist_core::{Pid, ProcessInfo, ProcessTree, build_tree};

use crate::actions::*;
use crate::help::HELP_LINES;
use crate::source::Source;
use crate::state::{
    FIELD_LABELS, FocusEntry, ProcNode, build_subtree, display_name, expanded_chain, fields,
    find_matches, flatten, resolve_holders,
};
use crate::table::ProcessTableDelegate;

/// Initial data for the window, mirroring the CLI modes.
pub enum Init {
    Tree {
        snapshot: Vec<ProcessInfo>,
        pid: Pid,
    },
    Processes {
        snapshot: Vec<ProcessInfo>,
    },
    Holders {
        snapshot: Vec<ProcessInfo>,
        path: PathBuf,
        holders: Vec<ProcessInfo>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tree,
    Processes,
    Holders,
}

impl View {
    fn tag(self) -> &'static str {
        match self {
            View::Tree => "TREE",
            View::Processes => "PROCESSES",
            View::Holders => "HOLDERS",
        }
    }
}

struct TreeView {
    state: Entity<TreeState>,
    model: ProcNode,
    focus_pid: Pid,
    focus_name: String,
    expanded: HashSet<Pid>,
    flat: Vec<(Pid, usize)>,
    by_pid: HashMap<Pid, ProcessInfo>,
}

/// Failure of a background provider roundtrip.
pub enum FetchError {
    Io(io::Error),
    NotFound(Pid),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::Io(e) => write!(f, "{e}"),
            FetchError::NotFound(pid) => write!(f, "process {pid} no longer exists"),
        }
    }
}

type FetchResult = Result<(Vec<ProcessInfo>, Option<Vec<Pid>>, Option<ProcessTree>), FetchError>;

pub struct Phg {
    source: Source,
    view: View,
    focus_stack: Vec<FocusEntry>,
    snapshot: Vec<ProcessInfo>,
    tree: Option<TreeView>,
    processes: Option<Entity<TableState<ProcessTableDelegate>>>,
    holders: Option<Entity<TableState<ProcessTableDelegate>>>,
    holders_path: Option<PathBuf>,
    search: Entity<InputState>,
    saved_query: String,
    committed_query: String,
    matches: Vec<usize>,
    match_ix: usize,
    detail_field: usize,
    details_focus: FocusHandle,
    status: Option<String>,
    focus_seq: u64,
    _subs: Vec<Subscription>,
}

impl Phg {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, source: Source, init: Init) -> Self {
        Theme::sync_system_appearance(Some(window), cx);

        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Search (pid: name: cmd: exe: service:)")
        });

        let subs = vec![
            cx.subscribe_in(
                &search,
                window,
                |this, _, event: &InputEvent, window, cx| match event {
                    InputEvent::Change => this.on_search_change(cx),
                    InputEvent::PressEnter { .. } => this.on_search_commit(window, cx),
                    _ => {}
                },
            ),
            cx.observe_window_appearance(window, |_, window, cx| {
                Theme::sync_system_appearance(Some(window), cx);
            }),
            cx.observe(&search, |_, _, cx| cx.notify()),
        ];

        let (snapshot, view, pid, holders_path, holders) = match init {
            Init::Tree { snapshot, pid } => (snapshot, View::Tree, Some(pid), None, None),
            Init::Processes { snapshot } => (snapshot, View::Processes, None, None, None),
            Init::Holders {
                snapshot,
                path,
                holders,
            } => (snapshot, View::Holders, None, Some(path), Some(holders)),
        };

        let mut this = Self {
            source,
            view,
            focus_stack: Vec::new(),
            snapshot,
            tree: None,
            processes: None,
            holders: None,
            holders_path,
            search,
            saved_query: String::new(),
            committed_query: String::new(),
            matches: Vec::new(),
            match_ix: 0,
            detail_field: 0,
            details_focus: cx.focus_handle(),
            status: None,
            focus_seq: 0,
            _subs: subs,
        };

        match view {
            View::Tree => {
                let pid = pid.expect("tree init carries a pid");
                let tree = build_tree(&this.snapshot, pid).expect("pid validated at startup");
                this.rebuild_tree(pid, &tree, true, cx);
            }
            View::Processes => {
                let table = this.make_table(this.snapshot.clone(), window, cx);
                this.processes = Some(table);
            }
            View::Holders => {
                let table = this.make_table(holders.unwrap_or_default(), window, cx);
                this.holders = Some(table);
            }
        }

        cx.defer_in(window, |this, window, cx| this.focus_main(window, cx));
        this
    }

    // --- entity wiring -----------------------------------------------------

    fn make_table(
        &mut self,
        all: Vec<ProcessInfo>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<TableState<ProcessTableDelegate>> {
        let table = cx.new(|cx| {
            TableState::new(ProcessTableDelegate::new(all), window, cx)
                .sortable(true)
                .row_selectable(true)
        });
        if table.read(cx).delegate().rows_count(cx) > 0 {
            table.update(cx, |t, cx| t.set_selected_row(0, cx));
        }
        self._subs.push(cx.observe(&table, |_, _, cx| cx.notify()));
        self._subs.push(cx.subscribe_in(
            &table,
            window,
            |this, table, event: &TableEvent, window, cx| {
                if let TableEvent::DoubleClickedRow(ix) = event
                    && let Some(pid) = table.read(cx).delegate().pid_at(*ix)
                {
                    this.focus_pid(pid, window, cx);
                }
            },
        ));
        table
    }

    fn to_tree_items(node: &ProcNode, expanded: &HashSet<Pid>) -> TreeItem {
        let pid = node.info.pid;
        TreeItem::new(
            pid.to_string(),
            format!("{} ({})", display_name(&node.info), pid),
        )
        .expanded(expanded.contains(&pid))
        .children(
            node.children
                .iter()
                .map(|child| Self::to_tree_items(child, expanded)),
        )
    }

    /// (Re)builds the tree view around `focus_pid` from the current snapshot.
    /// `tree` must be the `build_tree` result for the same snapshot and pid.
    fn rebuild_tree(
        &mut self,
        focus_pid: Pid,
        tree: &ProcessTree,
        fresh_expansion: bool,
        cx: &mut Context<Self>,
    ) {
        let model = build_subtree(&self.snapshot, focus_pid).expect("build_tree succeeded");
        let expanded = match (fresh_expansion, self.tree.as_ref()) {
            (false, Some(old)) => old.expanded.clone(),
            _ => expanded_chain(&self.snapshot, focus_pid),
        };
        let flat = flatten(&model, &expanded);
        let items = vec![Self::to_tree_items(&model, &expanded)];
        let by_pid: HashMap<Pid, ProcessInfo> =
            self.snapshot.iter().map(|p| (p.pid, p.clone())).collect();
        let focus_name = display_name(&tree.current);

        match self.tree.as_mut() {
            Some(view) => {
                view.model = model;
                view.focus_pid = focus_pid;
                view.focus_name = focus_name;
                view.expanded = expanded;
                view.flat = flat;
                view.by_pid = by_pid;
                view.state
                    .update(cx, |state, cx| state.set_items(items, cx));
            }
            None => {
                let state = cx.new(|cx| TreeState::new(cx).items(items));
                self._subs.push(cx.observe(&state, |_, _, cx| cx.notify()));
                self._subs
                    .push(cx.subscribe(&state, |this, _, event: &TreeEvent, cx| {
                        if let Some(view) = this.tree.as_mut() {
                            match event {
                                TreeEvent::Expanded(id) => {
                                    if let Ok(pid) = id.parse() {
                                        view.expanded.insert(pid);
                                    }
                                }
                                TreeEvent::Collapsed(id) => {
                                    if let Ok(pid) = id.parse() {
                                        view.expanded.remove(&pid);
                                    }
                                }
                            }
                            view.flat = flatten(&view.model, &view.expanded);
                        }
                        cx.notify();
                    }));
                self.tree = Some(TreeView {
                    state,
                    model,
                    focus_pid,
                    focus_name,
                    expanded,
                    flat,
                    by_pid,
                });
            }
        }

        // Re-select and reveal the focused process (set_items resets selection).
        self.select_tree_pid(focus_pid, cx);
    }

    fn select_tree_pid(&mut self, pid: Pid, cx: &mut Context<Self>) {
        let Some(view) = self.tree.as_ref() else {
            return;
        };
        let id: SharedString = pid.to_string().into();
        view.state.update(cx, |state, cx| {
            if let Some(ix) = state.index_of(&id) {
                state.set_selected_index(Some(ix), cx);
                state.scroll_to_item(ix, ScrollStrategy::Center);
            }
        });
    }

    // --- view helpers ------------------------------------------------------

    fn active_table(&self) -> Option<&Entity<TableState<ProcessTableDelegate>>> {
        match self.view {
            View::Processes => self.processes.as_ref(),
            View::Holders => self.holders.as_ref(),
            View::Tree => None,
        }
    }

    // --- test-facing accessors ------------------------------------------------

    #[cfg(test)]
    pub(crate) fn view_kind(&self) -> View {
        self.view
    }

    #[cfg(test)]
    pub(crate) fn tree_focus_pid(&self) -> Option<Pid> {
        self.tree.as_ref().map(|t| t.focus_pid)
    }

    pub(crate) fn selected_pid(&self, cx: &App) -> Option<Pid> {
        match self.view {
            View::Tree => {
                let view = self.tree.as_ref()?;
                let ix = view.state.read(cx).selected_index()?;
                view.flat.get(ix).map(|(pid, _)| *pid)
            }
            _ => {
                let table = self.active_table()?;
                let ix = table.read(cx).selected_row()?;
                table.read(cx).delegate().pid_at(ix)
            }
        }
    }

    fn info_for(&self, pid: Pid) -> Option<&ProcessInfo> {
        self.snapshot.iter().find(|p| p.pid == pid)
    }

    fn selected_info(&self, cx: &App) -> Option<ProcessInfo> {
        let pid = self.selected_pid(cx)?;
        self.info_for(pid).cloned()
    }

    pub(crate) fn position(&self, cx: &App) -> (usize, usize) {
        match self.view {
            View::Tree => {
                let Some(view) = self.tree.as_ref() else {
                    return (0, 0);
                };
                let ix = view.state.read(cx).selected_index().map_or(0, |ix| ix + 1);
                (ix, view.flat.len())
            }
            _ => {
                let Some(table) = self.active_table() else {
                    return (0, 0);
                };
                let state = table.read(cx);
                (
                    state.selected_row().map_or(0, |ix| ix + 1),
                    state.delegate().rows_count(cx),
                )
            }
        }
    }

    pub(crate) fn breadcrumb(&self) -> String {
        if self.focus_stack.is_empty() {
            return String::new();
        }
        let mut parts: Vec<String> = self
            .focus_stack
            .iter()
            .map(|entry| match entry {
                FocusEntry::Process(_, name) => name.clone(),
                FocusEntry::Holders => self
                    .holders_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "Holders".into()),
                FocusEntry::Processes => "Processes".into(),
            })
            .collect();
        let current = match self.view {
            View::Tree => self
                .tree
                .as_ref()
                .map(|t| t.focus_name.clone())
                .unwrap_or_default(),
            View::Processes => "Processes".into(),
            View::Holders => self
                .holders_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        };
        parts.push(current);
        parts.join(" > ")
    }

    // --- focus / restore ---------------------------------------------------

    pub(crate) fn focus_pid(&mut self, pid: Pid, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.clone();
        self.focus_seq += 1;
        let seq = self.focus_seq;
        self.status = Some(format!("Focusing pid {pid}…"));
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let bg = cx.background_executor().clone();
            let result = bg
                .spawn(async move {
                    let snapshot = source.snapshot().map_err(FetchError::Io)?;
                    let tree = build_tree(&snapshot, pid).map_err(|_| FetchError::NotFound(pid))?;
                    Ok::<_, FetchError>((snapshot, None::<Vec<Pid>>, Some(tree)))
                })
                .await;
            this.update_in(cx, |this, _, cx| {
                // A newer focus/restore superseded this request while it ran.
                if this.focus_seq == seq {
                    this.apply_focus(pid, result, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    fn apply_focus(&mut self, pid: Pid, result: FetchResult, cx: &mut Context<Self>) {
        match result {
            Err(e) => {
                self.status = Some(format!("phg: {e}"));
            }
            Ok((snapshot, _, Some(tree))) => {
                let entry = match self.view {
                    View::Tree => FocusEntry::Process(
                        self.tree.as_ref().map_or(pid, |t| t.focus_pid),
                        self.tree
                            .as_ref()
                            .map(|t| t.focus_name.clone())
                            .unwrap_or_default(),
                    ),
                    View::Processes => FocusEntry::Processes,
                    View::Holders => FocusEntry::Holders,
                };
                self.focus_stack.push(entry);
                self.install_snapshot(snapshot, None, cx);
                self.rebuild_tree(pid, &tree, true, cx);
                self.view = View::Tree;
                self.matches.clear();
                self.status = None;
            }
            _ => unreachable!("focus fetch always carries a tree"),
        }
        cx.notify();
    }

    pub(crate) fn restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(entry) = self.focus_stack.last().cloned() else {
            self.status = Some("Already at the top level".into());
            cx.notify();
            return;
        };
        self.focus_seq += 1;
        match entry {
            FocusEntry::Processes => {
                self.focus_stack.pop();
                self.view = View::Processes;
                self.matches.clear();
                self.status = None;
                cx.notify();
            }
            FocusEntry::Holders => {
                self.focus_stack.pop();
                self.view = View::Holders;
                self.matches.clear();
                self.status = None;
                cx.notify();
            }
            FocusEntry::Process(pid, _) => {
                self.focus_pid_for_restore(pid, window, cx);
            }
        }
    }

    fn focus_pid_for_restore(&mut self, pid: Pid, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let seq = self.focus_seq;
        cx.spawn_in(window, async move |this, cx| {
            let bg = cx.background_executor().clone();
            let result = bg
                .spawn(async move {
                    let snapshot = source.snapshot().map_err(FetchError::Io)?;
                    let tree = build_tree(&snapshot, pid).map_err(|_| FetchError::NotFound(pid))?;
                    Ok::<_, FetchError>((snapshot, None::<Vec<Pid>>, Some(tree)))
                })
                .await;
            this.update_in(cx, |this, _window, cx| {
                // A newer focus/restore superseded this request while it ran.
                if this.focus_seq != seq {
                    return;
                }
                match result {
                    Ok((snapshot, _, Some(tree))) => {
                        // Only now consume the history entry, so a failed or
                        // superseded restore leaves it available for retry.
                        this.focus_stack.pop();
                        this.install_snapshot(snapshot, None, cx);
                        this.rebuild_tree(pid, &tree, true, cx);
                        this.view = View::Tree;
                        this.status = None;
                    }
                    Ok(_) => unreachable!(),
                    Err(e) => {
                        this.status = Some(format!("phg: {e}"));
                    }
                }
            })
            .ok();
        })
        .detach();
    }

    fn show_all_processes(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == View::Processes {
            return;
        }
        let entry = match self.view {
            View::Tree => FocusEntry::Process(
                self.tree.as_ref().map_or(0, |t| t.focus_pid),
                self.tree
                    .as_ref()
                    .map(|t| t.focus_name.clone())
                    .unwrap_or_default(),
            ),
            View::Holders => FocusEntry::Holders,
            View::Processes => unreachable!(),
        };
        self.focus_stack.push(entry);
        self.focus_seq += 1;
        if self.processes.is_none() {
            let table = self.make_table(self.snapshot.clone(), window, cx);
            self.processes = Some(table);
        }
        self.view = View::Processes;
        self.matches.clear();
        cx.notify();
    }

    /// Installs `snapshot` and syncs every cached table with it, preserving
    /// each table's selection by PID. Holder rows are re-resolved against the
    /// new snapshot — from `holder_pids` when freshly fetched, otherwise from
    /// the PIDs the holders table currently holds.
    fn install_snapshot(
        &mut self,
        snapshot: Vec<ProcessInfo>,
        holder_pids: Option<Vec<Pid>>,
        cx: &mut Context<Self>,
    ) {
        self.snapshot = snapshot;
        if let Some(table) = self.processes.clone() {
            let all = self.snapshot.clone();
            table.update(cx, |t, cx| {
                let selected = t.selected_row().and_then(|ix| t.delegate().pid_at(ix));
                t.delegate_mut().set_all(all);
                Self::reselect(t, selected, cx);
            });
        }
        if let Some(table) = self.holders.clone() {
            let snapshot = self.snapshot.clone();
            table.update(cx, move |t, cx| {
                let selected = t.selected_row().and_then(|ix| t.delegate().pid_at(ix));
                let pids = match &holder_pids {
                    Some(pids) => pids.clone(),
                    None => t.delegate().all().iter().map(|p| p.pid).collect(),
                };
                t.delegate_mut().set_all(resolve_holders(&snapshot, &pids));
                Self::reselect(t, selected, cx);
            });
        }
    }

    // --- refresh ------------------------------------------------------------

    fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let view = self.view;
        let focus_pid = self.tree.as_ref().map(|t| t.focus_pid);
        let holders_path = self.holders_path.clone();
        self.focus_seq += 1;
        let seq = self.focus_seq;
        self.status = Some("Refreshing…".into());
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let bg = cx.background_executor().clone();
            let result = bg
                .spawn(async move {
                    let snapshot = source.snapshot().map_err(FetchError::Io)?;
                    let holders = match (&view, &holders_path) {
                        (View::Holders, Some(path)) => {
                            Some(source.holders(path).map_err(FetchError::Io)?)
                        }
                        _ => None,
                    };
                    let tree = match (&view, focus_pid) {
                        (View::Tree, Some(pid)) => Some(
                            build_tree(&snapshot, pid).map_err(|_| FetchError::NotFound(pid))?,
                        ),
                        _ => None,
                    };
                    Ok::<_, FetchError>((snapshot, holders, tree))
                })
                .await;
            this.update_in(cx, move |this, _, cx| {
                this.apply_refresh(seq, view, focus_pid, result, cx);
            })
            .ok();
        })
        .detach();
    }

    fn apply_refresh(
        &mut self,
        seq: u64,
        view: View,
        focus_pid: Option<Pid>,
        result: FetchResult,
        cx: &mut Context<Self>,
    ) {
        // Discard a stale refresh: a newer refresh/focus superseded it,
        // or the view or focus changed while it ran.
        let current_focus = self.tree.as_ref().map(|t| t.focus_pid);
        if self.focus_seq != seq || self.view != view || current_focus != focus_pid {
            return;
        }
        let (snapshot, holders, tree) = match result {
            Ok(data) => data,
            Err(e) => {
                self.status = Some(format!("phg: {e}"));
                cx.notify();
                return;
            }
        };
        self.install_snapshot(snapshot, holders, cx);
        if self.view == View::Tree
            && let Some(tree) = tree
        {
            let selected = self.selected_pid(cx);
            let focus_pid = tree.current.pid;
            self.rebuild_tree(focus_pid, &tree, false, cx);
            if let Some(pid) = selected {
                self.select_tree_pid(pid, cx);
            }
        }
        self.matches.clear();
        self.status = None;
        if matches!(self.view, View::Tree | View::Holders) && !self.committed_query.is_empty() {
            self.recompute_matches(cx);
        }
        cx.notify();
    }

    fn reselect(
        table: &mut TableState<ProcessTableDelegate>,
        pid: Option<Pid>,
        cx: &mut Context<TableState<ProcessTableDelegate>>,
    ) {
        let row = pid.and_then(|pid| table.delegate().row_of(pid));
        match row {
            Some(ix) => table.set_selected_row(ix, cx),
            None if table.delegate().rows_count(cx) > 0 => table.set_selected_row(0, cx),
            None => table.clear_selection(cx),
        }
    }

    // --- movement ------------------------------------------------------------

    fn move_by(&mut self, delta: isize, cx: &mut Context<Self>) {
        match self.view {
            View::Tree => {
                let Some(view) = self.tree.as_ref() else {
                    return;
                };
                let len = view.flat.len();
                if len == 0 {
                    return;
                }
                let state = view.state.clone();
                state.update(cx, |state, cx| {
                    let ix = state.selected_index().unwrap_or(0) as isize + delta;
                    let ix = ix.clamp(0, len as isize - 1) as usize;
                    state.set_selected_index(Some(ix), cx);
                    state.scroll_to_item(ix, ScrollStrategy::Nearest);
                });
            }
            _ => {
                let Some(table) = self.active_table().cloned() else {
                    return;
                };
                table.update(cx, |t, cx| {
                    let len = t.delegate().rows_count(cx);
                    if len == 0 {
                        return;
                    }
                    let ix = t.selected_row().unwrap_or(0) as isize + delta;
                    t.set_selected_row(ix.clamp(0, len as isize - 1) as usize, cx);
                });
            }
        }
        cx.notify();
    }

    fn move_to(&mut self, ix: usize, cx: &mut Context<Self>) {
        match self.view {
            View::Tree => {
                let Some(view) = self.tree.as_ref() else {
                    return;
                };
                let last = view.flat.len().saturating_sub(1);
                let ix = ix.min(last);
                let state = view.state.clone();
                state.update(cx, |state, cx| {
                    state.set_selected_index(Some(ix), cx);
                    state.scroll_to_item(ix, ScrollStrategy::Nearest);
                });
            }
            _ => {
                let Some(table) = self.active_table().cloned() else {
                    return;
                };
                table.update(cx, |t, cx| {
                    let last = t.delegate().rows_count(cx).saturating_sub(1);
                    t.set_selected_row(ix.min(last), cx);
                });
            }
        }
        cx.notify();
    }

    fn page_size(&self, cx: &App) -> isize {
        if let Some(table) = self.active_table() {
            let range = table.read(cx).visible_range().rows().clone();
            let n = range.end.saturating_sub(range.start);
            if n > 0 {
                return n as isize;
            }
        }
        10
    }

    // --- search ---------------------------------------------------------------

    fn on_search_change(&mut self, cx: &mut Context<Self>) {
        if self.view != View::Processes {
            return;
        }
        let query = self.search.read(cx).value().to_string();
        let Some(table) = self.processes.clone() else {
            return;
        };
        table.update(cx, |t, cx| {
            t.delegate_mut().set_filter(query);
            let len = t.delegate().rows_count(cx);
            match t.selected_row() {
                Some(ix) if ix < len => {}
                _ if len > 0 => t.set_selected_row(0, cx),
                _ => t.clear_selection(cx),
            }
            cx.notify();
        });
        cx.notify();
    }

    fn on_search_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.view {
            View::Processes => {
                self.status = None;
            }
            View::Tree | View::Holders => {
                self.committed_query = self.search.read(cx).value().to_string();
                self.apply_matches(cx);
            }
        }
        self.focus_main(window, cx);
        cx.notify();
    }

    fn collect_match_rows(&self, cx: &App) -> Vec<ProcessInfo> {
        match self.view {
            View::Tree => self
                .tree
                .as_ref()
                .map(|t| {
                    t.flat
                        .iter()
                        .filter_map(|(pid, _)| t.by_pid.get(pid).cloned())
                        .collect()
                })
                .unwrap_or_default(),
            _ => self
                .holders
                .as_ref()
                .map(|t| t.read(cx).delegate().rows().to_vec())
                .unwrap_or_default(),
        }
    }

    fn apply_matches(&mut self, cx: &mut Context<Self>) {
        let query = self.committed_query.clone();
        let rows = self.collect_match_rows(cx);
        self.matches = find_matches(&rows, &query);
        self.match_ix = 0;
        if self.matches.is_empty() {
            self.status = Some(if query.is_empty() {
                String::new()
            } else {
                format!("No matches for `{query}`")
            });
        } else {
            self.goto_match(cx);
        }
    }

    /// Recomputes matches after a refresh without moving a still-present
    /// selection: when the selected row is still a match, `match_ix` points
    /// at it; otherwise selection moves to the first match.
    fn recompute_matches(&mut self, cx: &mut Context<Self>) {
        let selected_row = match self.view {
            View::Tree => self
                .tree
                .as_ref()
                .and_then(|v| v.state.read(cx).selected_index()),
            _ => self.active_table().and_then(|t| t.read(cx).selected_row()),
        };
        let rows = self.collect_match_rows(cx);
        self.matches = find_matches(&rows, &self.committed_query.clone());
        match selected_row.and_then(|row| self.matches.iter().position(|&ix| ix == row)) {
            Some(pos) => {
                self.match_ix = pos;
                self.status = Some(format!("Match {}/{}", pos + 1, self.matches.len()));
            }
            None => self.apply_matches(cx),
        }
    }

    fn goto_match(&mut self, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            return;
        }
        let row_ix = self.matches[self.match_ix];
        match self.view {
            View::Tree => self.move_to(row_ix, cx),
            _ => {
                if let Some(table) = self.active_table().cloned() {
                    table.update(cx, |t, cx| t.set_selected_row(row_ix, cx));
                }
            }
        }
        self.status = Some(format!(
            "Match {}/{}",
            self.match_ix + 1,
            self.matches.len()
        ));
    }

    fn cycle_match(&mut self, delta: isize, cx: &mut Context<Self>) {
        if self.matches.is_empty() {
            return;
        }
        let len = self.matches.len() as isize;
        self.match_ix = ((self.match_ix as isize + delta).rem_euclid(len)) as usize;
        self.goto_match(cx);
    }

    pub(crate) fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.saved_query = self.search.read(cx).value().to_string();
        self.search.update(cx, |input, cx| input.focus(window, cx));
    }

    fn cancel_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let saved = self.saved_query.clone();
        if self.view == View::Processes
            && let Some(table) = self.processes.clone()
        {
            let saved = saved.clone();
            table.update(cx, |t, cx| {
                t.delegate_mut().set_filter(saved);
                cx.notify();
            });
        }
        self.search.update(cx, |input, cx| {
            input.set_value(saved, window, cx);
        });
        self.focus_main(window, cx);
        cx.notify();
    }

    // --- panes ------------------------------------------------------------------

    fn focus_main(&self, window: &mut Window, cx: &mut App) {
        match self.view {
            View::Tree => {
                if let Some(view) = self.tree.as_ref() {
                    view.state.update(cx, |state, cx| state.focus(window, cx));
                }
            }
            _ => {
                if let Some(table) = self.active_table() {
                    let handle = table.read(cx).focus_handle(cx);
                    window.focus(&handle, cx);
                }
            }
        }
    }

    fn focus_details(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.details_focus, cx);
    }

    // --- copy ---------------------------------------------------------------------

    fn copy(&mut self, text: String, what: &str, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.status = Some(format!("Copied {what}"));
        cx.notify();
    }

    fn yank_name_pid(&mut self, pid: Pid, cx: &mut Context<Self>) {
        match self.info_for(pid) {
            Some(info) => {
                let text = format!("{} ({})", display_name(info), pid);
                self.copy(text, "name (pid)", cx);
            }
            None => {
                self.status = Some(format!("phg: unknown pid {pid}"));
                cx.notify();
            }
        }
    }

    fn yank_command(&mut self, pid: Pid, cx: &mut Context<Self>) {
        match self.info_for(pid) {
            Some(ProcessInfo {
                command: Some(command),
                ..
            }) => {
                let text = command.clone();
                self.copy(text, "command line", cx);
            }
            _ => {
                self.status = Some(format!("No command line for pid {pid}"));
                cx.notify();
            }
        }
    }

    fn yank_exe(&mut self, pid: Pid, cx: &mut Context<Self>) {
        match self.info_for(pid) {
            Some(ProcessInfo { exe: Some(exe), .. }) => {
                let text = exe.clone();
                self.copy(text, "executable path", cx);
            }
            _ => {
                self.status = Some(format!("No executable path for pid {pid}"));
                cx.notify();
            }
        }
    }

    fn yank_field(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.selected_info(cx) else {
            return;
        };
        match &fields(&info)[self.detail_field] {
            Some(value) => {
                let text = value.clone();
                self.copy(text, FIELD_LABELS[self.detail_field], cx);
            }
            None => {
                self.status = Some(format!("{} is empty", FIELD_LABELS[self.detail_field]));
                cx.notify();
            }
        }
    }

    // --- help -----------------------------------------------------------------

    fn toggle_help(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if window.has_active_dialog(cx) {
            window.close_dialog(cx);
            return;
        }
        window.open_dialog(cx, |dialog, _, _| {
            let mut body = div().id("help-body").flex().flex_col().overflow_y_scroll();
            for line in HELP_LINES {
                body = body.child(div().text_xs().child(*line));
            }
            dialog
                .title("phg — help")
                .close_button(true)
                .overlay_closable(true)
                .w(px(560.))
                .max_w(px(560.))
                .child(body)
        });
    }

    // --- render ----------------------------------------------------------------

    fn render_toolbar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .key_context("PhgSearch")
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_2()
            .border_b_1()
            .border_color(theme.border)
            .on_action(cx.listener(|this, _: &CancelSearch, window, cx| {
                this.cancel_search(window, cx);
            }))
            .child(
                Button::new("view-all")
                    .outline()
                    .label("All Processes")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.show_all_processes(window, cx);
                    })),
            )
            .child(
                Button::new("back")
                    .outline()
                    .label("Back")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.restore(window, cx);
                    })),
            )
            .child(div().w(px(320.)).child(Input::new(&self.search)))
            .when(self.view != View::Processes, |this| {
                this.child(
                    Button::new("prev-match")
                        .outline()
                        .label("Prev")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cycle_match(-1, cx);
                        })),
                )
                .child(
                    Button::new("next-match")
                        .outline()
                        .label("Next")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.cycle_match(1, cx);
                        })),
                )
            })
            .child(
                Button::new("refresh")
                    .outline()
                    .label("Refresh")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.refresh(window, cx);
                    })),
            )
            .child(div().flex_1())
            .child(
                Button::new("help")
                    .outline()
                    .label("? Help")
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.toggle_help(window, cx);
                    })),
            )
    }

    fn render_main(&mut self, _cx: &mut Context<Self>) -> AnyElement {
        match self.view {
            View::Tree => {
                let Some(view) = self.tree.as_ref() else {
                    return div().child("No process tree").into_any_element();
                };
                let current_pid = view.focus_pid;
                tree(&view.state, move |_ix, entry, _selected, _window, cx| {
                    let pid: Pid = entry.item().id.parse().unwrap_or(0);
                    let is_current = pid == current_pid;
                    let label = entry.item().label.clone();
                    let theme = cx.theme();
                    ListItem::new(("tree-item", pid as usize))
                        .pl(px(10.) + px(18.) * entry.depth() as f32)
                        .child(
                            div()
                                .when(is_current, |this| {
                                    this.font_weight(gpui_kit::FontWeight::BOLD)
                                        .text_color(theme.accent)
                                })
                                .child(label),
                        )
                        .on_click(move |event, window, cx| {
                            if event.click_count() == 2 && event.standard_click() {
                                window.dispatch_action(Box::new(FocusProcess { pid }), cx);
                            }
                        })
                })
                .context_menu(|_ix, entry, menu, _window, _cx| {
                    let pid: Pid = entry.item().id.parse().unwrap_or(0);
                    Self::process_menu(menu, pid, true, true)
                })
                .into_any_element()
            }
            View::Processes | View::Holders => {
                let Some(table) = self.active_table().cloned() else {
                    return div().child("No processes").into_any_element();
                };
                div()
                    .size_full()
                    .child(DataTable::new(&table).stripe(true))
                    .into_any_element()
            }
        }
    }

    fn process_menu(menu: PopupMenu, pid: Pid, has_command: bool, has_exe: bool) -> PopupMenu {
        menu.menu("Focus this process", Box::new(FocusProcess { pid }))
            .separator()
            .menu("Copy name (pid)", Box::new(CopyNamePid { pid }))
            .menu_with_enable(
                "Copy command line",
                Box::new(CopyCommand { pid }),
                has_command,
            )
            .menu("Copy PID", Box::new(CopyPid { pid }))
            .menu_with_enable("Copy executable path", Box::new(CopyExe { pid }), has_exe)
    }

    fn render_details(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let info = self.selected_info(cx);
        let focused = self.details_focus.is_focused(window);
        let field_values: Vec<Option<String>> = info
            .as_ref()
            .map_or_else(|| vec![None; FIELD_LABELS.len()], fields);

        let mut list = div().flex().flex_col().py_1();
        for (ix, (label, value)) in FIELD_LABELS
            .iter()
            .zip(field_values.iter())
            .enumerate()
            .take(FIELD_LABELS.len())
        {
            let selected = focused && ix == self.detail_field;
            let value = value.clone().unwrap_or_else(|| "-".into());
            list = list.child(
                div()
                    .id(("detail", ix))
                    .test_support()
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(22.))
                    .px_2()
                    .rounded_sm()
                    .when(selected, |this| this.bg(theme.list_active))
                    .hover(|this| this.bg(theme.list_hover))
                    .child(
                        div()
                            .w(px(90.))
                            .text_color(theme.muted_foreground)
                            .child(*label),
                    )
                    .child(div().flex_1().overflow_hidden().child(value))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.detail_field = ix;
                        this.yank_field(cx);
                    })),
            );
        }

        div()
            .id("details")
            .test_support()
            .key_context("PhgDetails")
            .track_focus(&self.details_focus)
            .border_t_1()
            .border_color(theme.border)
            .h(px(170.))
            .flex_none()
            .overflow_hidden()
            .px_3()
            .py_1()
            .on_action(cx.listener(|this, _: &DetailDown, _, cx| {
                this.detail_field = (this.detail_field + 1).min(FIELD_LABELS.len() - 1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &DetailUp, _, cx| {
                this.detail_field = this.detail_field.saturating_sub(1);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &YankField, _, cx| {
                this.yank_field(cx);
            }))
            .on_action(cx.listener(|this, _: &FocusMain, window, cx| {
                this.focus_main(window, cx);
            }))
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("Details — click a field to copy"),
            )
            .child(list)
    }

    fn render_status(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (pos, total) = self.position(cx);
        let tag = self.view.tag();
        let message = self.status.clone().unwrap_or_else(|| {
            "right-click: actions · double-click: focus · /: search · ?: help".into()
        });
        let breadcrumb = self.breadcrumb();
        StatusBar::new()
            .left(format!("{tag} {pos}/{total}"))
            .child(message)
            .right(breadcrumb)
    }
}

impl Render for Phg {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(self.render_toolbar(cx))
            .child(
                div()
                    .key_context("PhgMain")
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .on_action(cx.listener(|this, _: &MoveDown, _, cx| this.move_by(1, cx)))
                    .on_action(cx.listener(|this, _: &MoveUp, _, cx| this.move_by(-1, cx)))
                    .on_action(cx.listener(|this, _: &HalfPageDown, _, cx| {
                        let n = (this.page_size(cx) / 2).max(1);
                        this.move_by(n, cx);
                    }))
                    .on_action(cx.listener(|this, _: &HalfPageUp, _, cx| {
                        let n = (this.page_size(cx) / 2).max(1);
                        this.move_by(-n, cx);
                    }))
                    .on_action(cx.listener(|this, _: &FullPageDown, _, cx| {
                        let n = this.page_size(cx).max(1);
                        this.move_by(n, cx);
                    }))
                    .on_action(cx.listener(|this, _: &FullPageUp, _, cx| {
                        let n = this.page_size(cx).max(1);
                        this.move_by(-n, cx);
                    }))
                    .on_action(cx.listener(|this, _: &MoveStart, _, cx| this.move_to(0, cx)))
                    .on_action(cx.listener(|this, _: &MoveEnd, _, cx| {
                        this.move_to(usize::MAX, cx);
                    }))
                    .on_action(cx.listener(|this, _: &FocusSelected, window, cx| {
                        if let Some(pid) = this.selected_pid(cx) {
                            this.focus_pid(pid, window, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, _: &RestoreFocus, window, cx| {
                        this.restore(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &FocusSearch, window, cx| {
                        this.focus_search(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &NextMatch, _, cx| {
                        this.cycle_match(1, cx);
                    }))
                    .on_action(cx.listener(|this, _: &PrevMatch, _, cx| {
                        this.cycle_match(-1, cx);
                    }))
                    .on_action(cx.listener(|this, _: &Refresh, window, cx| {
                        this.refresh(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &ToggleHelp, window, cx| {
                        this.toggle_help(window, cx);
                    }))
                    .on_action(cx.listener(|_this, _: &QuitApp, _, cx| {
                        cx.quit();
                    }))
                    .on_action(cx.listener(|this, _: &FocusDetails, window, cx| {
                        this.focus_details(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &ShowAllProcesses, window, cx| {
                        this.show_all_processes(window, cx);
                    }))
                    .on_action(cx.listener(|this, _: &YankNamePid, _, cx| {
                        if let Some(pid) = this.selected_pid(cx) {
                            this.yank_name_pid(pid, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, _: &YankCommand, _, cx| {
                        if let Some(pid) = this.selected_pid(cx) {
                            this.yank_command(pid, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, action: &FocusProcess, window, cx| {
                        this.focus_pid(action.pid, window, cx);
                    }))
                    .on_action(cx.listener(|this, action: &CopyNamePid, _, cx| {
                        this.yank_name_pid(action.pid, cx);
                    }))
                    .on_action(cx.listener(|this, action: &CopyCommand, _, cx| {
                        this.yank_command(action.pid, cx);
                    }))
                    .on_action(cx.listener(|this, action: &CopyPid, _, cx| {
                        let text = action.pid.to_string();
                        this.copy(text, "PID", cx);
                    }))
                    .on_action(cx.listener(|this, action: &CopyExe, _, cx| {
                        this.yank_exe(action.pid, cx);
                    }))
                    .child(self.render_main(cx)),
            )
            .child(self.render_details(window, cx))
            .child(self.render_status(cx))
    }
}
