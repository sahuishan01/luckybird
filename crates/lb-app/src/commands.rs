/// Every editor action is represented as a `Command`.
///
/// Commands are dispatched by the key-binding table and via the command
/// palette.  They are plain data (no closures) so they can be serialised,
/// logged, and replayed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    // ── Cursor movement ────────────────────────────────────────────────────
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    MoveWordForward,
    MoveWordBackward,
    MoveLineStart,
    MoveLineEnd,
    MoveDocStart,
    MoveDocEnd,
    PageUp,
    PageDown,

    // ── Text editing ───────────────────────────────────────────────────────
    InsertChar(char),
    InsertNewline,
    InsertTab,
    DeleteBackward,
    DeleteForward,

    // ── Undo / redo ────────────────────────────────────────────────────────
    Undo,
    Redo,

    // ── File I/O ───────────────────────────────────────────────────────────
    Save,
    OpenFile(String),

    // ── Session ────────────────────────────────────────────────────────────
    Quit,
    ForceQuit,

    // ── LSP actions ────────────────────────────────────────────────────────
    Hover,
    GotoDefinition,
    TriggerCompletion,

    // ── Multi-cursor ───────────────────────────────────────────────────────
    AddCursorAbove,
    AddCursorBelow,
    SelectNextMatch,

    // ── UI overlays ────────────────────────────────────────────────────────
    CommandPalette,
    FilePicker,
    ToggleComment,

    // ── Modal editing ──────────────────────────────────────────────────────
    EnterNormalMode,
    EnterInsertMode,
    EnterInsertAfter,
    OpenLineBelow,
    OpenLineAbove,
}

/// Parse a command name string (from config keybindings or command palette)
/// into a `Command`.
///
/// Returns `None` for unknown command names.
pub fn from_name(name: &str) -> Option<Command> {
    match name {
        "move_left"           => Some(Command::MoveLeft),
        "move_right"          => Some(Command::MoveRight),
        "move_up"             => Some(Command::MoveUp),
        "move_down"           => Some(Command::MoveDown),
        "move_word_forward"   => Some(Command::MoveWordForward),
        "move_word_backward"  => Some(Command::MoveWordBackward),
        "move_line_start"     => Some(Command::MoveLineStart),
        "move_line_end"       => Some(Command::MoveLineEnd),
        "move_doc_start"      => Some(Command::MoveDocStart),
        "move_doc_end"        => Some(Command::MoveDocEnd),
        "page_up"             => Some(Command::PageUp),
        "page_down"           => Some(Command::PageDown),
        "insert_newline"      => Some(Command::InsertNewline),
        "insert_tab"          => Some(Command::InsertTab),
        "delete_backward"     => Some(Command::DeleteBackward),
        "delete_forward"      => Some(Command::DeleteForward),
        "undo"                => Some(Command::Undo),
        "redo"                => Some(Command::Redo),
        "save"                => Some(Command::Save),
        "quit"                => Some(Command::Quit),
        "force_quit"          => Some(Command::ForceQuit),
        "hover"               => Some(Command::Hover),
        "goto_definition"     => Some(Command::GotoDefinition),
        "trigger_completion"  => Some(Command::TriggerCompletion),
        "add_cursor_above"    => Some(Command::AddCursorAbove),
        "add_cursor_below"    => Some(Command::AddCursorBelow),
        "select_next_match"   => Some(Command::SelectNextMatch),
        "command_palette"     => Some(Command::CommandPalette),
        "file_picker"         => Some(Command::FilePicker),
        "toggle_comment"      => Some(Command::ToggleComment),
        "enter_normal_mode"   => Some(Command::EnterNormalMode),
        "enter_insert_mode"   => Some(Command::EnterInsertMode),
        "enter_insert_after"  => Some(Command::EnterInsertAfter),
        "open_line_below"     => Some(Command::OpenLineBelow),
        "open_line_above"     => Some(Command::OpenLineAbove),
        _                     => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_quit() {
        assert_eq!(from_name("quit"), Some(Command::Quit));
    }

    #[test]
    fn from_name_unknown_returns_none() {
        assert!(from_name("nonexistent_command").is_none());
    }
}
