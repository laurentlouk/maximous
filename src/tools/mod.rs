pub mod memory;
pub mod messages;
pub mod tasks;
pub mod agents;
pub mod changes;
pub mod sessions;

use rusqlite::Connection;
use serde_json::Value;

#[derive(Debug, serde::Serialize)]
pub struct ToolResult {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ToolResult {
    pub fn success(data: Value) -> Self {
        Self { ok: true, data: Some(data), error: None }
    }

    pub fn fail(msg: &str) -> Self {
        Self { ok: false, data: None, error: Some(msg.to_string()) }
    }
}

pub fn dispatch_tool(name: &str, args: &Value, conn: &Connection) -> ToolResult {
    match name {
        "memory_set" => memory::set(args, conn),
        "memory_get" => memory::get(args, conn),
        "memory_search" => memory::search(args, conn),
        "memory_search_index" => memory::search_index(args, conn),
        "memory_delete" => memory::delete(args, conn),
        "message_send" => messages::send(args, conn),
        "message_read" => messages::read(args, conn),
        "message_ack" => messages::ack(args, conn),
        "task_create" => tasks::create(args, conn),
        "task_update" => tasks::update(args, conn),
        "task_list" => tasks::list(args, conn),
        "agent_register" => agents::register(args, conn),
        "agent_heartbeat" => agents::heartbeat(args, conn),
        "agent_list" => agents::list(args, conn),
        "poll_changes" => changes::poll(args, conn),
        "session_start" => sessions::start(args, conn),
        "session_end" => sessions::end(args, conn),
        "session_list" => sessions::list(args, conn),
        _ => ToolResult::fail(&format!("Unknown tool: {}", name)),
    }
}
