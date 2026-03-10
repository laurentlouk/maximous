use serde::{Deserialize, Serialize};
use serde_json::Value;

// --- JSON-RPC Types ---

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[allow(dead_code)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

impl JsonRpcResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(id),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

// --- MCP Tool Definition ---

#[derive(Debug, Serialize)]
pub struct ToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

pub fn tool_definitions() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "memory_set".into(),
            description: "Store a value in shared memory with optional TTL".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key within namespace"},
                    "value": {"type": "string", "description": "JSON string value to store"},
                    "ttl_seconds": {"type": "integer", "description": "Optional TTL in seconds"}
                },
                "required": ["namespace", "key", "value"]
            }),
        },
        ToolDef {
            name: "memory_get".into(),
            description: "Read a value from shared memory by namespace and key, or list all keys in a namespace".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key within namespace (omit to list all keys)"}
                },
                "required": ["namespace"]
            }),
        },
        ToolDef {
            name: "memory_search".into(),
            description: "Full-text search across memory values".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query (matches against values)"},
                    "namespace": {"type": "string", "description": "Optional namespace filter"}
                },
                "required": ["query"]
            }),
        },
        ToolDef {
            name: "memory_delete".into(),
            description: "Delete a key from memory or expire all stale entries".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "namespace": {"type": "string", "description": "Memory namespace"},
                    "key": {"type": "string", "description": "Key to delete (omit to expire all stale entries in namespace)"}
                },
                "required": ["namespace"]
            }),
        },
        ToolDef {
            name: "message_send".into(),
            description: "Send a message to a channel with priority".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Target channel"},
                    "sender": {"type": "string", "description": "Sender identifier"},
                    "content": {"type": "string", "description": "Message content (JSON string)"},
                    "priority": {"type": "integer", "description": "0=critical, 1=high, 2=normal, 3=low", "default": 2}
                },
                "required": ["channel", "sender", "content"]
            }),
        },
        ToolDef {
            name: "message_read".into(),
            description: "Read messages from a channel, optionally only unacknowledged".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "channel": {"type": "string", "description": "Channel to read from"},
                    "unacknowledged_only": {"type": "boolean", "description": "Only return unacknowledged messages", "default": false},
                    "limit": {"type": "integer", "description": "Max messages to return", "default": 50}
                },
                "required": ["channel"]
            }),
        },
        ToolDef {
            name: "message_ack".into(),
            description: "Acknowledge a message by ID".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "Message ID to acknowledge"}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "task_create".into(),
            description: "Create a new task with optional dependencies".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string", "description": "Task title"},
                    "priority": {"type": "integer", "description": "0=critical, 1=high, 2=normal, 3=low", "default": 2},
                    "dependencies": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Array of task IDs this task depends on"
                    }
                },
                "required": ["title"]
            }),
        },
        ToolDef {
            name: "task_update".into(),
            description: "Update a task's status, assignment, or result".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Task ID"},
                    "status": {"type": "string", "enum": ["pending", "ready", "running", "done", "failed"]},
                    "assigned_to": {"type": "string", "description": "Agent ID to assign to"},
                    "result": {"type": "string", "description": "Task result (JSON string)"}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "task_list".into(),
            description: "List tasks filtered by status or assignee".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {"type": "string", "description": "Filter by status"},
                    "assigned_to": {"type": "string", "description": "Filter by assigned agent"}
                }
            }),
        },
        ToolDef {
            name: "agent_register".into(),
            description: "Register an agent with capabilities".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Agent ID"},
                    "name": {"type": "string", "description": "Agent display name"},
                    "capabilities": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "List of capabilities"
                    },
                    "metadata": {"type": "string", "description": "Optional JSON metadata"}
                },
                "required": ["id", "name"]
            }),
        },
        ToolDef {
            name: "agent_heartbeat".into(),
            description: "Update agent heartbeat and optionally status".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Agent ID"},
                    "status": {"type": "string", "enum": ["idle", "active", "stopped"]}
                },
                "required": ["id"]
            }),
        },
        ToolDef {
            name: "agent_list".into(),
            description: "List active agents (heartbeat within last 60 seconds)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_stale": {"type": "boolean", "description": "Include agents with old heartbeats", "default": false}
                }
            }),
        },
        ToolDef {
            name: "poll_changes".into(),
            description: "Get all changes since a given change ID (observation pattern)".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "since_id": {"type": "integer", "description": "Return changes with ID greater than this", "default": 0},
                    "table_name": {"type": "string", "description": "Optional filter by table name"},
                    "limit": {"type": "integer", "description": "Max changes to return", "default": 100}
                }
            }),
        },
    ]
}

// --- MCP Protocol Handlers ---

pub fn handle_initialize(id: Value) -> JsonRpcResponse {
    JsonRpcResponse::success(id, serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "maximous",
            "version": env!("CARGO_PKG_VERSION")
        }
    }))
}

pub fn handle_tools_list(id: Value) -> JsonRpcResponse {
    let tools = tool_definitions();
    JsonRpcResponse::success(id, serde_json::json!({ "tools": tools }))
}

// --- Stdio Loop ---

use rusqlite::Connection;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

pub fn run_stdio(conn: Arc<Mutex<Connection>>) {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse::error(
                    serde_json::json!(null),
                    -32700,
                    &format!("Parse error: {}", e),
                );
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        let response = dispatch(&req, &conn);

        if let Some(resp) = response {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap());
            let _ = stdout.flush();
        }
    }
}

fn dispatch(req: &JsonRpcRequest, conn: &Arc<Mutex<Connection>>) -> Option<JsonRpcResponse> {
    let id = req.id.clone();

    match req.method.as_str() {
        "initialize" => Some(handle_initialize(id?)),
        "notifications/initialized" => None,
        "ping" => Some(JsonRpcResponse::success(id?, serde_json::json!({}))),
        "tools/list" => Some(handle_tools_list(id?)),
        "tools/call" => {
            let params = req.params.as_ref();
            let tool_name = params
                .and_then(|p| p.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            let arguments = params
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or(serde_json::json!({}));

            let conn = conn.lock().unwrap();
            let result = crate::tools::dispatch_tool(tool_name, &arguments, &conn);

            Some(JsonRpcResponse::success(
                id?,
                serde_json::json!({
                    "content": [{
                        "type": "text",
                        "text": serde_json::to_string(&result).unwrap()
                    }]
                }),
            ))
        }
        _ => Some(JsonRpcResponse::error(
            id.unwrap_or(serde_json::json!(null)),
            -32601,
            &format!("Method not found: {}", req.method),
        )),
    }
}
