use constellation_a2a::{
    AgentCapabilities, AgentCard, JsonRpcError, JsonRpcResponse, Skill, TaskGetResult, TaskState,
};
use constellation_server::{build_app, AppState};
use constellation_store::Store;
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tempfile::tempdir;
use tokio::net::TcpListener;
use url::Url;

fn card() -> AgentCard {
    AgentCard {
        name: "self".into(),
        description: None,
        url: Url::parse("http://127.0.0.1:0").unwrap(),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities::default(),
        default_input_modes: vec!["text".into()],
        default_output_modes: vec!["text".into()],
        skills: vec![Skill {
            id: "x".into(),
            name: "x".into(),
            description: None,
            tags: vec![],
        }],
    }
}

#[tokio::test]
async fn tasks_send_persists_inbound() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState {
        store: store.clone(),
        card: card(),
    };

    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = json!({
        "jsonrpc":"2.0","id":"1","method":"tasks/send",
        "params":{"id":"t1","message":{"role":"user","parts":[{"type":"text","text":"hi"}]}}
    });
    let resp: JsonRpcResponse<TaskGetResult> = reqwest::Client::new()
        .post(format!("http://{addr}"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(resp.error.is_none(), "{:?}", resp.error);
    assert_eq!(resp.result.unwrap().status.state, TaskState::Submitted);

    let stored = constellation_store::tasks_in::get(&store, "t1")
        .unwrap()
        .unwrap();
    assert_eq!(stored.from_peer, "unknown");
    let part = stored
        .request
        .parts
        .first()
        .expect("request had a text part");
    match part {
        constellation_a2a::Part::Text { text } => assert_eq!(text, "hi"),
    }
}

#[tokio::test]
async fn agent_card_endpoint_returns_card() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState {
        store,
        card: card(),
    };
    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let resp: AgentCard = reqwest::get(format!("http://{addr}/.well-known/agent.json"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(resp.name, "self");
}

#[tokio::test]
async fn tasks_send_records_caller_url_from_header() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState {
        store: store.clone(),
        card: card(),
    };
    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = json!({
        "jsonrpc":"2.0","id":"1","method":"tasks/send",
        "params":{"id":"t-hdr","message":{"role":"user","parts":[{"type":"text","text":"hello"}]}}
    });
    let _ = reqwest::Client::new()
        .post(format!("http://{addr}"))
        .header("X-A2A-Source-Url", "http://10.0.0.42:7777")
        .json(&body)
        .send()
        .await
        .unwrap();

    let stored = constellation_store::tasks_in::get(&store, "t-hdr")
        .unwrap()
        .unwrap();
    assert_eq!(stored.from_peer, "http://10.0.0.42:7777");
}

#[tokio::test]
async fn tasks_get_returns_task_not_found_for_unknown_id() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState {
        store,
        card: card(),
    };
    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = json!({
        "jsonrpc":"2.0","id":"1","method":"tasks/get",
        "params":{"id":"does-not-exist"}
    });
    let resp: JsonRpcResponse<TaskGetResult> = reqwest::Client::new()
        .post(format!("http://{addr}"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(resp.result.is_none());
    let err: JsonRpcError = resp.error.expect("error");
    assert_eq!(err.code, -32001);
}

#[tokio::test]
async fn tasks_cancel_returns_not_implemented() {
    let dir = tempdir().unwrap();
    let store = Arc::new(Store::open(dir.path().join("s.db")).unwrap());
    let state = AppState {
        store,
        card: card(),
    };
    let app = build_app(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let body = json!({
        "jsonrpc":"2.0","id":"1","method":"tasks/cancel",
        "params":{"id":"x"}
    });
    let resp: JsonRpcResponse<TaskGetResult> = reqwest::Client::new()
        .post(format!("http://{addr}"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let err = resp.error.expect("error");
    assert_eq!(err.code, -32004);
}
