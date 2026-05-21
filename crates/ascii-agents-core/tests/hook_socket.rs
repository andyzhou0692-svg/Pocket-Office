use std::time::Duration;

use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tokio::sync::mpsc;

use ascii_agents_core::source::hook::HookSocketListener;
use ascii_agents_core::source::AgentEvent;
use ascii_agents_core::state::reducer::Transport;

#[tokio::test]
async fn listener_parses_line_and_emits_event() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ascii-agents.sock");

    let (tx, mut rx) = mpsc::channel::<(Transport, AgentEvent)>(16);
    let listener = HookSocketListener::bind(path.clone()).await.unwrap();
    let handle = tokio::spawn(async move { listener.run(tx).await });

    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut s = UnixStream::connect(&path).await.unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "SessionStart",
        "session_id": "ses-1",
        "transcript_path": "/p/a.jsonl",
        "cwd": "/repo"
    });
    let mut line = serde_json::to_vec(&payload).unwrap();
    line.push(b'\n');
    s.write_all(&line).await.unwrap();
    s.shutdown().await.unwrap();

    let (transport, ev) = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transport, Transport::Hook);
    assert!(matches!(ev, AgentEvent::SessionStart { .. }));

    handle.abort();
}

#[tokio::test]
async fn listener_skips_malformed_line_and_keeps_going() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("ascii-agents.sock");
    let (tx, mut rx) = mpsc::channel::<(Transport, AgentEvent)>(16);
    let listener = HookSocketListener::bind(path.clone()).await.unwrap();
    let handle = tokio::spawn(async move { listener.run(tx).await });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let mut s = UnixStream::connect(&path).await.unwrap();
    s.write_all(b"not json\n").await.unwrap();
    let payload = serde_json::json!({
        "hook_event_name": "SessionEnd",
        "session_id": "ses-1",
        "transcript_path": "/p/a.jsonl",
        "cwd": "/repo",
        "reason": "exit"
    });
    let mut line = serde_json::to_vec(&payload).unwrap();
    line.push(b'\n');
    s.write_all(&line).await.unwrap();
    s.shutdown().await.unwrap();

    let (transport, ev) = tokio::time::timeout(Duration::from_millis(500), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(transport, Transport::Hook);
    assert!(matches!(ev, AgentEvent::SessionEnd { .. }));
    handle.abort();
}
