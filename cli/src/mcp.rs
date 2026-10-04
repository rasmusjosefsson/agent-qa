//! `agent-qa mcp` — stdio MCP server exposing the CLI to agent clients.
//!
//! Newline-delimited JSON-RPC 2.0 on stdin/stdout, per the MCP stdio
//! transport. Every tool spawns this same binary with the matching verb and
//! returns stdout+stderr as text content — the server stays a thin shell so
//! verbs and their MCP surface can never drift apart.

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};

pub fn run(_args: &[String]) -> Result<u8> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if let Some(resp) = handle(&msg) {
            writeln!(out, "{}", serde_json::to_string(&resp)?)?;
            out.flush()?;
        }
    }
    Ok(0u8)
}

fn handle(msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned()?;
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": msg.pointer("/params/protocolVersion")
                .and_then(Value::as_str)
                .unwrap_or("2024-11-05"),
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "agent-qa", "version": env!("CARGO_PKG_VERSION") },
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call_tool(msg),
        "resources/list" => Ok(json!({ "resources": [] })),
        "prompts/list" => Ok(json!({ "prompts": [] })),
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

fn call_tool(msg: &Value) -> Result<Value, Value> {
    let name = msg
        .pointer("/params/name")
        .and_then(Value::as_str)
        .unwrap_or("");
    let args = msg
        .pointer("/params/arguments")
        .cloned()
        .unwrap_or(json!({}));
    let argv: Vec<String> = match name {
        "qa_info" => vec!["info".into(), "--json".into()],
        "scenario_list" => vec!["scenario".into(), "ls".into()],
        "scenario_check" => match args.get("path").and_then(Value::as_str) {
            Some(p) => vec!["scenario".into(), "check".into(), p.into()],
            None => vec!["scenario".into(), "check-all".into()],
        },
        "replay" => {
            let sid = args.get("sid").and_then(Value::as_str).unwrap_or("");
            if sid.is_empty() {
                return Err(tool_err("replay requires 'sid'"));
            }
            let mut v = vec!["replay".into(), sid.into()];
            if args.get("update_baselines").and_then(Value::as_bool) == Some(true) {
                v.push("--update-baselines".into());
            }
            v
        }
        "audit_summary" => {
            let sid = args.get("sid").and_then(Value::as_str).unwrap_or("");
            if sid.is_empty() {
                return Err(tool_err("audit_summary requires 'sid'"));
            }
            let run_id = args.get("run").and_then(Value::as_str).unwrap_or("latest");
            vec!["audit".into(), "summary".into(), sid.into(), run_id.into()]
        }
        "baselines_status" => vec![
            "baselines".into(),
            "status".into(),
            "--all".into(),
            "--json".into(),
        ],
        "baselines_pull" => vec!["baselines".into(), "pull".into(), "--all".into()],
        _ => return Err(tool_err(&format!("unknown tool: {name}"))),
    };
    Ok(exec_tool(&argv))
}

fn tool_err(message: &str) -> Value {
    json!({ "code": -32602, "message": message })
}

fn exec_tool(argv: &[String]) -> Value {
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return text_result(&format!("current_exe: {e}"), true),
    };
    let out = std::process::Command::new(exe).args(argv).output();
    match out {
        Ok(o) => {
            let mut text = String::from_utf8_lossy(&o.stdout).into_owned();
            let err = String::from_utf8_lossy(&o.stderr);
            if !err.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&err);
            }
            text_result(&text, !o.status.success())
        }
        Err(e) => text_result(&format!("spawn agent-qa {}: {e}", argv.join(" ")), true),
    }
}

fn text_result(text: &str, is_error: bool) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": is_error,
    })
}

fn tools() -> Value {
    json!([
        {
            "name": "qa_info",
            "description": "agent-qa version, resolved paths, scenario + profile counts.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        },
        {
            "name": "scenario_list",
            "description": "List every scenario id under the scenarios root.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        },
        {
            "name": "scenario_check",
            "description": "Validate + lint one scenario file (pass 'path') or every scenario under the root.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Path to a scenario.json; omit to check all." },
                },
                "additionalProperties": false,
            },
        },
        {
            "name": "replay",
            "description": "Replay a scenario end-to-end in a real browser. Returns the runner output; exit status decides isError.",
            "inputSchema": {
                "type": "object",
                "required": ["sid"],
                "properties": {
                    "sid": { "type": "string", "description": "Scenario id (or path to a scenario dir)." },
                    "update_baselines": { "type": "boolean", "description": "Re-mint shot/domshot baselines from this run." },
                },
                "additionalProperties": false,
            },
        },
        {
            "name": "audit_summary",
            "description": "One-line pass/fail summary of a scenario run (latest by default).",
            "inputSchema": {
                "type": "object",
                "required": ["sid"],
                "properties": {
                    "sid": { "type": "string" },
                    "run": { "type": "string", "description": "Run id, default 'latest'." },
                },
                "additionalProperties": false,
            },
        },
        {
            "name": "baselines_status",
            "description": "Golden-baseline store status for every scenario (local/github/turso).",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        },
        {
            "name": "baselines_pull",
            "description": "Pull goldens from the configured remote store for every scenario.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_echoes_client_version() {
        let resp = handle(&json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-03-26" },
        }))
        .unwrap();
        assert_eq!(resp["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(resp["result"]["serverInfo"]["name"], "agent-qa");
    }

    #[test]
    fn notifications_get_no_response() {
        assert!(
            handle(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none()
        );
    }

    #[test]
    fn tools_call_rejects_unknown_name() {
        let resp = handle(&json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "nope", "arguments": {} },
        }))
        .unwrap();
        assert!(resp.get("error").is_some());
    }

    #[test]
    fn tools_call_maps_replay_args() {
        let resp = handle(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "audit_summary", "arguments": {} },
        }))
        .unwrap();
        // missing sid → invalid-params error, not a dispatch
        assert_eq!(resp["error"]["code"], -32602);
    }
}
