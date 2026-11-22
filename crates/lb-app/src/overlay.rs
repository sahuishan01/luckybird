//! The actions overlay: a searchable menu (`Ctrl-Space` or `F1`), language
//! pickers and one-line prompts. In lists, typing filters the entries.

use lb_tui::{KeyCode, KeyEvent2, OverlayModel, OverlayRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Save,
    SaveAs,
    Open,
    Reload,
    InstallGrammar,
    InstallServer,
    Quit,
    ForceQuit,
}

struct MenuEntry {
    section: &'static str,
    label: &'static str,
    action: MenuAction,
}

const MENU: &[MenuEntry] = &[
    MenuEntry { section: "File", label: "Save", action: MenuAction::Save },
    MenuEntry { section: "File", label: "Save as...", action: MenuAction::SaveAs },
    MenuEntry { section: "File", label: "Open...", action: MenuAction::Open },
    MenuEntry { section: "File", label: "Reload file (F5)", action: MenuAction::Reload },
    MenuEntry { section: "Language", label: "Install syntax highlighting...", action: MenuAction::InstallGrammar },
    MenuEntry { section: "Language", label: "Install language server...", action: MenuAction::InstallServer },
    MenuEntry { section: "Session", label: "Quit", action: MenuAction::Quit },
    MenuEntry { section: "Session", label: "Quit without saving", action: MenuAction::ForceQuit },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickKind {
    Grammar,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    SaveAs,
    Open,
}

#[derive(Debug)]
pub enum Overlay {
    /// `selected` indexes the entries that match `query`.
    Menu { query: String, selected: usize },
    Pick { kind: PickKind, items: Vec<String>, query: String, selected: usize },
    Input { kind: InputKind, text: String },
}

/// What the app should do after a key press.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Stay,
    Close,
    Action(MenuAction),
    Picked(PickKind, String),
    Submitted(InputKind, String),
}

/// Case-insensitive: every whitespace-separated word of `query` appears in `text`.
fn matches(query: &str, text: &str) -> bool {
    let text = text.to_lowercase();
    query.split_whitespace().all(|word| text.contains(&word.to_lowercase()))
}

fn visible_menu(query: &str) -> Vec<&'static MenuEntry> {
    MENU.iter().filter(|e| matches(query, e.label)).collect()
}

fn visible_items<'a>(items: &'a [String], query: &str) -> Vec<&'a String> {
    items.iter().filter(|i| matches(query, i)).collect()
}

impl Overlay {
    pub fn menu() -> Self {
        Overlay::Menu { query: String::new(), selected: 0 }
    }

    pub fn pick(kind: PickKind, mut items: Vec<String>) -> Self {
        items.sort();
        Overlay::Pick { kind, items, query: String::new(), selected: 0 }
    }

    pub fn input(kind: InputKind, initial: String) -> Self {
        Overlay::Input { kind, text: initial }
    }

    pub fn handle_key(&mut self, key: &KeyEvent2) -> Outcome {
        let plain = !key.modifiers.ctrl && !key.modifiers.alt;
        if matches!(key.code, KeyCode::F(1)) && plain {
            return Outcome::Close;
        }
        if is_ctrl_space(key) {
            return Outcome::Close;
        }

        match self {
            Overlay::Input { kind, text } => match key.code {
                KeyCode::Escape => Outcome::Close,
                KeyCode::Enter if !text.trim().is_empty() => {
                    Outcome::Submitted(*kind, text.trim().to_owned())
                }
                KeyCode::Backspace => {
                    text.pop();
                    Outcome::Stay
                }
                KeyCode::Char(c) if plain => {
                    text.push(c);
                    Outcome::Stay
                }
                _ => Outcome::Stay,
            },
            Overlay::Menu { query, selected } => {
                let count = visible_menu(query).len();
                match edit_list(key, query, selected, count) {
                    ListKey::Handled => Outcome::Stay,
                    ListKey::Close => Outcome::Close,
                    ListKey::Enter => visible_menu(query)
                        .get(*selected)
                        .map_or(Outcome::Stay, |e| Outcome::Action(e.action)),
                }
            }
            Overlay::Pick { kind, items, query, selected } => {
                let count = visible_items(items, query).len();
                match edit_list(key, query, selected, count) {
                    ListKey::Handled => Outcome::Stay,
                    ListKey::Close => Outcome::Close,
                    ListKey::Enter => visible_items(items, query)
                        .get(*selected)
                        .map_or(Outcome::Stay, |item| Outcome::Picked(*kind, (*item).clone())),
                }
            }
        }
    }

    pub fn model(&self) -> OverlayModel {
        const FOOTER: &str = "type to filter  Up/Down move  Enter select  Esc back";
        match self {
            Overlay::Menu { query, selected } => {
                let mut rows = Vec::new();
                let mut selected_row = None;
                let mut section = "";
                for (i, entry) in visible_menu(query).into_iter().enumerate() {
                    if entry.section != section {
                        section = entry.section;
                        rows.push(OverlayRow { text: section.to_owned(), selectable: false });
                    }
                    if i == *selected {
                        selected_row = Some(rows.len());
                    }
                    rows.push(OverlayRow { text: entry.label.to_owned(), selectable: true });
                }
                if rows.is_empty() {
                    rows.push(OverlayRow { text: "no matches".to_owned(), selectable: false });
                }
                OverlayModel {
                    title: "Actions".to_owned(),
                    rows,
                    selected: selected_row,
                    input: Some(query.clone()),
                    footer: FOOTER,
                }
            }
            Overlay::Pick { kind, items, query, selected } => {
                let mut rows: Vec<OverlayRow> = visible_items(items, query)
                    .into_iter()
                    .map(|i| OverlayRow { text: i.clone(), selectable: true })
                    .collect();
                let has_matches = !rows.is_empty();
                if !has_matches {
                    rows.push(OverlayRow { text: "no matches".to_owned(), selectable: false });
                }
                OverlayModel {
                    title: match kind {
                        PickKind::Grammar => "Install syntax highlighting".to_owned(),
                        PickKind::Server => "Install language server".to_owned(),
                    },
                    rows,
                    selected: has_matches.then_some(*selected),
                    input: Some(query.clone()),
                    footer: FOOTER,
                }
            }
            Overlay::Input { kind, text } => OverlayModel {
                title: match kind {
                    InputKind::SaveAs => "Save as".to_owned(),
                    InputKind::Open => "Open file".to_owned(),
                },
                rows: Vec::new(),
                selected: None,
                input: Some(text.clone()),
                footer: "type a path  Enter confirm  Esc cancel",
            },
        }
    }
}

/// `Ctrl-Space` arrives as a space with Ctrl on most terminals and as NUL on others.
pub fn is_ctrl_space(key: &KeyEvent2) -> bool {
    matches!(key.code, KeyCode::Null)
        || (matches!(key.code, KeyCode::Char(' ')) && key.modifiers.ctrl && !key.modifiers.alt)
}

enum ListKey {
    Handled,
    Close,
    Enter,
}

/// Shared key handling for searchable lists: typing edits `query`, arrows /
/// Tab / Ctrl-N / Ctrl-P / Ctrl-J / Ctrl-K move, `Esc` clears the query and
/// then closes.
fn edit_list(key: &KeyEvent2, query: &mut String, selected: &mut usize, count: usize) -> ListKey {
    let ctrl = key.modifiers.ctrl && !key.modifiers.alt;
    let step: Option<isize> = match key.code {
        KeyCode::Down | KeyCode::Tab => Some(1),
        KeyCode::Up | KeyCode::BackTab => Some(-1),
        KeyCode::Char('n' | 'j') if ctrl => Some(1),
        KeyCode::Char('p' | 'k') if ctrl => Some(-1),
        _ => None,
    };
    if let Some(step) = step {
        *selected = moved(*selected, count, step);
        return ListKey::Handled;
    }
    match key.code {
        KeyCode::Enter => ListKey::Enter,
        KeyCode::Escape if query.is_empty() => ListKey::Close,
        KeyCode::Escape => {
            query.clear();
            *selected = 0;
            ListKey::Handled
        }
        KeyCode::Backspace => {
            query.pop();
            *selected = 0;
            ListKey::Handled
        }
        KeyCode::Char(c) if !key.modifiers.ctrl && !key.modifiers.alt => {
            query.push(c);
            *selected = 0;
            ListKey::Handled
        }
        _ => ListKey::Handled,
    }
}

fn moved(current: usize, len: usize, step: isize) -> usize {
    if len == 0 {
        return 0;
    }
    (current as isize + step).rem_euclid(len as isize) as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_tui::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent2 {
        KeyEvent2 { code, modifiers: KeyModifiers::none() }
    }

    fn ch(c: char) -> KeyEvent2 {
        key(KeyCode::Char(c))
    }

    fn ctrl(c: char) -> KeyEvent2 {
        let mut k = ch(c);
        k.modifiers.ctrl = true;
        k
    }

    fn type_text(o: &mut Overlay, text: &str) {
        for c in text.chars() {
            o.handle_key(&ch(c));
        }
    }

    #[test]
    fn typing_filters_the_menu() {
        let mut o = Overlay::menu();
        type_text(&mut o, "save");
        let labels: Vec<_> = o.model().rows.into_iter().filter(|r| r.selectable).map(|r| r.text).collect();
        assert_eq!(labels, vec!["Save", "Save as..."]);
    }

    #[test]
    fn filter_is_case_insensitive_and_matches_each_word() {
        assert!(matches("HIGHLIGHT", "Install syntax highlighting..."));
        assert!(matches("file reload", "Reload file (F5)"));
        assert!(matches("", "anything"));
        assert!(!matches("rld", "Reload file (F5)"));
        assert!(!matches("zz", "Save"));
    }

    #[test]
    fn enter_picks_from_the_filtered_list() {
        let mut o = Overlay::menu();
        type_text(&mut o, "open");
        assert_eq!(o.handle_key(&key(KeyCode::Enter)), Outcome::Action(MenuAction::Open));
    }

    #[test]
    fn arrows_tab_and_ctrl_keys_move_and_wrap() {
        let mut o = Overlay::menu();
        o.handle_key(&key(KeyCode::Down));
        assert!(matches!(o, Overlay::Menu { selected: 1, .. }));
        o.handle_key(&ctrl('n'));
        o.handle_key(&key(KeyCode::Tab));
        assert!(matches!(o, Overlay::Menu { selected: 3, .. }));
        o.handle_key(&ctrl('p'));
        o.handle_key(&ctrl('k'));
        o.handle_key(&key(KeyCode::Up));
        o.handle_key(&key(KeyCode::BackTab));
        assert!(matches!(o, Overlay::Menu { selected: 7, .. }));
    }

    #[test]
    fn letters_j_and_k_are_search_text_not_navigation() {
        let mut o = Overlay::menu();
        o.handle_key(&ch('j'));
        assert!(matches!(&o, Overlay::Menu { query, selected: 0 } if query == "j"));
    }

    #[test]
    fn escape_clears_the_query_then_closes() {
        let mut o = Overlay::menu();
        type_text(&mut o, "qu");
        assert_eq!(o.handle_key(&key(KeyCode::Escape)), Outcome::Stay);
        assert!(matches!(&o, Overlay::Menu { query, .. } if query.is_empty()));
        assert_eq!(o.handle_key(&key(KeyCode::Escape)), Outcome::Close);
    }

    #[test]
    fn backspace_edits_the_query() {
        let mut o = Overlay::menu();
        type_text(&mut o, "sx");
        o.handle_key(&key(KeyCode::Backspace));
        assert!(matches!(&o, Overlay::Menu { query, .. } if query == "s"));
    }

    #[test]
    fn f1_and_ctrl_space_close() {
        assert_eq!(Overlay::menu().handle_key(&key(KeyCode::F(1))), Outcome::Close);
        assert_eq!(Overlay::menu().handle_key(&ctrl(' ')), Outcome::Close);
        assert_eq!(Overlay::menu().handle_key(&key(KeyCode::Null)), Outcome::Close);
    }

    #[test]
    fn space_is_typed_into_the_search() {
        let mut o = Overlay::menu();
        type_text(&mut o, "save as");
        assert_eq!(o.handle_key(&key(KeyCode::Enter)), Outcome::Action(MenuAction::SaveAs));
    }

    #[test]
    fn no_matches_shows_a_notice_and_enter_does_nothing() {
        let mut o = Overlay::menu();
        type_text(&mut o, "zzz");
        let model = o.model();
        assert_eq!(model.rows.len(), 1);
        assert_eq!(model.selected, None);
        assert_eq!(o.handle_key(&key(KeyCode::Enter)), Outcome::Stay);
    }

    #[test]
    fn menu_hides_sections_without_matches() {
        let mut o = Overlay::menu();
        type_text(&mut o, "quit");
        let headers: Vec<_> = o.model().rows.into_iter().filter(|r| !r.selectable).map(|r| r.text).collect();
        assert_eq!(headers, vec!["Session"]);
    }

    #[test]
    fn picker_filters_languages() {
        let mut o = Overlay::pick(PickKind::Grammar, vec!["rust".into(), "python".into(), "json".into()]);
        type_text(&mut o, "py");
        assert_eq!(
            o.handle_key(&key(KeyCode::Enter)),
            Outcome::Picked(PickKind::Grammar, "python".into())
        );
    }

    #[test]
    fn input_collects_text_and_submits_trimmed() {
        let mut o = Overlay::input(InputKind::SaveAs, String::new());
        type_text(&mut o, " a.py");
        o.handle_key(&key(KeyCode::Backspace));
        o.handle_key(&ch('y'));
        assert_eq!(
            o.handle_key(&key(KeyCode::Enter)),
            Outcome::Submitted(InputKind::SaveAs, "a.py".into())
        );
    }

    #[test]
    fn empty_input_does_not_submit_and_escape_closes() {
        let mut o = Overlay::input(InputKind::Open, String::new());
        assert_eq!(o.handle_key(&key(KeyCode::Enter)), Outcome::Stay);
        assert_eq!(o.handle_key(&key(KeyCode::Escape)), Outcome::Close);
    }
}
