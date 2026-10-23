use std::collections::{HashMap, HashSet};

use prochist_core::{Pid, ProcessInfo, ProcessRole, TreeError, build_tree};

pub const FIELD_LABELS: [&str; 6] = ["PID", "PPID", "Name", "Command", "Executable", "Service"];

pub fn role_tag(p: &ProcessInfo) -> Option<String> {
    match &p.role {
        Some(ProcessRole::WindowsService { services }) => {
            let first = services.first()?;
            let mut tag = format!("[[WinService]] {}", first.name);
            if services.len() > 1 {
                tag.push_str(&format!(" (+{})", services.len() - 1));
            }
            Some(tag)
        }
        Some(ProcessRole::SystemdService { unit }) => Some(format!("[[SystemdService]] {unit}")),
        None => None,
    }
}

pub fn display_name(p: &ProcessInfo) -> String {
    if let Some(tag) = role_tag(p) {
        return tag;
    }
    match &p.exe {
        Some(exe) => exe
            .rsplit(['/', '\\'])
            .next()
            .filter(|f| !f.is_empty())
            .map(|f| f.to_string())
            .unwrap_or_else(|| p.name.clone()),
        None => p.name.clone(),
    }
}

pub fn service_field(info: &ProcessInfo) -> Option<String> {
    match &info.role {
        Some(ProcessRole::WindowsService { services }) if !services.is_empty() => {
            let names: Vec<String> = services
                .iter()
                .map(|s| match &s.display_name {
                    Some(d) => format!("{} ({d})", s.name),
                    None => s.name.clone(),
                })
                .collect();
            Some(names.join(", "))
        }
        Some(ProcessRole::SystemdService { unit }) => Some(unit.clone()),
        _ => None,
    }
}

pub fn fields(info: &ProcessInfo) -> Vec<Option<String>> {
    vec![
        Some(info.pid.to_string()),
        Some(info.ppid.to_string()),
        Some(info.name.clone()),
        info.command.clone(),
        info.exe.clone(),
        service_field(info),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField {
    Any,
    Pid,
    Name,
    Cmd,
    Exe,
    Service,
}

pub fn parse_query(query: &str) -> (SearchField, &str) {
    let prefixes = [
        ("pid:", SearchField::Pid),
        ("name:", SearchField::Name),
        ("cmd:", SearchField::Cmd),
        ("exe:", SearchField::Exe),
        ("service:", SearchField::Service),
    ];
    for (prefix, field) in prefixes {
        if let Some(rest) = query.strip_prefix(prefix) {
            return (field, rest.trim());
        }
    }
    (SearchField::Any, query)
}

pub fn field_matches(info: &ProcessInfo, field: SearchField, query: &str) -> bool {
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
        SearchField::Service => service_field(info).as_deref().is_some_and(contains),
    }
}

pub fn filtered<'a>(all: &'a [ProcessInfo], filter: &str) -> impl Iterator<Item = &'a ProcessInfo> {
    let (field, query) = parse_query(filter);
    all.iter()
        .filter(move |info| field_matches(info, field, query))
}

/// Row indices within `rows` whose process matches `query` (jump-to-match).
pub fn find_matches(rows: &[ProcessInfo], query: &str) -> Vec<usize> {
    let (field, query) = parse_query(query);
    rows.iter()
        .enumerate()
        .filter(|(_, info)| field_matches(info, field, query))
        .map(|(ix, _)| ix)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusEntry {
    Process(Pid, String),
    Holders,
    Processes,
}

/// Full descendant tree rooted at a process, built from a flat snapshot.
#[derive(Debug, Clone)]
pub struct ProcNode {
    pub info: ProcessInfo,
    pub children: Vec<ProcNode>,
}

/// Builds the subtree of the outermost ancestor of `focus` (the whole process
/// hierarchy "around" that PID, recursively). Errors only when `focus` is not
/// in the snapshot, mirroring `build_tree`.
pub fn build_subtree(snapshot: &[ProcessInfo], focus: Pid) -> Result<ProcNode, TreeError> {
    let tree = build_tree(snapshot, focus)?;
    let root_pid = tree.ancestors.first().map_or(focus, |a| a.pid);

    let by_pid: HashMap<Pid, &ProcessInfo> = snapshot.iter().map(|p| (p.pid, p)).collect();
    let mut children_of: HashMap<Pid, Vec<&ProcessInfo>> = HashMap::new();
    for p in snapshot {
        children_of.entry(p.ppid).or_default().push(p);
    }
    for children in children_of.values_mut() {
        children.sort_by_key(|p| p.pid);
    }

    let mut visited = HashSet::new();
    Ok(build_node(root_pid, &by_pid, &children_of, &mut visited))
}

fn build_node(
    pid: Pid,
    by_pid: &HashMap<Pid, &ProcessInfo>,
    children_of: &HashMap<Pid, Vec<&ProcessInfo>>,
    visited: &mut HashSet<Pid>,
) -> ProcNode {
    visited.insert(pid);
    let info = by_pid[&pid].clone();
    let child_pids: Vec<Pid> = children_of
        .get(&pid)
        .map(|children| children.iter().map(|c| c.pid).collect())
        .unwrap_or_default();
    let mut children = Vec::new();
    for child in child_pids {
        if visited.insert(child) {
            children.push(build_node(child, by_pid, children_of, visited));
        }
    }
    ProcNode { info, children }
}

/// PIDs on the path from the subtree root to `focus`, inclusive — the nodes
/// that start expanded and revealed.
pub fn expanded_chain(snapshot: &[ProcessInfo], focus: Pid) -> HashSet<Pid> {
    match build_tree(snapshot, focus) {
        Ok(tree) => tree
            .ancestors
            .iter()
            .map(|a| a.pid)
            .chain(std::iter::once(focus))
            .collect(),
        Err(_) => HashSet::from([focus]),
    }
}

/// Visible-order (pid, depth) walk of the tree given the expansion state.
pub fn flatten(node: &ProcNode, expanded: &HashSet<Pid>) -> Vec<(Pid, usize)> {
    fn walk(node: &ProcNode, depth: usize, expanded: &HashSet<Pid>, out: &mut Vec<(Pid, usize)>) {
        out.push((node.info.pid, depth));
        if expanded.contains(&node.info.pid) {
            for child in &node.children {
                walk(child, depth + 1, expanded, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(node, 0, expanded, &mut out);
    out
}

/// Resolves holder PIDs against a snapshot, skipping stale entries.
pub fn resolve_holders(snapshot: &[ProcessInfo], pids: &[Pid]) -> Vec<ProcessInfo> {
    let by_pid: HashMap<Pid, &ProcessInfo> = snapshot.iter().map(|p| (p.pid, p)).collect();
    pids.iter()
        .filter_map(|pid| by_pid.get(pid).map(|p| (*p).clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use prochist_core::ServiceEntry;

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

    fn snapshot() -> Vec<ProcessInfo> {
        vec![
            proc(1, 0, "init"),
            proc(100, 1, "login"),
            proc(200, 100, "shell"),
            proc(300, 200, "editor"),
            proc(400, 200, "worker"),
            proc(500, 400, "thread-pool"),
            proc(600, 999, "orphan"),
        ]
    }

    #[test]
    fn display_name_prefers_exe_basename() {
        let mut p = proc(1, 0, "cmd.exe");
        p.exe = Some("C:\\Windows\\System32\\cmd.exe".to_string());
        assert_eq!(display_name(&p), "cmd.exe");
        let p = proc(1, 0, "fallback");
        assert_eq!(display_name(&p), "fallback");
    }

    #[test]
    fn display_name_uses_service_tag() {
        let mut p = proc(4, 0, "svchost.exe");
        p.role = Some(ProcessRole::WindowsService {
            services: vec![
                ServiceEntry {
                    name: "DcomLaunch".to_string(),
                    display_name: None,
                },
                ServiceEntry {
                    name: "Power".to_string(),
                    display_name: None,
                },
            ],
        });
        assert_eq!(display_name(&p), "[[WinService]] DcomLaunch (+1)");

        let mut p = proc(5, 0, "nginx");
        p.role = Some(ProcessRole::SystemdService {
            unit: "nginx.service".to_string(),
        });
        assert_eq!(display_name(&p), "[[SystemdService]] nginx.service");
    }

    #[test]
    fn fields_service_joins_names_with_display() {
        let mut p = proc(4, 0, "svchost.exe");
        p.role = Some(ProcessRole::WindowsService {
            services: vec![
                ServiceEntry {
                    name: "DcomLaunch".to_string(),
                    display_name: Some("DCOM Server Process Launcher".to_string()),
                },
                ServiceEntry {
                    name: "Power".to_string(),
                    display_name: None,
                },
            ],
        });
        let f = fields(&p);
        assert_eq!(f.len(), FIELD_LABELS.len());
        assert_eq!(
            f[5].as_deref(),
            Some("DcomLaunch (DCOM Server Process Launcher), Power")
        );
    }

    #[test]
    fn parse_query_prefixes() {
        assert_eq!(parse_query("pid:12"), (SearchField::Pid, "12"));
        assert_eq!(parse_query("name: foo "), (SearchField::Name, "foo"));
        assert_eq!(parse_query("cmd:-c"), (SearchField::Cmd, "-c"));
        assert_eq!(parse_query("exe:/usr/bin"), (SearchField::Exe, "/usr/bin"));
        assert_eq!(
            parse_query("service:nginx"),
            (SearchField::Service, "nginx")
        );
        assert_eq!(parse_query("plain"), (SearchField::Any, "plain"));
    }

    #[test]
    fn field_matches_rules() {
        let mut p = proc(1234, 0, "nginx");
        p.command = Some("nginx: master process".to_string());
        p.exe = Some("/usr/sbin/nginx".to_string());
        assert!(field_matches(&p, SearchField::Any, "NGINX"));
        assert!(field_matches(&p, SearchField::Any, "master"));
        assert!(!field_matches(&p, SearchField::Any, "sbin"));
        assert!(field_matches(&p, SearchField::Pid, "12"));
        assert!(!field_matches(&p, SearchField::Pid, "34"));
        assert!(field_matches(&p, SearchField::Exe, "sbin/nginx"));
        assert!(field_matches(&p, SearchField::Any, ""));
    }

    #[test]
    fn filtered_applies_query() {
        let mut snap = snapshot();
        snap[3].command = Some("code .".to_string());
        let names: Vec<_> = filtered(&snap, "name:work").map(|p| p.pid).collect();
        assert_eq!(names, vec![400]);
        assert_eq!(filtered(&snap, "").count(), snap.len());
    }

    #[test]
    fn build_subtree_roots_at_outermost_ancestor() {
        let snap = snapshot();
        let root = build_subtree(&snap, 300).unwrap();
        assert_eq!(root.info.pid, 1);
        assert_eq!(root.children.len(), 1);
        let login = &root.children[0];
        assert_eq!(login.info.pid, 100);
        let shell = &login.children[0];
        assert_eq!(shell.info.pid, 200);
        assert_eq!(
            shell
                .children
                .iter()
                .map(|c| c.info.pid)
                .collect::<Vec<_>>(),
            vec![300, 400]
        );
        assert_eq!(shell.children[1].children[0].info.pid, 500);
    }

    #[test]
    fn build_subtree_unknown_pid_errors() {
        let snap = snapshot();
        assert!(matches!(
            build_subtree(&snap, 4242),
            Err(TreeError::NotFound(4242))
        ));
    }

    #[test]
    fn build_subtree_survives_cycles() {
        let mut snap = vec![proc(1, 2, "a"), proc(2, 1, "b")];
        snap[0].ppid = 2;
        let root = build_subtree(&snap, 1).unwrap();
        let mut count = 0;
        let mut stack = vec![&root];
        while let Some(node) = stack.pop() {
            count += 1;
            stack.extend(node.children.iter());
        }
        assert_eq!(count, 2);
    }

    #[test]
    fn expanded_chain_covers_ancestors_and_focus() {
        let snap = snapshot();
        let chain = expanded_chain(&snap, 300);
        assert!(chain.contains(&1));
        assert!(chain.contains(&100));
        assert!(chain.contains(&200));
        assert!(chain.contains(&300));
        assert!(!chain.contains(&400));
    }

    #[test]
    fn flatten_respects_expansion() {
        let snap = snapshot();
        let root = build_subtree(&snap, 300).unwrap();
        let all: HashSet<Pid> = [1, 100, 200, 300, 400].into_iter().collect();
        let flat = flatten(&root, &all);
        assert_eq!(
            flat,
            vec![(1, 0), (100, 1), (200, 2), (300, 3), (400, 3), (500, 4)]
        );
        let collapsed: HashSet<Pid> = [1].into_iter().collect();
        let flat = flatten(&root, &collapsed);
        assert_eq!(flat, vec![(1, 0), (100, 1)]);
    }

    #[test]
    fn resolve_holders_skips_stale_pids() {
        let snap = snapshot();
        let holders = resolve_holders(&snap, &[200, 9999, 1]);
        assert_eq!(
            holders.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![200, 1]
        );
    }

    #[test]
    fn find_matches_uses_query_syntax() {
        let snap = snapshot();
        assert_eq!(find_matches(&snap, "name:or"), vec![3, 4, 6]);
        assert_eq!(find_matches(&snap, "pid:1"), vec![0, 1]);
        assert!(find_matches(&snap, "name:zzz").is_empty());
    }
}
