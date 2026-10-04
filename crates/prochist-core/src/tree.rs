use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use crate::model::{Pid, ProcessInfo, ProcessTree};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeError {
    NotFound(Pid),
}

impl fmt::Display for TreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TreeError::NotFound(pid) => write!(f, "no such process: {pid}"),
        }
    }
}

impl Error for TreeError {}

pub fn build_tree(snapshot: &[ProcessInfo], pid: Pid) -> Result<ProcessTree, TreeError> {
    let by_pid: HashMap<Pid, &ProcessInfo> = snapshot.iter().map(|p| (p.pid, p)).collect();
    let current = by_pid.get(&pid).ok_or(TreeError::NotFound(pid))?;

    let mut ancestors = Vec::new();
    let mut seen = HashSet::from([pid]);
    let mut next = current.ppid;
    while let Some(parent) = by_pid.get(&next) {
        if !seen.insert(parent.pid) {
            break;
        }
        ancestors.push((*parent).clone());
        next = parent.ppid;
    }
    ancestors.reverse();

    let mut children: Vec<ProcessInfo> = snapshot
        .iter()
        .filter(|p| p.ppid == pid && p.pid != pid)
        .cloned()
        .collect();
    children.sort_by_key(|p| p.pid);

    Ok(ProcessTree {
        ancestors,
        current: (*current).clone(),
        children,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proc(pid: Pid, ppid: Pid, name: &str) -> ProcessInfo {
        ProcessInfo {
            pid,
            ppid,
            name: name.to_string(),
            command: None,
        }
    }

    fn sample() -> Vec<ProcessInfo> {
        vec![
            proc(1, 0, "init"),
            proc(100, 1, "login"),
            proc(200, 100, "bash"),
            proc(300, 200, "ph"),
            proc(301, 300, "worker"),
            proc(302, 300, "logger"),
            proc(400, 1, "unrelated"),
        ]
    }

    #[test]
    fn builds_ancestor_chain_and_children() {
        let tree = build_tree(&sample(), 300).unwrap();
        let ancestor_pids: Vec<Pid> = tree.ancestors.iter().map(|p| p.pid).collect();
        assert_eq!(ancestor_pids, vec![1, 100, 200]);
        assert_eq!(tree.current.pid, 300);
        let child_pids: Vec<Pid> = tree.children.iter().map(|p| p.pid).collect();
        assert_eq!(child_pids, vec![301, 302]);
    }

    #[test]
    fn stops_chain_when_parent_is_dead() {
        let snapshot = vec![proc(500, 999, "daemon"), proc(501, 500, "task")];
        let tree = build_tree(&snapshot, 500).unwrap();
        assert!(tree.ancestors.is_empty());
        assert_eq!(tree.children.len(), 1);
    }

    #[test]
    fn guards_against_ppid_cycles() {
        let snapshot = vec![proc(10, 11, "a"), proc(11, 10, "b")];
        let tree = build_tree(&snapshot, 10).unwrap();
        let ancestor_pids: Vec<Pid> = tree.ancestors.iter().map(|p| p.pid).collect();
        assert_eq!(ancestor_pids, vec![11]);
    }

    #[test]
    fn errors_on_unknown_pid() {
        assert_eq!(build_tree(&sample(), 999), Err(TreeError::NotFound(999)));
    }

    #[test]
    fn children_are_sorted_by_pid() {
        let snapshot = vec![proc(1, 0, "init"), proc(9, 1, "z"), proc(3, 1, "a")];
        let tree = build_tree(&snapshot, 1).unwrap();
        let child_pids: Vec<Pid> = tree.children.iter().map(|p| p.pid).collect();
        assert_eq!(child_pids, vec![3, 9]);
    }
}
