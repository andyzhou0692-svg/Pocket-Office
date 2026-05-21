use std::time::Duration;

use tempfile::TempDir;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

use ascii_agents_core::source::jsonl::JsonlWatcher;
use ascii_agents_core::source::AgentEvent;

#[tokio::test]
async fn watcher_emits_session_start_then_activity_for_tool_use() {
    let dir = TempDir::new().unwrap();
    let projects_root = dir.path().to_path_buf();
    let project_dir = projects_root.join("proj-x");
    tokio::fs::create_dir_all(&project_dir).await.unwrap();
    let transcript = project_dir.join("ses-abc.jsonl");

    let (tx, mut rx) = mpsc::channel::<AgentEvent>(32);
    let watcher = JsonlWatcher::new(projects_root.clone());
    let handle = tokio::spawn(async move { watcher.run(tx).await });

    tokio::time::sleep(Duration::from_millis(50)).await;

    let mut f = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&transcript)
        .await
        .unwrap();
    let start_line = serde_json::json!({
        "type": "system",
        "subtype": "session_start",
        "sessionId": "ses-abc",
        "cwd": "/repo"
    });
    f.write_all(format!("{start_line}\n").as_bytes())
        .await
        .unwrap();
    let assistant_line = serde_json::json!({
        "type": "assistant",
        "sessionId": "ses-abc",
        "cwd": "/repo",
        "message": {
            "role": "assistant",
            "content": [
                { "type": "tool_use", "id": "tu_1", "name": "Bash",
                  "input": { "command": "ls" } }
            ]
        }
    });
    f.write_all(format!("{assistant_line}\n").as_bytes())
        .await
        .unwrap();
    f.flush().await.unwrap();
    drop(f);

    let mut got_start = false;
    let mut got_activity = false;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
            Ok(Some(AgentEvent::SessionStart { .. })) => got_start = true,
            Ok(Some(AgentEvent::ActivityStart { .. })) => got_activity = true,
            Ok(Some(_)) => {}
            Ok(None) | Err(_) => {}
        }
        if got_start && got_activity {
            break;
        }
    }
    assert!(got_start, "expected SessionStart from JSONL watcher");
    assert!(got_activity, "expected ActivityStart from JSONL watcher");
    handle.abort();
}
