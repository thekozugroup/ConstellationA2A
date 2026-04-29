use chrono::Utc;
use constellation_a2a::{
    JsonRpcResponse, Message, Part, Role, TaskGetResult, TaskState, TaskStatus,
};
use constellation_client::A2aClient;
use serde_json::json;
use wiremock::{matchers::method, Mock, MockServer, ResponseTemplate};

fn fake_result(id: &str) -> TaskGetResult {
    TaskGetResult {
        id: id.into(),
        status: TaskStatus {
            state: TaskState::Completed,
            timestamp: Utc::now(),
        },
        history: vec![Message {
            role: Role::Agent,
            parts: vec![Part::Text { text: "ok".into() }],
        }],
    }
}

#[tokio::test]
async fn send_task_round_trip() {
    let server = MockServer::start().await;
    let result = fake_result("t-1");
    let response: JsonRpcResponse<TaskGetResult> = JsonRpcResponse::ok(json!("1"), result);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .mount(&server)
        .await;
    let client = A2aClient::new();
    let msg = Message {
        role: Role::User,
        parts: vec![Part::Text { text: "hi".into() }],
    };
    let task = client
        .send_task(&server.uri(), "t-1", &msg)
        .await
        .expect("send");
    assert_eq!(task.id, "t-1");
    assert_eq!(task.status.state, TaskState::Completed);
}

#[tokio::test]
async fn get_task_round_trip() {
    let server = MockServer::start().await;
    let result = fake_result("t-2");
    let response: JsonRpcResponse<TaskGetResult> = JsonRpcResponse::ok(json!("1"), result);
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .mount(&server)
        .await;
    let client = A2aClient::new();
    let task = client.get_task(&server.uri(), "t-2").await.expect("get");
    assert_eq!(task.id, "t-2");
}
