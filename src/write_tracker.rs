//! Correlates write acknowledgements with the exact edit or undo submitted.
//!
//! Entries are returned only once their write completes. The caller decides
//! whether a confirmed edit belongs on the undo stack or a failed undo must be
//! restored there. Clearing pending work invalidates late acknowledgements.

use std::collections::HashMap;

use crate::bridge::BridgeCmd;
use crate::undo::UndoEntry;

pub struct WriteCompletion {
    pub entry: UndoEntry,
    pub was_undo: bool,
    pub success: bool,
}

struct PendingWrite {
    entry: UndoEntry,
    was_undo: bool,
}

pub struct WriteTracker {
    last_id: u64,
    pending: HashMap<u64, PendingWrite>,
}

impl WriteTracker {
    pub fn new() -> Self {
        Self {
            last_id: 0,
            pending: HashMap::new(),
        }
    }

    pub fn is_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Each edit, including mirror writes, gets its own acknowledgement.
    pub fn submit(&mut self, edits: Vec<UndoEntry>) -> Vec<BridgeCmd> {
        edits
            .into_iter()
            .map(|entry| self.track(entry, false))
            .collect()
    }

    /// Write the original bytes while retaining the original entry so a failed
    /// undo can be restored to the undo stack without reversing it twice.
    pub fn submit_undo(&mut self, entry: UndoEntry) -> BridgeCmd {
        self.track(entry, true)
    }

    fn track(&mut self, entry: UndoEntry, was_undo: bool) -> BridgeCmd {
        // Exhaustion must not wrap and accidentally acknowledge another write.
        self.last_id = self.last_id.checked_add(1).expect("write ID exhausted");
        let id = self.last_id;
        let cmd = BridgeCmd::WriteBytes {
            id,
            addr: entry.addr,
            data: if was_undo {
                entry.old_raw.clone()
            } else {
                entry.new_raw.clone()
            },
        };
        self.pending.insert(id, PendingWrite { entry, was_undo });
        cmd
    }

    /// Unknown, duplicate, and cleared IDs never complete another entry.
    pub fn complete(&mut self, id: u64, success: bool) -> Option<WriteCompletion> {
        self.pending.remove(&id).map(|pending| WriteCompletion {
            entry: pending.entry,
            was_undo: pending.was_undo,
            success,
        })
    }

    /// Forget save-address state without ever reusing its write IDs.
    pub fn clear(&mut self) {
        self.pending.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn entry(addr: u64) -> UndoEntry {
        UndoEntry {
            description: format!("Edit {addr}"),
            addr,
            old_raw: vec![1, 2],
            new_raw: vec![3, 4],
            timestamp: Instant::now(),
        }
    }

    fn write(cmd: BridgeCmd) -> (u64, u64, Vec<u8>) {
        match cmd {
            BridgeCmd::WriteBytes { id, addr, data } => (id, addr, data),
            _ => panic!("expected a write command"),
        }
    }

    #[test]
    fn edit_waits_for_confirmation_and_completes_once() {
        let mut tracker = WriteTracker::new();
        assert!(!tracker.is_pending());
        let original = entry(100);
        let timestamp = original.timestamp;
        let (id, addr, data) = write(tracker.submit(vec![original]).remove(0));
        assert_eq!(addr, 100);
        assert_eq!(data, vec![3, 4]);
        assert!(tracker.is_pending());
        assert!(tracker.complete(id + 1, true).is_none());
        assert!(tracker.is_pending());
        let completed = tracker.complete(id, true).unwrap();
        assert!(completed.success);
        assert!(!completed.was_undo);
        assert_eq!(completed.entry.addr, 100);
        assert_eq!(completed.entry.old_raw, vec![1, 2]);
        assert_eq!(completed.entry.new_raw, vec![3, 4]);
        assert_eq!(completed.entry.timestamp, timestamp);
        assert!(!tracker.is_pending());
        assert!(tracker.complete(id, true).is_none());
    }

    #[test]
    fn failed_edit_is_not_reported_as_confirmed() {
        let mut tracker = WriteTracker::new();
        let (id, _, _) = write(tracker.submit(vec![entry(100)]).remove(0));
        let completed = tracker.complete(id, false).unwrap();
        assert!(!completed.success);
        assert!(!completed.was_undo);
        assert_eq!(completed.entry.addr, 100);
        assert!(!tracker.is_pending());
    }

    #[test]
    fn undo_writes_old_bytes_and_preserves_entry_for_failed_restore() {
        for success in [false, true] {
            let mut tracker = WriteTracker::new();
            let (id, addr, data) = write(tracker.submit_undo(entry(100)));
            assert_eq!(addr, 100);
            assert_eq!(data, vec![1, 2]);
            assert!(tracker.is_pending());
            let completed = tracker.complete(id, success).unwrap();
            assert!(completed.was_undo);
            assert_eq!(completed.success, success);
            assert_eq!(completed.entry.old_raw, vec![1, 2]);
            assert_eq!(completed.entry.new_raw, vec![3, 4]);
            assert!(!tracker.is_pending());
        }
    }

    #[test]
    fn clear_rejects_stale_ids_without_reusing_them() {
        let mut tracker = WriteTracker::new();
        let (old_id, _, _) = write(tracker.submit(vec![entry(100)]).remove(0));
        tracker.clear();
        assert!(!tracker.is_pending());
        let (new_id, _, _) = write(tracker.submit_undo(entry(200)));
        assert!(new_id > old_id);
        assert!(tracker.complete(old_id, true).is_none());
        assert!(tracker.is_pending());
        assert_eq!(tracker.complete(new_id, true).unwrap().entry.addr, 200);
    }

    #[test]
    fn out_of_order_responses_keep_distinct_mirror_writes() {
        let mut tracker = WriteTracker::new();
        let mut commands = tracker.submit(vec![entry(100), entry(200)]).into_iter();
        let (first, _, _) = write(commands.next().unwrap());
        let (second, _, _) = write(commands.next().unwrap());
        assert!(second > first);
        let completed = tracker.complete(second, true).unwrap();
        assert_eq!(completed.entry.addr, 200);
        assert!(tracker.is_pending());
        let completed = tracker.complete(first, false).unwrap();
        assert_eq!(completed.entry.addr, 100);
        assert!(!completed.success);
        assert!(!tracker.is_pending());
    }

    #[test]
    fn empty_submission_does_not_create_pending_work() {
        let mut tracker = WriteTracker::new();
        assert!(tracker.submit(Vec::new()).is_empty());
        assert!(!tracker.is_pending());
    }
}
