#[derive(Debug, Clone)]
pub enum UiEvent {
    Ready { active: bool, grabbed: bool },
    Mode { active: bool },
    Key(KeyAction),
    Inserted,
    Error(String),
}

#[derive(Debug, Clone)]
pub enum KeyAction {
    Char(String),
    DeleteBackward,
    DeleteForward,
    DeleteWordBackward,
    DeleteWordForward,
    DeleteToStart,
    DeleteToEnd,
    MoveLeft { extend: bool },
    MoveRight { extend: bool },
    MoveWordLeft { extend: bool },
    MoveWordRight { extend: bool },
    MoveHome { extend: bool },
    MoveEnd { extend: bool },
    SelectAll,
    ToggleList,
    NextCategory,
    PreviousCategory,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Enter,
    Escape,
    Up,
    Down,
}

pub enum UiCommand {
    Insert(String),
    Cancel,
}
