use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn send(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn read(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn ack(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
