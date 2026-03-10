use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn create(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn update(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn list(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
