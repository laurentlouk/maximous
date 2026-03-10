use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn poll(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
