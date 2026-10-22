use gpui_kit::component::menu::PopupMenu;
use gpui_kit::component::table::{Column, ColumnSort, TableDelegate, TableState};
use gpui_kit::{App, Context, IntoElement, ParentElement as _, SharedString, Window, div, px};
use prochist_core::{Pid, ProcessInfo};

use crate::actions::{CopyCommand, CopyExe, CopyNamePid, CopyPid, FocusProcess};
use crate::state::{display_name, filtered, service_field};

pub const COLUMN_COUNT: usize = 5;

/// Table data for the processes and holders views. Owns the full process set
/// plus the filter/sort state; `rows` is the derived, visible order.
pub struct ProcessTableDelegate {
    all: Vec<ProcessInfo>,
    rows: Vec<ProcessInfo>,
    filter: String,
    sort_col: Option<usize>,
    sort_dir: ColumnSort,
}

impl ProcessTableDelegate {
    pub fn new(all: Vec<ProcessInfo>) -> Self {
        let mut this = Self {
            all,
            rows: Vec::new(),
            filter: String::new(),
            sort_col: Some(0),
            sort_dir: ColumnSort::Ascending,
        };
        this.rebuild();
        this
    }

    pub fn set_all(&mut self, all: Vec<ProcessInfo>) {
        self.all = all;
        self.rebuild();
    }

    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.rebuild();
    }

    pub fn pid_at(&self, row_ix: usize) -> Option<Pid> {
        self.rows.get(row_ix).map(|p| p.pid)
    }

    pub fn row_of(&self, pid: Pid) -> Option<usize> {
        self.rows.iter().position(|p| p.pid == pid)
    }

    pub fn rows(&self) -> &[ProcessInfo] {
        &self.rows
    }

    fn rebuild(&mut self) {
        self.rows = filtered(&self.all, &self.filter).cloned().collect();
        self.apply_sort();
    }

    fn apply_sort(&mut self) {
        let Some(col) = self.sort_col else { return };
        match self.sort_dir {
            ColumnSort::Default => self.rows.sort_by_key(|p| p.pid),
            ColumnSort::Ascending => self.sort_rows(col, false),
            ColumnSort::Descending => self.sort_rows(col, true),
        }
    }

    fn sort_rows(&mut self, col: usize, descending: bool) {
        let cmp = |a: &ProcessInfo, b: &ProcessInfo| match col {
            0 => a.pid.cmp(&b.pid),
            4 => service_field(a)
                .unwrap_or_default()
                .to_lowercase()
                .cmp(&service_field(b).unwrap_or_default().to_lowercase()),
            _ => col_text(a, col)
                .to_lowercase()
                .cmp(&col_text(b, col).to_lowercase()),
        };
        if descending {
            self.rows.sort_by(|a, b| cmp(a, b).reverse());
        } else {
            self.rows.sort_by(cmp);
        }
    }
}

fn col_text(p: &ProcessInfo, col: usize) -> &str {
    match col {
        1 => &p.name,
        2 => p.command.as_deref().unwrap_or(""),
        3 => p.exe.as_deref().unwrap_or(""),
        _ => "",
    }
}

impl TableDelegate for ProcessTableDelegate {
    fn columns_count(&self, _: &App) -> usize {
        COLUMN_COUNT
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, _: &App) -> Column {
        let sort = if self.sort_col == Some(col_ix) {
            self.sort_dir
        } else {
            ColumnSort::Default
        };
        let column = match col_ix {
            0 => Column::new("pid", "PID").width(px(90.)).text_right(),
            1 => Column::new("name", "Name").width(px(220.)),
            2 => Column::new("command", "Command").width(px(420.)),
            3 => Column::new("executable", "Executable").width(px(320.)),
            _ => Column::new("service", "Service").width(px(240.)),
        };
        column.sort(sort)
    }

    fn perform_sort(
        &mut self,
        col_ix: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sort_col = (sort != ColumnSort::Default).then_some(col_ix);
        self.sort_dir = sort;
        self.rebuild();
        cx.notify();
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(info) = self.rows.get(row_ix) else {
            return div().child("");
        };
        let text: SharedString = match col_ix {
            0 => info.pid.to_string().into(),
            1 => display_name(info).into(),
            2 => info.command.clone().unwrap_or_else(|| "-".into()).into(),
            3 => info.exe.clone().unwrap_or_else(|| "-".into()).into(),
            _ => service_field(info).unwrap_or_else(|| "-".into()).into(),
        };
        div().child(text)
    }

    fn context_menu(
        &mut self,
        row_ix: usize,
        menu: PopupMenu,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> PopupMenu {
        let Some(info) = self.rows.get(row_ix) else {
            return menu;
        };
        let pid = info.pid;
        let has_command = info.command.is_some();
        let has_exe = info.exe.is_some();
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
}
