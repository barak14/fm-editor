//! Persistent log of game-build detection reports.
//!
//! Release builds on Windows have no visible console, so detection reports
//! written only to stderr are lost. Each distinct report is appended as one
//! JSON line to `<local data dir>/fm-editor/logs/game-detection.log` and also
//! printed to stderr. Auto-connect polls every few seconds, so identical repeat
//! reports for the same PID are suppressed; a changed report is logged again.
//!
//! Logging is best-effort: any failure is reported on stderr and never affects
//! attachment.

use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_FILE_NAME: &str = "game-detection.log";
const BACKUP_FILE_NAME: &str = "game-detection.log.1";
/// Persistent lock file serializing log writes across editor processes.
const LOCK_FILE_NAME: &str = "game-detection.lock";
/// Rotate the log before it would grow beyond this size.
const MAX_LOG_BYTES: u64 = 512 * 1024;
/// Longest report body kept per record, before the truncation marker.
const MAX_REPORT_BYTES: usize = 32 * 1024;
/// Number of PIDs whose last report is remembered for deduplication.
const MAX_REMEMBERED_PIDS: usize = 64;

/// Last report seen per PID, oldest-touched first.
#[derive(Default)]
struct Dedup {
    entries: VecDeque<(u32, String)>,
}

impl Dedup {
    /// Remembers `report` for `pid`. Returns `false` when it repeats the last
    /// report already remembered for that PID.
    fn observe(&mut self, pid: u32, report: &str) -> bool {
        if let Some(index) = self.entries.iter().position(|(p, _)| *p == pid) {
            let (_, previous) = self.entries.remove(index).unwrap_or_default();
            let changed = previous != report;
            self.entries.push_back((pid, report.to_owned()));
            return changed;
        }
        while self.entries.len() >= MAX_REMEMBERED_PIDS {
            self.entries.pop_front();
        }
        self.entries.push_back((pid, report.to_owned()));
        true
    }

    /// Drops the remembered report for `pid` so its next report is treated as
    /// new; used when persisting it failed, so a later poll retries.
    fn forget(&mut self, pid: u32) {
        self.entries.retain(|(p, _)| *p != pid);
    }
}

/// Process-local dedup state; `VecDeque::new` is const, so no lazy cell is needed.
static DEDUP: Mutex<Dedup> = Mutex::new(Dedup {
    entries: VecDeque::new(),
});

fn default_log_dir() -> Option<PathBuf> {
    dirs::data_local_dir().map(|dir| dir.join("fm-editor").join("logs"))
}

/// Records a detection report for `pid`: printed to stderr and appended to the
/// persistent log, unless it repeats the previous report for the same PID.
pub(super) fn record(pid: u32, report: &str) {
    let report = cap_report(report);
    // The guard is held through stderr, append and rotation so concurrent
    // callers in this process cannot interleave records or race the rename.
    // A panic while holding the lock leaves the dedup map usable; recover it.
    let mut dedup = DEDUP.lock().unwrap_or_else(PoisonError::into_inner);
    if !dedup.observe(pid, &report) {
        return;
    }
    eprintln!("[fm-editor] game detection (pid {pid}):\n{report}");
    let Some(dir) = default_log_dir() else {
        eprintln!("[fm-editor] warning: game detection report not saved: no local data directory");
        dedup.forget(pid);
        return;
    };
    if let Err(err) = append_record(&dir, pid, &report, unix_now()) {
        eprintln!(
            "[fm-editor] warning: could not write game detection log in {}: {err}",
            dir.display()
        );
        // Not persisted: let the next poll retry instead of deduplicating it.
        dedup.forget(pid);
    }
    drop(dedup);
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Caps `report` at `MAX_REPORT_BYTES` on a UTF-8 boundary, appending an
/// explicit marker when anything was cut.
fn cap_report(report: &str) -> String {
    if report.len() <= MAX_REPORT_BYTES {
        return report.to_owned();
    }
    let mut end = MAX_REPORT_BYTES;
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}\n[report truncated: kept {end} of {} bytes]",
        &report[..end],
        report.len()
    )
}

fn format_record(timestamp: u64, pid: u32, report: &str) -> String {
    let value = serde_json::json!({
        "timestamp": timestamp,
        "editor_version": env!("CARGO_PKG_VERSION"),
        "platform": format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH),
        "pid": pid,
        "report": report,
    });
    let mut line = value.to_string();
    line.push('\n');
    line
}

/// Appends one JSONL record under `dir`, rotating to a single backup first if
/// the record would push the log past `MAX_LOG_BYTES`.
fn append_record(dir: &Path, pid: u32, report: &str, timestamp: u64) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let _lock = try_lock_dir(dir)?;
    let path = dir.join(LOG_FILE_NAME);
    let line = format_record(timestamp, pid, report);
    let current = match fs::metadata(&path) {
        Ok(meta) => meta.len(),
        Err(err) if err.kind() == io::ErrorKind::NotFound => 0,
        Err(err) => return Err(err),
    };
    if current > 0 && current + line.len() as u64 > MAX_LOG_BYTES {
        let backup = dir.join(BACKUP_FILE_NAME);
        match fs::remove_file(&backup) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        fs::rename(&path, &backup)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
    file.write_all(line.as_bytes())
}

/// Takes an exclusive, non-blocking lock on the persistent lock file in `dir`,
/// serializing metadata/rotation/append across editor processes. The lock is
/// released when the returned file drops; the lock file itself is never
/// removed. Contention returns `WouldBlock` so logging never stalls attachment.
fn try_lock_dir(dir: &Path) -> io::Result<fs::File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(LOCK_FILE_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(fs::TryLockError::WouldBlock) => Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "game detection log is locked by another editor process",
        )),
        Err(fs::TryLockError::Error(err)) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Fresh, unique directory under the system temp dir; never user data.
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fm-editor-detection-log-{tag}-{}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn read_records(path: &Path) -> Vec<serde_json::Value> {
        fs::read_to_string(path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[test]
    fn dedup_suppresses_identical_repeats_per_pid() {
        let mut dedup = Dedup::default();
        assert!(dedup.observe(10, "a"));
        assert!(!dedup.observe(10, "a"));
        assert!(dedup.observe(10, "b"), "changed report logs again");
        assert!(!dedup.observe(10, "b"));
        assert!(dedup.observe(10, "a"), "reverting is also a change");
        assert!(dedup.observe(11, "a"), "different PID is independent");
        assert!(!dedup.observe(11, "a"));
        assert!(!dedup.observe(10, "a"));
    }

    #[test]
    fn forget_permits_retry_of_same_report_for_that_pid_only() {
        let mut dedup = Dedup::default();
        assert!(dedup.observe(10, "a"));
        assert!(dedup.observe(11, "a"));
        dedup.forget(10);
        assert!(
            dedup.observe(10, "a"),
            "forgotten pid retries identical report"
        );
        assert!(!dedup.observe(10, "a"), "dedup resumes after a retry");
        assert!(!dedup.observe(11, "a"), "other pids unaffected");
        dedup.forget(99); // unknown pid is a no-op
        assert_eq!(dedup.entries.len(), 2);
    }

    #[test]
    fn dedup_remembers_at_most_64_pids() {
        let mut dedup = Dedup::default();
        for pid in 0..MAX_REMEMBERED_PIDS as u32 {
            assert!(dedup.observe(pid, "r"));
        }
        // Touch pid 0 so pid 1 becomes the oldest.
        assert!(!dedup.observe(0, "r"));
        assert!(dedup.observe(1000, "r"));
        assert_eq!(dedup.entries.len(), MAX_REMEMBERED_PIDS);
        assert!(!dedup.observe(0, "r"), "recently touched pid retained");
        assert!(dedup.observe(1, "r"), "oldest pid was evicted");
        assert_eq!(dedup.entries.len(), MAX_REMEMBERED_PIDS);
    }

    #[test]
    fn records_are_jsonl_with_escaped_multiline_report() {
        let dir = temp_dir("jsonl");
        let report = "line one\nline \"two\"\tC:\\fm.exe\r\n\u{e9}";
        append_record(&dir, 4242, report, 1_700_000_000).unwrap();
        append_record(&dir, 7, "second", 1_700_000_003).unwrap();
        let text = fs::read_to_string(dir.join(LOG_FILE_NAME)).unwrap();
        assert_eq!(text.lines().count(), 2, "one physical line per record");
        let records = read_records(&dir.join(LOG_FILE_NAME));
        assert_eq!(records[0]["report"], report);
        assert_eq!(records[0]["pid"], 4242);
        assert_eq!(records[0]["timestamp"], 1_700_000_000u64);
        assert_eq!(records[0]["editor_version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(
            records[0]["platform"],
            format!("{}/{}", std::env::consts::OS, std::env::consts::ARCH)
        );
        assert_eq!(records[1]["report"], "second");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rotation_keeps_exactly_one_backup() {
        let dir = temp_dir("rotate");
        let report = "x".repeat(MAX_REPORT_BYTES);
        // ~32KiB per record: enough appends to rotate more than once.
        for i in 0..40 {
            append_record(&dir, i, &report, 0).unwrap();
        }
        let log = dir.join(LOG_FILE_NAME);
        let backup = dir.join(BACKUP_FILE_NAME);
        assert!(fs::metadata(&log).unwrap().len() <= MAX_LOG_BYTES);
        assert!(fs::metadata(&backup).unwrap().len() <= MAX_LOG_BYTES);
        let mut names: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(
            names,
            [LOCK_FILE_NAME, LOG_FILE_NAME, BACKUP_FILE_NAME],
            "only log, one backup and the persistent lock file"
        );
        // Newest record is in the live log; backup holds the preceding ones.
        let live = read_records(&log);
        assert_eq!(live.last().unwrap()["pid"], 39);
        let old = read_records(&backup);
        assert_eq!(
            old.last().unwrap()["pid"].as_u64().unwrap() + 1,
            live[0]["pid"].as_u64().unwrap()
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn long_report_is_truncated_on_char_boundary_with_marker() {
        let short = "short report";
        assert_eq!(cap_report(short), short);

        // Two-byte chars after one ASCII byte: the cap falls mid-char.
        let long = format!("a{}", "\u{e9}".repeat(MAX_REPORT_BYTES));
        let capped = cap_report(&long);
        let (body, marker) = capped.rsplit_once('\n').unwrap();
        assert!(body.len() <= MAX_REPORT_BYTES);
        assert_eq!(body.len(), MAX_REPORT_BYTES - 1);
        assert!(long.starts_with(body));
        assert_eq!(
            marker,
            format!(
                "[report truncated: kept {} of {} bytes]",
                body.len(),
                long.len()
            )
        );
    }

    #[test]
    fn write_errors_are_returned_not_panicked() {
        let dir = temp_dir("error");
        fs::create_dir_all(&dir).unwrap();
        // A regular file where the log directory should be.
        let blocker = dir.join("not-a-dir");
        fs::write(&blocker, b"").unwrap();
        assert!(append_record(&blocker, 1, "report", 0).is_err());
        // The log path itself occupied by a directory.
        let occupied = dir.join("occupied");
        fs::create_dir_all(occupied.join(LOG_FILE_NAME)).unwrap();
        assert!(append_record(&occupied, 1, "report", 0).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn lock_contention_fails_fast_then_succeeds_after_unlock() {
        let dir = temp_dir("lock");
        fs::create_dir_all(&dir).unwrap();
        // Another holder (a separate open handle, as another process would have).
        let holder = try_lock_dir(&dir).unwrap();
        let err = append_record(&dir, 1, "blocked", 0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::WouldBlock);
        assert!(
            !dir.join(LOG_FILE_NAME).exists(),
            "nothing written while locked"
        );
        drop(holder);
        append_record(&dir, 1, "after unlock", 0).unwrap();
        assert_eq!(
            read_records(&dir.join(LOG_FILE_NAME))[0]["report"],
            "after unlock"
        );
        assert!(dir.join(LOCK_FILE_NAME).exists(), "lock file persists");
        let _ = fs::remove_dir_all(&dir);
    }
}
