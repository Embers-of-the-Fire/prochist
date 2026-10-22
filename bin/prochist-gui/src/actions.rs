use gpui_kit::*;
use prochist_core::Pid;

actions!(
    phg,
    [
        QuitApp,
        Refresh,
        FocusSearch,
        CancelSearch,
        ToggleHelp,
        MoveUp,
        MoveDown,
        HalfPageUp,
        HalfPageDown,
        FullPageUp,
        FullPageDown,
        MoveStart,
        MoveEnd,
        FocusSelected,
        RestoreFocus,
        NextMatch,
        PrevMatch,
        FocusDetails,
        FocusMain,
        DetailUp,
        DetailDown,
        ShowAllProcesses,
        YankNamePid,
        YankCommand,
        YankField,
    ]
);

#[derive(Clone, PartialEq, Action)]
#[action(namespace = phg, no_json)]
pub struct FocusProcess {
    pub pid: Pid,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = phg, no_json)]
pub struct CopyNamePid {
    pub pid: Pid,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = phg, no_json)]
pub struct CopyCommand {
    pub pid: Pid,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = phg, no_json)]
pub struct CopyPid {
    pub pid: Pid,
}

#[derive(Clone, PartialEq, Action)]
#[action(namespace = phg, no_json)]
pub struct CopyExe {
    pub pid: Pid,
}
