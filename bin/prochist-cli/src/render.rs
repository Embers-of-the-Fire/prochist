use std::fmt::Write as _;

use prochist_core::{ProcessInfo, ProcessTree};

fn node(out: &mut String, indent: usize, p: &ProcessInfo, is_last: bool) {
    let base = "    ".repeat(indent);
    let connector = if is_last { "└── " } else { "├── " };
    let _ = writeln!(out, "{base}{connector}{} ({})", p.name, p.pid);
    if let Some(command) = &p.command {
        let continuation = if is_last { "    " } else { "│   " };
        let _ = writeln!(out, "{base}{continuation}{command}");
    }
}

pub fn render(tree: &ProcessTree) -> String {
    let mut out = String::new();
    for ancestor in &tree.ancestors {
        node(&mut out, 0, ancestor, false);
    }
    node(&mut out, 0, &tree.current, true);
    let last = tree.children.len().saturating_sub(1);
    for (i, child) in tree.children.iter().enumerate() {
        node(&mut out, 1, child, i == last);
    }
    out
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
        }
    }

    #[test]
    fn renders_layout_with_command_continuations() {
        let mut worker = proc(301, 300, "worker");
        worker.command = Some("/usr/bin/worker --daemon".to_string());
        let tree = ProcessTree {
            ancestors: vec![proc(1, 0, "init"), proc(200, 1, "bash")],
            current: proc(300, 200, "ph"),
            children: vec![worker, proc(302, 300, "logger")],
        };
        let expected = "├── init (1)\n├── bash (200)\n└── ph (300)\n    ├── worker (301)\n    │   /usr/bin/worker --daemon\n    └── logger (302)\n";
        assert_eq!(render(&tree), expected);
    }

    #[test]
    fn shows_current_command_line() {
        let mut current = proc(300, 200, "ph");
        current.command = Some("target/debug/ph 300".to_string());
        let tree = ProcessTree {
            ancestors: vec![],
            current,
            children: vec![],
        };
        assert_eq!(render(&tree), "└── ph (300)\n    target/debug/ph 300\n");
    }
}
