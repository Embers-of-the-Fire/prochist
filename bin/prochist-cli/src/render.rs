use std::fmt::Write as _;
use std::path::Path;

use prochist_core::{ProcessInfo, ProcessRole, ProcessTree};

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    pub ascii: bool,
    pub long: bool,
    pub executable: bool,
    pub max_ancestors: Option<usize>,
    pub max_children: Option<usize>,
}

struct Glyphs {
    branch: &'static str,
    last: &'static str,
    pipe: &'static str,
    blank: &'static str,
}

const UNICODE: Glyphs = Glyphs {
    branch: "├── ",
    last: "└── ",
    pipe: "│   ",
    blank: "    ",
};

const ASCII: Glyphs = Glyphs {
    branch: "|-- ",
    last: "+-- ",
    pipe: "|   ",
    blank: "    ",
};

fn name_of(p: &ProcessInfo, executable: bool) -> String {
    match &p.exe {
        Some(exe) if executable => exe.clone(),
        Some(exe) => Path::new(exe)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.name.clone()),
        None => p.name.clone(),
    }
}

fn role_tag(p: &ProcessInfo) -> Option<String> {
    match &p.role {
        Some(ProcessRole::WindowsService { services }) => {
            let first = services.first()?;
            let mut tag = format!("[[WinService]] {}", first.name);
            if services.len() > 1 {
                let _ = write!(tag, " (+{})", services.len() - 1);
            }
            Some(tag)
        }
        Some(ProcessRole::SystemdService { unit }) => Some(format!("[[SystemdService]] {unit}")),
        None => None,
    }
}

fn node_label(p: &ProcessInfo, opts: &RenderOptions) -> String {
    let name = name_of(p, opts.executable);
    match role_tag(p) {
        Some(tag) if opts.executable => format!("{name} {tag}"),
        Some(tag) => tag,
        None => name,
    }
}

fn node(
    out: &mut String,
    g: &Glyphs,
    indent: usize,
    p: &ProcessInfo,
    is_last: bool,
    opts: &RenderOptions,
) {
    let base = g.blank.repeat(indent);
    let connector = if is_last { g.last } else { g.branch };
    let _ = writeln!(out, "{base}{connector}{} ({})", node_label(p, opts), p.pid);
    if opts.long
        && let Some(command) = &p.command
    {
        let continuation = if is_last { g.blank } else { g.pipe };
        let _ = writeln!(out, "{base}{continuation}{command}");
    }
}

fn omitted(out: &mut String, g: &Glyphs, indent: usize, count: usize, is_last: bool) {
    let base = g.blank.repeat(indent);
    let connector = if is_last { g.last } else { g.branch };
    let noun = if count == 1 { "process" } else { "processes" };
    let _ = writeln!(out, "{base}{connector}... {count} {noun} omitted");
}

pub fn render_holders(path: &Path, holders: &[ProcessInfo], opts: &RenderOptions) -> String {
    let g = if opts.ascii { &ASCII } else { &UNICODE };
    let mut out = String::new();
    let _ = writeln!(out, "{}", path.display());

    let shown = match opts.max_children {
        Some(max) => holders.len().min(max),
        None => holders.len(),
    };
    let hidden = holders.len() - shown;
    for (i, holder) in holders[..shown].iter().enumerate() {
        node(&mut out, g, 0, holder, hidden == 0 && i == shown - 1, opts);
    }
    if hidden > 0 {
        omitted(&mut out, g, 0, hidden, true);
    }
    out
}

pub fn render(tree: &ProcessTree, opts: &RenderOptions) -> String {
    let g = if opts.ascii { &ASCII } else { &UNICODE };
    let mut out = String::new();

    let skip = match opts.max_ancestors {
        Some(max) => tree.ancestors.len().saturating_sub(max),
        None => 0,
    };
    if skip > 0 {
        omitted(&mut out, g, 0, skip, false);
    }
    for ancestor in &tree.ancestors[skip..] {
        node(&mut out, g, 0, ancestor, false, opts);
    }

    node(&mut out, g, 0, &tree.current, true, opts);

    let shown = match opts.max_children {
        Some(max) => tree.children.len().min(max),
        None => tree.children.len(),
    };
    let hidden = tree.children.len() - shown;
    for (i, child) in tree.children[..shown].iter().enumerate() {
        node(&mut out, g, 1, child, hidden == 0 && i == shown - 1, opts);
    }
    if hidden > 0 {
        omitted(&mut out, g, 1, hidden, true);
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
            exe: None,
            open_files: Vec::new(),
            role: None,
        }
    }

    fn sample() -> ProcessTree {
        let mut worker = proc(301, 300, "worker");
        worker.exe = Some("/usr/bin/worker".to_string());
        worker.command = Some("/usr/bin/worker --daemon".to_string());
        let mut current = proc(300, 200, "ph");
        current.exe = Some("/usr/local/bin/ph".to_string());
        current.command = Some("/usr/local/bin/ph 300".to_string());
        let mut init = proc(1, 0, "init");
        init.exe = Some("/sbin/init".to_string());
        init.command = Some("/sbin/init splash".to_string());
        ProcessTree {
            ancestors: vec![init, proc(200, 1, "bash")],
            current,
            children: vec![worker, proc(302, 300, "logger")],
        }
    }

    #[test]
    fn default_shows_binary_names_only() {
        let expected = "├── init (1)\n├── bash (200)\n└── ph (300)\n    ├── worker (301)\n    └── logger (302)\n";
        assert_eq!(render(&sample(), &RenderOptions::default()), expected);
    }

    #[test]
    fn long_shows_full_command_continuations() {
        let opts = RenderOptions {
            long: true,
            ..Default::default()
        };
        let expected = "├── init (1)\n│   /sbin/init splash\n├── bash (200)\n└── ph (300)\n    /usr/local/bin/ph 300\n    ├── worker (301)\n    │   /usr/bin/worker --daemon\n    └── logger (302)\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn executable_replaces_name_with_full_path() {
        let opts = RenderOptions {
            executable: true,
            ..Default::default()
        };
        let expected = "├── /sbin/init (1)\n├── bash (200)\n└── /usr/local/bin/ph (300)\n    ├── /usr/bin/worker (301)\n    └── logger (302)\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn ascii_uses_ascii_glyphs() {
        let opts = RenderOptions {
            ascii: true,
            long: true,
            ..Default::default()
        };
        let expected = "|-- init (1)\n|   /sbin/init splash\n|-- bash (200)\n+-- ph (300)\n    /usr/local/bin/ph 300\n    |-- worker (301)\n    |   /usr/bin/worker --daemon\n    +-- logger (302)\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn max_ancestors_omits_oldest() {
        let opts = RenderOptions {
            max_ancestors: Some(1),
            ..Default::default()
        };
        let expected = "├── ... 1 process omitted\n├── bash (200)\n└── ph (300)\n    ├── worker (301)\n    └── logger (302)\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn max_ancestors_zero_omits_all() {
        let opts = RenderOptions {
            max_ancestors: Some(0),
            ..Default::default()
        };
        let expected = "├── ... 2 processes omitted\n└── ph (300)\n    ├── worker (301)\n    └── logger (302)\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn max_ancestors_above_length_is_noop() {
        let opts = RenderOptions {
            max_ancestors: Some(10),
            ..Default::default()
        };
        assert_eq!(
            render(&sample(), &opts),
            render(&sample(), &RenderOptions::default())
        );
    }

    #[test]
    fn max_children_keeps_first_and_marks_last() {
        let opts = RenderOptions {
            max_children: Some(1),
            ..Default::default()
        };
        let expected = "├── init (1)\n├── bash (200)\n└── ph (300)\n    ├── worker (301)\n    └── ... 1 process omitted\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    #[test]
    fn max_children_zero_shows_only_marker() {
        let opts = RenderOptions {
            max_children: Some(0),
            ..Default::default()
        };
        let expected =
            "├── init (1)\n├── bash (200)\n└── ph (300)\n    └── ... 2 processes omitted\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    fn holders_sample() -> Vec<ProcessInfo> {
        let mut vim = proc(123, 1, "vim");
        vim.exe = Some("/usr/bin/vim".to_string());
        vim.command = Some("/usr/bin/vim /var/log/app.log".to_string());
        let mut code = proc(456, 1, "code");
        code.exe = Some("/usr/share/code/code".to_string());
        code.command = Some("/usr/share/code/code /var/log".to_string());
        vec![vim, code]
    }

    #[test]
    fn holders_default_lists_processes_under_path() {
        let expected = "/var/log/app.log\n├── vim (123)\n└── code (456)\n";
        assert_eq!(
            render_holders(
                Path::new("/var/log/app.log"),
                &holders_sample(),
                &RenderOptions::default()
            ),
            expected
        );
    }

    #[test]
    fn holders_with_long_ascii_and_executable() {
        let opts = RenderOptions {
            ascii: true,
            long: true,
            executable: true,
            ..Default::default()
        };
        let expected = "/var/log/app.log\n|-- /usr/bin/vim (123)\n|   /usr/bin/vim /var/log/app.log\n+-- /usr/share/code/code (456)\n    /usr/share/code/code /var/log\n";
        assert_eq!(
            render_holders(Path::new("/var/log/app.log"), &holders_sample(), &opts),
            expected
        );
    }

    #[test]
    fn holders_max_children_omits_rest() {
        let opts = RenderOptions {
            max_children: Some(1),
            ..Default::default()
        };
        let expected = "/var/log/app.log\n├── vim (123)\n└── ... 1 process omitted\n";
        assert_eq!(
            render_holders(Path::new("/var/log/app.log"), &holders_sample(), &opts),
            expected
        );
    }

    #[test]
    fn limits_with_long_and_ascii() {
        let opts = RenderOptions {
            ascii: true,
            long: true,
            max_ancestors: Some(1),
            max_children: Some(1),
            ..Default::default()
        };
        let expected = "|-- ... 1 process omitted\n|-- bash (200)\n+-- ph (300)\n    /usr/local/bin/ph 300\n    |-- worker (301)\n    |   /usr/bin/worker --daemon\n    +-- ... 1 process omitted\n";
        assert_eq!(render(&sample(), &opts), expected);
    }

    fn service_tree() -> ProcessTree {
        let mut services = proc(500, 400, "services.exe");
        services.exe = Some("C:\\Windows\\System32\\services.exe".to_string());
        let mut svc = proc(600, 500, "FooSvc.exe");
        svc.exe = Some("C:\\Services\\FooSvc.exe".to_string());
        svc.role = Some(ProcessRole::WindowsService {
            services: vec![prochist_core::ServiceEntry {
                name: "FooSvc".to_string(),
                display_name: Some("Foo Service".to_string()),
            }],
        });
        let mut shared = proc(700, 500, "svchost.exe");
        shared.role = Some(ProcessRole::WindowsService {
            services: vec![
                prochist_core::ServiceEntry {
                    name: "DcomLaunch".to_string(),
                    display_name: None,
                },
                prochist_core::ServiceEntry {
                    name: "RpcSs".to_string(),
                    display_name: None,
                },
            ],
        });
        let mut helper = proc(800, 700, "helper.exe");
        helper.exe = Some("C:\\Services\\helper.exe".to_string());
        ProcessTree {
            ancestors: vec![services, svc],
            current: shared,
            children: vec![helper],
        }
    }

    #[test]
    fn windows_service_replaces_name_with_tag() {
        let expected = "├── services.exe (500)\n├── [[WinService]] FooSvc (600)\n└── [[WinService]] DcomLaunch (+1) (700)\n    └── helper.exe (800)\n";
        assert_eq!(render(&service_tree(), &RenderOptions::default()), expected);
    }

    #[test]
    fn windows_service_tag_follows_binary_in_executable_mode() {
        let opts = RenderOptions {
            executable: true,
            ..Default::default()
        };
        let expected = "├── C:\\Windows\\System32\\services.exe (500)\n├── C:\\Services\\FooSvc.exe [[WinService]] FooSvc (600)\n└── svchost.exe [[WinService]] DcomLaunch (+1) (700)\n    └── C:\\Services\\helper.exe (800)\n";
        assert_eq!(render(&service_tree(), &opts), expected);
    }
}
