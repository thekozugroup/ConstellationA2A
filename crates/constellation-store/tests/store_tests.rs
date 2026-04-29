use chrono::Utc;
use constellation_a2a::{AgentCapabilities, AgentCard, Message, Part, Role, Skill, TaskState};
use constellation_store::{peers, tasks_in, tasks_out, Store};
use tempfile::tempdir;
use url::Url;

fn sample_card() -> AgentCard {
    AgentCard {
        name: "test-peer".into(),
        description: Some("test".into()),
        url: Url::parse("http://10.0.0.5:7777").unwrap(),
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

#[test]
fn store_initializes_schema() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    assert!(peers::list_peers(&store).unwrap().is_empty());
}

#[test]
fn peer_upsert_round_trip() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    peers::upsert_peer(&store, &sample_card(), Utc::now()).unwrap();
    let list = peers::list_peers(&store).unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].card.name, "test-peer");
}

#[test]
fn inbound_task_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    let msg = Message {
        role: Role::User,
        parts: vec![Part::Text { text: "hi".into() }],
    };
    tasks_in::insert(&store, "t1", "peer-a", &msg).unwrap();
    let pending = tasks_in::list_pending(&store).unwrap();
    assert_eq!(pending.len(), 1);
    let response = Message {
        role: Role::Agent,
        parts: vec![Part::Text { text: "ok".into() }],
    };
    tasks_in::set_response(&store, "t1", &response, TaskState::Completed).unwrap();
    let got = tasks_in::get(&store, "t1").unwrap().unwrap();
    assert_eq!(got.state, TaskState::Completed);
    assert!(got.updated_at <= chrono::Utc::now());
    assert!(tasks_in::list_pending(&store).unwrap().is_empty());
}

#[test]
fn outbound_task_lifecycle() {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path().join("store.db")).unwrap();
    let msg = Message {
        role: Role::User,
        parts: vec![Part::Text { text: "go".into() }],
    };
    tasks_out::insert(&store, "t9", "peer-b", &msg).unwrap();
    let response = Message {
        role: Role::Agent,
        parts: vec![Part::Text {
            text: "done".into(),
        }],
    };
    tasks_out::set_response(&store, "t9", &response, TaskState::Completed).unwrap();
    let got = tasks_out::get(&store, "t9").unwrap().unwrap();
    assert_eq!(got.state, TaskState::Completed);
    assert!(got.updated_at <= chrono::Utc::now());
}
