use crate::buffer::Change;

// ─── Node ────────────────────────────────────────────────────────────────────

struct Node {
    change: Change,
    parent: Option<usize>,
    children: Vec<usize>,
    /// Which child branch was visited most recently (drives `redo`).
    last_visited_child: Option<usize>,
}

// ─── UndoTree ────────────────────────────────────────────────────────────────

/// Branching undo history (Helix-style).
///
/// Typing after an undo creates a new branch rather than erasing future states.
/// `redo` follows the most recently visited branch at each node.
pub struct UndoTree {
    nodes: Vec<Node>,
    /// `None` = at initial (pristine) state.
    head: Option<usize>,
    /// Last node whose parent is `None` (used by `redo` from initial state).
    root_last_child: Option<usize>,
}

impl UndoTree {
    pub fn new() -> Self {
        Self { nodes: Vec::new(), head: None, root_last_child: None }
    }

    /// Record `change` as the new head, branching if we're mid-history.
    pub fn record(&mut self, change: Change) {
        let idx = self.nodes.len();
        let parent = self.head;
        self.nodes.push(Node {
            change,
            parent,
            children: Vec::new(),
            last_visited_child: None,
        });
        match parent {
            Some(p) => {
                if let Some(node) = self.nodes.get_mut(p) {
                    node.children.push(idx);
                    node.last_visited_child = Some(idx);
                }
            }
            None => {
                self.root_last_child = Some(idx);
            }
        }
        self.head = Some(idx);
    }

    /// Step backward: returns the `Change` to revert, or `None` if already at
    /// initial state. Caller must call `buffer.revert(change)`.
    pub fn undo(&mut self) -> Option<&Change> {
        let current = self.head?;
        let parent = self.nodes.get(current)?.parent;
        self.head = parent;
        self.nodes.get(current).map(|n| &n.change)
    }

    /// Step forward along the most recently visited branch.
    /// Returns the `Change` to re-apply, or `None` if nothing to redo.
    pub fn redo(&mut self) -> Option<&Change> {
        let child_idx = match self.head {
            None => self.root_last_child?,
            Some(h) => self.nodes.get(h)?.last_visited_child?,
        };
        self.head = Some(child_idx);
        self.nodes.get(child_idx).map(|n| &n.change)
    }

    pub fn can_undo(&self) -> bool {
        self.head.is_some()
    }

    pub fn can_redo(&self) -> bool {
        match self.head {
            None => self.root_last_child.is_some(),
            Some(h) => self.nodes.get(h)
                .and_then(|n| n.last_visited_child)
                .is_some(),
        }
    }
}

impl Default for UndoTree {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{Buffer, Edit};

    fn make_change(buf: &mut Buffer, text: &str) -> Change {
        let at = buf.len_bytes();
        buf.apply(&Edit::insert(at, text)).unwrap()
    }

    #[test]
    fn linear_undo_redo() {
        let mut buf = Buffer::new();
        let mut tree = UndoTree::new();

        let c1 = make_change(&mut buf, "hello");
        tree.record(c1);
        let c2 = make_change(&mut buf, " world");
        tree.record(c2);
        assert_eq!(buf.to_string(), "hello world");

        // Undo "world"
        let change = tree.undo().unwrap();
        buf.revert(change).unwrap();
        assert_eq!(buf.to_string(), "hello");

        // Redo "world"
        let change = tree.redo().unwrap();
        buf.apply(&Edit::insert(change.start, &change.new_text)).unwrap();
        assert_eq!(buf.to_string(), "hello world");
    }

    #[test]
    fn branch_on_type_after_undo() {
        let mut buf = Buffer::new();
        let mut tree = UndoTree::new();

        let c1 = make_change(&mut buf, "a");
        tree.record(c1);
        let c2 = make_change(&mut buf, "b");
        tree.record(c2);

        // Undo to "a"
        let ch = tree.undo().unwrap().clone();
        buf.revert(&ch).unwrap();
        assert_eq!(buf.to_string(), "a");

        // Type "c" — creates a new branch
        let c3 = make_change(&mut buf, "c");
        tree.record(c3);
        assert_eq!(buf.to_string(), "ac");

        // Undo again
        let ch = tree.undo().unwrap().clone();
        buf.revert(&ch).unwrap();
        assert_eq!(buf.to_string(), "a");

        // Redo goes to "ac" branch (most recently visited)
        let ch = tree.redo().unwrap();
        buf.apply(&Edit::insert(ch.start, &ch.new_text)).unwrap();
        assert_eq!(buf.to_string(), "ac");
    }

    #[test]
    fn nothing_to_undo_returns_none() {
        let mut tree = UndoTree::new();
        assert!(tree.undo().is_none());
        assert!(!tree.can_undo());
    }

    #[test]
    fn nothing_to_redo_returns_none() {
        let mut buf = Buffer::new();
        let mut tree = UndoTree::new();
        let c = make_change(&mut buf, "x");
        tree.record(c);
        assert!(tree.redo().is_none());
        assert!(!tree.can_redo());
    }
}
