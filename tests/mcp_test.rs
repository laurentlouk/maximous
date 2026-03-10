use maximous::mcp;

#[test]
fn test_parse_initialize_request() {
    let input = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}}}"#;
    let req: mcp::JsonRpcRequest = serde_json::from_str(input).unwrap();
    assert_eq!(req.method, "initialize");
    assert_eq!(req.id, Some(serde_json::json!(1)));
}

#[test]
fn test_serialize_success_response() {
    let resp = mcp::JsonRpcResponse::success(serde_json::json!(1), serde_json::json!({"ok": true}));
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"result\""));
    assert!(!json.contains("\"error\""));
}

#[test]
fn test_serialize_error_response() {
    let resp = mcp::JsonRpcResponse::error(serde_json::json!(1), -32601, "Method not found");
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("\"error\""));
    assert!(json.contains("Method not found"));
}

#[test]
fn test_tool_list_contains_all_tools() {
    let tools = mcp::tool_definitions();
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    assert!(names.contains(&"memory_set"));
    assert!(names.contains(&"memory_get"));
    assert!(names.contains(&"memory_search"));
    assert!(names.contains(&"memory_delete"));
    assert!(names.contains(&"message_send"));
    assert!(names.contains(&"message_read"));
    assert!(names.contains(&"message_ack"));
    assert!(names.contains(&"task_create"));
    assert!(names.contains(&"task_update"));
    assert!(names.contains(&"task_list"));
    assert!(names.contains(&"agent_register"));
    assert!(names.contains(&"agent_heartbeat"));
    assert!(names.contains(&"agent_list"));
    assert!(names.contains(&"poll_changes"));
    assert!(names.contains(&"memory_search_index"));
    assert!(names.contains(&"session_start"));
    assert!(names.contains(&"session_end"));
    assert!(names.contains(&"session_list"));
    assert_eq!(tools.len(), 18);
}
