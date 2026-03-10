use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn register(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn heartbeat(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn list(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
