//! UI integration tests: real components in a headless window, driven through
//! the gpui-kit test harness. Fixture data flows through `Source::Fixture`,
//! the same path as `--snapshot`.
#[cfg(test)]
mod tests {
    use gpui_kit::test::{TestQueryExt, TestWindowExt};
    use gpui_kit::{Entity, TestAppContext, VisualTestContext};
    use prochist_core::{Pid, ProcessInfo};

    use crate::app::{Init, Phg, View};
    use crate::source::Source;

    fn fixture() -> Vec<ProcessInfo> {
        let proc = |pid: Pid, ppid: Pid, name: &str, command: Option<&str>| ProcessInfo {
            pid,
            ppid,
            name: name.to_string(),
            command: command.map(str::to_string),
            exe: None,
            open_files: Vec::new(),
            role: None,
        };
        vec![
            proc(1, 0, "init", Some("/sbin/init splash")),
            proc(200, 1, "bash", Some("/usr/bin/bash -l")),
            proc(300, 200, "ph", Some("/usr/local/bin/ph 300")),
            proc(301, 300, "worker", Some("/usr/bin/worker --daemon")),
            proc(302, 300, "logger", None),
        ]
    }

    fn open(cx: &mut TestAppContext, init: Init) -> (Entity<Phg>, &mut VisualTestContext) {
        open_with(cx, Source::Fixture(fixture()), init)
    }

    fn open_with(
        cx: &mut TestAppContext,
        source: Source,
        init: Init,
    ) -> (Entity<Phg>, &mut VisualTestContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::bind_keys(cx);
        });
        let (view, cx) = cx.add_window_view(move |window, cx| Phg::new(window, cx, source, init));
        cx.run_until_parked();
        (view, cx)
    }

    #[gpui_kit::test]
    fn processes_view_starts_on_first_row(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        view.read_with(cx, |view, cx| {
            assert_eq!(view.selected_pid(cx), Some(1));
            let (_, total) = view.position(cx);
            assert_eq!(total, 5);
        });
    }

    #[gpui_kit::test]
    fn typing_in_search_live_filters_processes(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_search(window, cx));
            window.input("work", cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            let (_, total) = view.position(cx);
            assert_eq!(total, 1);
            assert_eq!(view.selected_pid(cx), Some(301));
        });
    }

    #[gpui_kit::test]
    fn tree_view_reveals_focused_process(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Tree {
                snapshot: fixture(),
                pid: 300,
            },
        );
        view.read_with(cx, |view, cx| {
            assert_eq!(view.selected_pid(cx), Some(300));
            let (_, total) = view.position(cx);
            // init > bash > ph, then ph's children worker + logger.
            assert_eq!(total, 5);
        });
    }

    #[gpui_kit::test]
    fn focus_re_roots_and_backspace_restores(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_pid(301, window, cx));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(matches!(view.view_kind(), View::Tree));
            assert_eq!(view.tree_focus_pid(), Some(301));
            assert!(view.breadcrumb().contains("Processes"));
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.restore(window, cx));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(matches!(view.view_kind(), View::Processes));
        });
    }

    #[gpui_kit::test]
    fn double_click_tree_row_focuses_process(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Tree {
                snapshot: fixture(),
                pid: 300,
            },
        );
        cx.update(|window, cx| {
            // The fourth visible row (init > bash > ph > worker) is worker (301).
            window.find_by_label("worker (301)");
            window.double_click(3, cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert_eq!(view.tree_focus_pid(), Some(301));
        });
    }

    #[gpui_kit::test]
    fn back_button_restores_previous_focus(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_pid(301, window, cx));
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.click("back", cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, _| {
            assert!(matches!(view.view_kind(), View::Processes));
        });
    }

    #[gpui_kit::test]
    fn next_prev_buttons_cycle_search_matches(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Tree {
                snapshot: fixture(),
                pid: 300,
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_search(window, cx));
            window.input("s", cx);
            window.press("enter", cx);
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert_eq!(view.selected_pid(cx), Some(1));
        });
        cx.update(|window, cx| {
            window.click("next-match", cx);
        });
        view.read_with(cx, |view, cx| {
            assert_eq!(view.selected_pid(cx), Some(200));
        });
        cx.update(|window, cx| {
            window.click("prev-match", cx);
        });
        view.read_with(cx, |view, cx| {
            assert_eq!(view.selected_pid(cx), Some(1));
        });
    }

    #[gpui_kit::test]
    fn match_buttons_show_only_in_tree_and_holders(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        cx.update(|window, _| {
            assert!(window.try_find_by_label("Prev").is_none());
            assert!(window.try_find_by_label("Next").is_none());
        });
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_pid(301, window, cx));
        });
        cx.run_until_parked();
        cx.update(|window, _| {
            assert!(window.try_find_by_label("Prev").is_some());
            assert!(window.try_find_by_label("Next").is_some());
        });
    }

    #[gpui_kit::test]
    fn focus_syncs_cached_process_table(cx: &mut TestAppContext) {
        // The source's next snapshot drops logger (302); the cached processes
        // table must be re-synced when the focus snapshot is installed.
        let mut updated = fixture();
        updated.retain(|p| p.pid != 302);
        let (view, cx) = open_with(
            cx,
            Source::Fixture(updated),
            Init::Processes {
                snapshot: fixture(),
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_pid(301, window, cx));
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.restore(window, cx));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert!(matches!(view.view_kind(), View::Processes));
            let (_, total) = view.position(cx);
            assert_eq!(total, 4);
        });
    }

    #[gpui_kit::test]
    fn focus_re_resolves_cached_holder_table(cx: &mut TestAppContext) {
        // The source's next snapshot drops bash (200), a current holder; the
        // cached holders table must be re-resolved against the new snapshot.
        let mut updated = fixture();
        updated.retain(|p| p.pid != 200);
        let holders = fixture()
            .into_iter()
            .filter(|p| p.pid == 200 || p.pid == 300)
            .collect();
        let (view, cx) = open_with(
            cx,
            Source::Fixture(updated),
            Init::Holders {
                snapshot: fixture(),
                path: "/tmp/lock".into(),
                holders,
            },
        );
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.focus_pid(300, window, cx));
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            view.update(cx, |view, cx| view.restore(window, cx));
        });
        cx.run_until_parked();
        view.read_with(cx, |view, cx| {
            assert!(matches!(view.view_kind(), View::Holders));
            let (_, total) = view.position(cx);
            assert_eq!(total, 1);
        });
    }

    #[gpui_kit::test]
    fn details_pane_fits_all_fields(cx: &mut TestAppContext) {
        let (view, cx) = open(
            cx,
            Init::Processes {
                snapshot: fixture(),
            },
        );
        let _ = view;
        cx.update(|window, _cx| {
            let pane = window.find("details").bounds();
            let last = window.find(("detail", 5usize)).bounds();
            assert!(
                last.bottom() <= pane.bottom(),
                "Service field must fit inside the details pane: {last:?} vs {pane:?}"
            );
        });
    }
}
