use std::time::Instant;

#[derive(Clone)]
pub struct UndoEntry {
    pub description: String,
    pub addr: u64,
    pub old_raw: Vec<u8>,
    pub new_raw: Vec<u8>,
    pub timestamp: Instant,
}

pub struct UndoStack {
    entries: Vec<UndoEntry>,
    max_entries: usize,
}

impl UndoStack {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            max_entries: 100,
        }
    }

    pub fn push(&mut self, entry: UndoEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.remove(0);
        }
        self.entries.push(entry);
    }

    pub fn pop(&mut self) -> Option<UndoEntry> {
        self.entries.pop()
    }

    pub fn last(&self) -> Option<&UndoEntry> {
        self.entries.last()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries(&self) -> &[UndoEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_preserves_entry_and_clear_discards_history() {
        let mut stack = UndoStack::new();
        assert!(stack.last().is_none());
        stack.push(UndoEntry {
            description: "Edit".to_owned(),
            addr: 100,
            old_raw: vec![1],
            new_raw: vec![2],
            timestamp: Instant::now(),
        });
        assert_eq!(stack.last().unwrap().description, "Edit");
        assert_eq!(stack.len(), 1);
        assert_eq!(stack.pop().unwrap().addr, 100);
        assert!(stack.last().is_none());
        stack.push(UndoEntry {
            description: "Another edit".to_owned(),
            addr: 200,
            old_raw: vec![2],
            new_raw: vec![3],
            timestamp: Instant::now(),
        });
        stack.clear();
        assert!(stack.is_empty());
        assert!(stack.last().is_none());
        assert!(stack.pop().is_none());
    }
}
