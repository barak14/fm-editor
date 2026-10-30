//! Axum HTTP server speaking MCP's streamable HTTP transport on `localhost:7799`.
//!
//! Routes:
//!   POST /mcp — one JSON-RPC 2.0 message per request. A request gets its
//!               response as an `application/json` body; a notification or a
//!               client response gets `202 Accepted` with no body.
//!   GET  /mcp — `405`. The server never initiates messages, so it offers no
//!               server-to-client event stream.
//!
//! Stateless: no `Mcp-Session-Id` is issued, so any number of clients can
//! connect at once (Claude Code `"type": "http"`, rusty `transport = "http"`).

use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::post,
};
use serde_json::{json, Value};

use super::{DebugState, log_push};

pub type SharedState = Arc<DebugState>;

/// Protocol revisions this server speaks, newest first. `initialize` echoes
/// the client's requested revision when listed, otherwise offers the newest.
const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26"];

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub async fn run(state: SharedState) -> Result<(), String> {
    let port = state.port;

    let app = Router::new()
        .route(
            "/mcp",
            post(mcp_post).get(|| async { StatusCode::METHOD_NOT_ALLOWED }),
        )
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| format!("bind {port} failed: {e}"))?;

    axum::serve(listener, app)
        .await
        .map_err(|e| format!("axum serve: {e}"))
}

// ---------------------------------------------------------------------------
// POST /mcp — JSON-RPC entry
// ---------------------------------------------------------------------------

async fn mcp_post(State(state): State<SharedState>, body: String) -> Response {
    let req: Value = match serde_json::from_str(&body) {
        Ok(v) => v,
        Err(e) => {
            let resp = json!({
                "jsonrpc": "2.0", "id": null,
                "error": { "code": -32700, "message": format!("Parse error: {e}") }
            });
            return (StatusCode::BAD_REQUEST, Json(resp)).into_response();
        }
    };

    // Notifications (no `id`) and client responses (no `method`) need no reply.
    if req.get("method").is_none() || req.get("id").is_none() {
        return StatusCode::ACCEPTED.into_response();
    }

    Json(dispatch(req, &state).await).into_response()
}

// ---------------------------------------------------------------------------
// MCP JSON-RPC dispatcher
// ---------------------------------------------------------------------------

async fn dispatch(req: Value, state: &DebugState) -> Value {
    let id     = req.get("id").cloned().unwrap_or(Value::Null);
    let method = req["method"].as_str().unwrap_or("");
    let params = req.get("params").cloned().unwrap_or(Value::Object(Default::default()));

    let result: Result<Value, String> = match method {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("");
            let version = PROTOCOL_VERSIONS
                .iter()
                .copied()
                .find(|v| *v == requested)
                .unwrap_or(PROTOCOL_VERSIONS[0]);
            Ok(json!({
                "protocolVersion": version,
                "capabilities":    { "tools": {} },
                "serverInfo":      { "name": "fm-debug", "version": "0.1.0" }
            }))
        }

        "tools/list" => Ok(json!({ "tools": tool_list() })),

        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or_default();
            call_tool(name, args, state).await
        }

        other => Err(format!("Unknown method: {other}")),
    };

    if let Err(e) = &result {
        log_push(&state.log, "error", format!("dispatch error [{method}]: {e}"));
    }

    match result {
        Ok(r)  => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id,
                          "error": { "code": -32000, "message": e } }),
    }
}

// ---------------------------------------------------------------------------
// Tool call dispatcher
// ---------------------------------------------------------------------------

async fn call_tool(name: &str, args: Value, state: &DebugState) -> Result<Value, String> {
    use super::tools::*;

    // Stamp activity so the GUI status LED can show live MCP traffic.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    state.last_activity_ms.store(now_ms, Ordering::Relaxed);

    let text = match name {
        "attach_status"       => attach::run(args, state)?,
        "get_entity_address"  => entities::get_entity_address(args, state)?,
        "search_entities"     => entities::search_entities(args, state)?,
        "dump_struct"         => memory::dump_struct(args, state)?,
        "probe_range"         => memory::probe_range(args, state)?,
        "scan_for_value"      => memory::scan_for_value(args, state)?,
        "follow_pointers"     => pointers::follow_pointers(args, state)?,
        "probe_pointer_chain" => pointers::probe_pointer_chain(args, state)?,
        "compare_entities"    => witness::compare_entities(args, state)?,
        "get_known_schema"    => schema::get_known_schema(args, state)?,
        "read_name_debug"     => name_debug::read_name_debug(args, state)?,
        "read_logs"           => logs::read_logs(state)?,
        "read_entity_fields"  => fields_read::read_entity_fields(args, state)?,
        other                 => return Err(format!("Unknown tool: {other}")),
    };

    Ok(json!({ "content": [{ "type": "text", "text": text }] }))
}

// ---------------------------------------------------------------------------
// Tool list — static JSON schema definitions for all 11 tools.
// ---------------------------------------------------------------------------

fn tool_list() -> Value {
    json!([
        {
            "name": "attach_status",
            "description": "Returns whether the MCP server is attached to fm.exe, the attached game version and PID, every running game, and basic entity counts. Call this first. With several games open it follows the game the editor is attached to; pass `pid` to switch to another one.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "pid": { "type": "integer", "description": "Attach to this fm.exe PID (see `running_games`). Omit to keep the current attachment." }
                },
                "required": []
            }
        },
        {
            "name": "get_entity_address",
            "description": "Returns the base memory address of an entity by array index OR database UID.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_type": { "type": "string", "enum": ["player", "staff", "club"] },
                    "index":       { "type": "integer", "description": "0-based array index" },
                    "uid":         { "type": "integer", "description": "Database UID (alternative to index)" }
                },
                "required": ["entity_type"]
            }
        },
        {
            "name": "search_entities",
            "description": "Search entities by name substring OR numeric UID. Returns up to 10 matches. First call builds a mini-cache (~10s for a full save). Pass rebuild_cache:true after loading a new save.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query":         { "type": "string", "description": "Name substring or numeric UID" },
                    "entity_type":   { "type": "string", "enum": ["player", "staff", "club"] },
                    "rebuild_cache": { "type": "boolean", "description": "Force a fresh cache rebuild (use after loading a new save)" }
                },
                "required": ["query", "entity_type"]
            }
        },
        {
            "name": "dump_struct",
            "description": "Hex-dump a memory region. Max 4096 bytes. Returns offset | hex | ASCII grid. Pass entity_type to append a known-field annotation table (struct_base defaults to address).",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address":     { "type": "integer", "description": "Start address (decimal or hex string)" },
                    "size":        { "type": "integer", "description": "Bytes to read (max 4096)" },
                    "entity_type": { "type": "string", "enum": ["player", "staff", "club"], "description": "If provided, annotates known field offsets after the hex dump" },
                    "struct_base": { "type": "integer", "description": "Entity struct base address for offset calculation (defaults to address)" }
                },
                "required": ["address", "size"]
            }
        },
        {
            "name": "probe_range",
            "description": "Sweep offsets start_offset..end_offset (step bytes each) and decode with encoding.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "base":         { "type": "integer" },
                    "start_offset": { "type": "integer" },
                    "end_offset":   { "type": "integer" },
                    "step":         { "type": "integer" },
                    "encoding":     { "type": "string", "description": "u8|u16le|u32le|u64le|i8|i16le|i32le|i64le|f32le|f64le|fm_string|bool_u8" }
                },
                "required": ["base", "start_offset", "end_offset", "step", "encoding"]
            }
        },
        {
            "name": "scan_for_value",
            "description": "Scan a struct for all offsets where the decoded value equals `value`. Primary triangulation tool.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "base":     { "type": "integer" },
                    "size":     { "type": "integer" },
                    "value":    { "type": "string", "description": "Target value as string (e.g. \"36\" for age=36)" },
                    "encoding": { "type": "string" }
                },
                "required": ["base", "size", "value", "encoding"]
            }
        },
        {
            "name": "follow_pointers",
            "description": "Find all valid pointer-sized values in a region and preview 64 bytes at each target. Discovers sub-record chains.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "base": { "type": "integer" },
                    "size": { "type": "integer", "description": "Region size (max 4096)" }
                },
                "required": ["base", "size"]
            }
        },
        {
            "name": "probe_pointer_chain",
            "description": "Follow a chain of pointer dereferences: base → deref(+offset[0]) → deref(+offset[1]) → ... Reaches deeply-nested sub-records in one call.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "base":    { "type": "integer" },
                    "offsets": { "type": "array", "items": { "type": "integer" }, "description": "Offsets to add at each hop" }
                },
                "required": ["base", "offsets"]
            }
        },
        {
            "name": "compare_entities",
            "description": "Read the same offset across multiple entity addresses. Primary cross-witness verification tool — confirms a candidate offset by checking it matches known values across 3+ entities.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "addresses": { "type": "array", "items": { "type": "integer" } },
                    "offset":    { "type": "integer" },
                    "size":      { "type": "integer" },
                    "encoding":  { "type": "string" }
                },
                "required": ["addresses", "offset", "size", "encoding"]
            }
        },
        {
            "name": "get_known_schema",
            "description": "Return all already-mapped fields for an entity type so Claude knows what's known and can build forward.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "entity_type": { "type": "string", "enum": ["player", "staff", "club"] }
                },
                "required": ["entity_type"]
            }
        },
        {
            "name": "read_name_debug",
            "description": "Diagnose name field resolution for a player or staff entity. Shows every name-related field: struct offset, whether it falls inside the struct read window, raw pointer value, validity, and resolved string. Use to diagnose blank names.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address":     { "type": "integer", "description": "Entity struct base address" },
                    "entity_type": { "type": "string", "enum": ["player", "staff"], "description": "Defaults to player" }
                },
                "required": ["address"]
            }
        },
        {
            "name": "read_logs",
            "description": "Drain and return the last 100 debug log entries (bridge errors, panics). Clears the buffer.",
            "inputSchema": { "type": "object", "properties": {}, "required": [] }
        },
        {
            "name": "read_entity_fields",
            "description": "Decode all mapped (or selected) fields for an entity at a given address. Returns a JSON array of { key, label, offset, raw_hex, value } entries sorted by offset. Use to programmatically verify field mappings without visual inspection.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "address":     { "type": "integer", "description": "Entity struct base address (decimal or hex string)" },
                    "entity_type": { "type": "string", "enum": ["player", "staff", "club"], "description": "Defaults to player" },
                    "fields":      { "type": "array", "items": { "type": "string" }, "description": "Optional list of field keys to read (e.g. [\"p_morale\",\"p_ca\"]). Omit to read all mapped fields." }
                },
                "required": ["address"]
            }
        }
    ])
}
