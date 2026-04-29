use constellation_a2a::{AgentCard, JsonRpcRequest, JsonRpcResponse, TaskGetResult};

#[test]
fn agent_card_round_trip() {
    let raw = include_str!("fixtures/agent_card.json");
    let card: AgentCard = serde_json::from_str(raw).expect("deserialize agent card");
    let again = serde_json::to_string(&card).expect("serialize");
    let reparsed: AgentCard = serde_json::from_str(&again).expect("re-parse");
    assert_eq!(card, reparsed);
    assert_eq!(card.name, "atmos-vnic");
    assert_eq!(card.skills.len(), 1);
    assert_eq!(card.skills[0].id, "research");
}

#[test]
fn tasks_send_request_round_trip() {
    let raw = include_str!("fixtures/tasks_send_request.json");
    let req: JsonRpcRequest = serde_json::from_str(raw).expect("deserialize request");
    assert_eq!(req.method, "tasks/send");
    let again = serde_json::to_string(&req).expect("serialize");
    let reparsed: JsonRpcRequest = serde_json::from_str(&again).expect("re-parse");
    assert_eq!(req, reparsed);
}

#[test]
fn tasks_get_response_round_trip() {
    let raw = include_str!("fixtures/tasks_get_response.json");
    let resp: JsonRpcResponse<TaskGetResult> =
        serde_json::from_str(raw).expect("deserialize response");
    assert!(resp.error.is_none());
    let result = resp.result.expect("result");
    assert_eq!(result.id, "task-abc");
    assert_eq!(result.status.state.as_str(), "completed");
    assert_eq!(result.history.len(), 2);
}

#[test]
fn rejects_wrong_jsonrpc_version() {
    let raw = r#"{"jsonrpc":"1.0","id":"1","method":"x","params":{}}"#;
    let res: Result<JsonRpcRequest, _> = serde_json::from_str(raw);
    assert!(res.is_err());
}
