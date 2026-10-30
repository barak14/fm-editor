//! Debug-only MCP server — compiled only when `debug_assertions` is set.
//!
//! Exposes 13 MCP tools via streamable HTTP on `localhost:7799` so an agent can probe
//! fm.exe memory directly during a live session — finding new field offsets,
//! diagnosing blank names, discovering sub-records — without recompiling.
//!
//! Entry point: call `debug::start(log)` once from `main()` before the eframe
//! event loop. Everything else is self-contained.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicU64;

pub mod mcp_server;
pub mod tools;

// ---------------------------------------------------------------------------
// Log buffer — bridge errors and panics are captured here, drained via
// the `read_logs` MCP tool so Claude can diagnose issues without a terminal.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct LogEntry {
    pub level:     &'static str,
    pub message:   String,
    pub timestamp: String,
}

pub type DebugLog = Arc<Mutex<VecDeque<LogEntry>>>;

pub fn new_log() -> DebugLog {
    Arc::new(Mutex::new(VecDeque::with_capacity(500)))
}

pub fn log_push(log: &DebugLog, level: &'static str, msg: impl Into<String>) {
    let mut g = log.lock().unwrap();
    if g.len() >= 500 {
        g.pop_front();
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    g.push_back(LogEntry {
        level,
        message:   msg.into(),
        timestamp: secs.to_string(),
    });
}

// ---------------------------------------------------------------------------
// Mini entity cache — built lazily on first search_entities call.
// Stores uid + addr + index + name for each entity, enabling fast name/UID
// search without the GUI's EntityCache.
// ---------------------------------------------------------------------------

pub struct EntityEntry {
    pub uid:   u32,
    pub addr:  u64,
    pub index: usize,
    pub name:  String,
}

pub struct MiniCache {
    pub players: Vec<EntityEntry>,
    pub staff:   Vec<EntityEntry>,
    pub clubs:   Vec<EntityEntry>,
}

// ---------------------------------------------------------------------------
// Shared debug state — Arc'd into every axum handler.
// ---------------------------------------------------------------------------

pub struct DebugState {
    /// Own FmProcess — separate from the GUI bridge. Both attach to fm.exe
    /// independently; cross-process reads (Win32 ReadProcessMemory / Linux
    /// process_vm_readv) support concurrent access from multiple attachers.
    pub fm:         Mutex<Option<crate::process::FmProcess>>,
    pub log:        DebugLog,
    /// Entity name/UID cache built lazily on first search_entities call.
    pub mini_cache: Mutex<Option<MiniCache>>,
    /// TCP port the MCP server is bound to (default 7799, overridden by --mcp-port).
    pub port:             u16,
    /// Unix-ms timestamp of the last MCP tool call — polled by the GUI LED.
    pub last_activity_ms: AtomicU64,
}

// SAFETY: DebugState holds FmProcess behind a Mutex. The cross-process read
// primitives on both platforms are thread-safe; the OS serialises access.
// All other fields are Send+Sync.
unsafe impl Send for DebugState {}
unsafe impl Sync for DebugState {}

impl DebugState {
    pub fn new(log: DebugLog, port: u16) -> Self {
        Self {
            fm:               Mutex::new(None),
            log,
            mini_cache:       Mutex::new(None),
            port,
            last_activity_ms: AtomicU64::new(0),
        }
    }
}

// ---------------------------------------------------------------------------
// Server start — spawns a dedicated tokio runtime thread so the MCP server
// never touches the eframe event loop.
// ---------------------------------------------------------------------------

pub fn start(log: DebugLog, port: u16) -> Arc<DebugState> {
    let state = Arc::new(DebugState::new(log, port));
    let srv   = Arc::clone(&state);
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new()
            .expect("debug mcp: failed to create tokio runtime");
        rt.block_on(async move {
            if let Err(e) = mcp_server::run(srv).await {
                eprintln!("MCP server crashed: {e}");
            }
        });
    });
    state
}
