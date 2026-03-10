use rusqlite::Connection;
use serde_json::Value;
use super::ToolResult;

pub fn set(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn get(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn search(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
pub fn delete(_args: &Value, _conn: &Connection) -> ToolResult { ToolResult::fail("not implemented") }
