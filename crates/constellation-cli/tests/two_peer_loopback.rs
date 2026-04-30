//! Spawn two `constellation` processes on loopback. A sends a task to B; B's
//! shell answers it via `constellation respond`; A's `wait` returns the answer.

use std::{
    fs,
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::Duration,
};
use tempfile::tempdir;

fn write_config(dir: &std::path::Path, name: &str, port: u16) -> PathBuf {
    let path = dir.join("config.toml");
    fs::write(
        &path,
        format!(
            r#"
[agent]
name = "{name}"
skills = ["test"]

[network]
bind = "127.0.0.1:{port}"
advertised_host = "127.0.0.1"
discovery = []

[store]
path = "{}"
"#,
            dir.join("store.db").display()
        ),
    )
    .unwrap();
    path
}

fn spawn_serve(exe: &str, config: &PathBuf) -> Child {
    Command::new(exe)
        .arg("--config")
        .arg(config)
        .arg("serve")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn serve")
}

fn cli(exe: &str, config: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(exe)
        .arg("--config")
        .arg(config)
        .args(args)
        .output()
        .expect("cli output")
}

fn insert_peer(dir: &std::path::Path, name: &str, url: &str) {
    use rusqlite::Connection;
    let db = dir.join("store.db");
    let conn = Connection::open(&db).expect("open db");
    let card = serde_json::json!({
        "name": name,
        "url": url,
        "version": "0.1.0",
        "capabilities": {
            "streaming": false,
            "pushNotifications": false,
            "stateTransitionHistory": false
        },
        "defaultInputModes": ["text"],
        "defaultOutputModes": ["text"],
        "skills": [{"id": "test", "name": "test", "tags": ["test"]}]
    });
    let card_json = card.to_string();
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT OR REPLACE INTO peers(id, name, url, card_json, last_seen) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![url, name, url, card_json, now],
    )
    .expect("upsert peer");
}

#[test]
fn two_peer_round_trip() {
    let exe = env!("CARGO_BIN_EXE_constellation");
    let a_dir = tempdir().unwrap();
    let b_dir = tempdir().unwrap();
    let a_cfg = write_config(a_dir.path(), "alice", 47771);
    let b_cfg = write_config(b_dir.path(), "bob", 47772);

    let mut a = spawn_serve(exe, &a_cfg);
    let mut b = spawn_serve(exe, &b_cfg);

    // Wait until both servers respond on /.well-known/agent.json
    for _ in 0..30 {
        let a_up = std::net::TcpStream::connect("127.0.0.1:47771").is_ok();
        let b_up = std::net::TcpStream::connect("127.0.0.1:47772").is_ok();
        if a_up && b_up {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    let alice_url = "http://127.0.0.1:47771";
    let bob_url = "http://127.0.0.1:47772";
    insert_peer(a_dir.path(), "bob", bob_url);
    insert_peer(b_dir.path(), "alice", alice_url);

    let send_out = cli(exe, &a_cfg, &["send", "bob", "say hi"]);
    assert!(
        send_out.status.success(),
        "send failed: stdout={:?} stderr={:?}",
        String::from_utf8_lossy(&send_out.stdout),
        String::from_utf8_lossy(&send_out.stderr)
    );
    let task_id = String::from_utf8_lossy(&send_out.stdout).trim().to_string();

    let inbox = cli(exe, &b_cfg, &["inbox"]);
    assert!(inbox.status.success());
    let inbox_text = String::from_utf8_lossy(&inbox.stdout);
    assert!(
        inbox_text.contains(&task_id),
        "bob inbox missing task {task_id}: {inbox_text}"
    );

    let respond = cli(exe, &b_cfg, &["respond", &task_id, "hi alice"]);
    assert!(
        respond.status.success(),
        "respond failed: stderr={}",
        String::from_utf8_lossy(&respond.stderr)
    );

    let wait = cli(exe, &a_cfg, &["wait", &task_id, "--timeout", "10"]);
    assert!(
        wait.status.success(),
        "wait failed: stderr={}",
        String::from_utf8_lossy(&wait.stderr)
    );
    let body = String::from_utf8_lossy(&wait.stdout);
    assert!(body.contains("hi alice"), "expected reply; got: {body}");

    let _ = a.kill();
    let _ = a.wait();
    let _ = b.kill();
    let _ = b.wait();
}
