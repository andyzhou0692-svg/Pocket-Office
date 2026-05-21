use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

use crate::source::decoder::{decode_jsonl_line, SOURCE_NAME};
use crate::source::AgentEvent;
use crate::AgentId;

pub struct JsonlWatcher {
    root: PathBuf,
}

impl JsonlWatcher {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub async fn run(self, tx: mpsc::Sender<AgentEvent>) -> Result<()> {
        let cursors: Arc<Mutex<HashMap<PathBuf, u64>>> = Arc::new(Mutex::new(HashMap::new()));
        let seen_sessions: Arc<Mutex<HashMap<PathBuf, bool>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let (notify_tx, mut notify_rx) =
            tokio::sync::mpsc::unbounded_channel::<PathBuf>();
        let mut watcher: RecommendedWatcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                            let _ = notify_tx.send(path);
                        }
                    }
                }
            })?;
        let _ = tokio::fs::create_dir_all(&self.root).await;
        watcher.watch(&self.root, RecursiveMode::Recursive)?;

        if let Ok(read) = std::fs::read_dir(&self.root) {
            for entry in read.flatten() {
                walk_jsonl(&entry.path(), &cursors, &seen_sessions, &tx).await;
            }
        }

        loop {
            tokio::select! {
                Some(path) = notify_rx.recv() => {
                    walk_jsonl(&path, &cursors, &seen_sessions, &tx).await;
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    if let Ok(read) = std::fs::read_dir(&self.root) {
                        for entry in read.flatten() {
                            walk_jsonl(&entry.path(), &cursors, &seen_sessions, &tx).await;
                        }
                    }
                }
            }
        }
    }
}

async fn walk_jsonl(
    path: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, u64>>>,
    seen: &Arc<Mutex<HashMap<PathBuf, bool>>>,
    tx: &mpsc::Sender<AgentEvent>,
) {
    if path.is_dir() {
        if let Ok(read) = std::fs::read_dir(path) {
            for entry in read.flatten() {
                Box::pin(walk_jsonl(&entry.path(), cursors, seen, tx)).await;
            }
        }
        return;
    }
    if path.extension().and_then(|s| s.to_str()) != Some("jsonl") {
        return;
    }

    let bytes = match tokio::fs::read(path).await {
        Ok(b) => b,
        Err(e) => {
            warn!("read {} failed: {e}", path.display());
            return;
        }
    };

    let mut cursors_g = cursors.lock().await;
    let cursor = cursors_g.entry(path.to_path_buf()).or_insert(0);
    if (*cursor as usize) >= bytes.len() {
        return;
    }
    let cursor_now = *cursor as usize;
    *cursor = bytes.len() as u64;
    drop(cursors_g);

    let new_bytes = &bytes[cursor_now..];
    let transcript_path_str = path.to_string_lossy().into_owned();

    // Emit SessionStart on first sight of this transcript.
    {
        let mut seen = seen.lock().await;
        if seen.insert(path.to_path_buf(), true).is_none() {
            let id = AgentId::from_transcript_path(&transcript_path_str);
            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let _ = tx
                .send(AgentEvent::SessionStart {
                    agent_id: id,
                    source: SOURCE_NAME.into(),
                    session_id,
                    cwd: PathBuf::new(),
                })
                .await;
        }
    }

    for line in new_bytes.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let s = match std::str::from_utf8(line) {
            Ok(s) => s,
            Err(_) => {
                warn!("non-utf8 line in {}", path.display());
                continue;
            }
        };
        let v: serde_json::Value = match serde_json::from_str(s) {
            Ok(v) => v,
            Err(e) => {
                debug!("skip non-json line in {}: {e}", path.display());
                continue;
            }
        };
        match decode_jsonl_line(&transcript_path_str, v) {
            Ok(events) => {
                for ev in events {
                    if tx.send(ev).await.is_err() {
                        return;
                    }
                }
            }
            Err(e) => warn!("decode error in {}: {e}", path.display()),
        }
    }
}
