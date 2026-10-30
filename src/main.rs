//! Main binary — thin wrapper around `fm_editor::app::FmEditorApp`.
//! All shared code lives in the library crate at `src/lib.rs`.

// Suppress the console window in release builds. Debug builds keep it so
// panics and stderr remain visible in the terminal.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use fm_editor::app::FmEditorApp;
use fm_editor::settings::AppSettings;

fn main() -> eframe::Result {
    // Debug-only: start the MCP server before the GUI event loop so early
    // bridge errors are captured. The server runs on a dedicated tokio thread
    // and does not touch the eframe loop.
    // Parse CLI args once; shared between debug and title logic.
    let args: Vec<String> = std::env::args().collect();

    // --branch <name>  or  --branch=<name>  — injected by build-and-launch.sh
    // for non-main worktrees so the window title shows which branch is running.
    let branch: Option<String> = args
        .windows(2)
        .find_map(|w| {
            if w[0] == "--branch" {
                Some(w[1].clone())
            } else {
                None
            }
        })
        .or_else(|| {
            args.iter()
                .find_map(|a| a.strip_prefix("--branch=").map(|s| s.to_string()))
        });

    #[cfg(debug_assertions)]
    let mcp_state = {
        // --mcp-port=7799  or  --mcp-port 7799  (default: 7799)
        let port: u16 = args
            .windows(2)
            .find_map(|w| {
                if w[0] == "--mcp-port" {
                    w[1].parse().ok()
                } else {
                    None
                }
            })
            .or_else(|| {
                args.iter()
                    .find_map(|a| a.strip_prefix("--mcp-port=").and_then(|s| s.parse().ok()))
            })
            .unwrap_or(7799);
        let log = fm_editor::debug::new_log();
        let state = fm_editor::debug::start(log, port);

        // Route panics through the MCP log buffer so `read_logs` captures them.
        // Keeps the default hook as fallback (still writes to stderr / log file).
        let panic_log = state.log.clone();
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            fm_editor::debug::log_push(&panic_log, "PANIC", info.to_string());
            prev_hook(info);
        }));

        state
    };

    let title = match branch.as_deref() {
        Some(b) => format!("FM Editor [{b}]"),
        None => "FM Editor".to_string(),
    };

    let settings = AppSettings::load();
    let f = settings.scale.zoom_factor();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0 * f, 800.0 * f])
            .with_title(title),
        ..Default::default()
    };
    eframe::run_native(
        "FM Editor",
        options,
        Box::new(move |cc| {
            let app = FmEditorApp::new(cc).with_branch(branch);
            #[cfg(debug_assertions)]
            let app = app.with_mcp_state(mcp_state);
            Ok(Box::new(app))
        }),
    )
}
