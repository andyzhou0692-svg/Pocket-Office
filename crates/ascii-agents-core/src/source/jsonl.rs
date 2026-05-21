use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Mutex;
use tracing::{debug, warn};

use crate::source::decoder::{decode_jsonl_line, SOURCE_NAME};
use crate::source::{AgentEvent, TaggedSender};
use crate::state::reducer::Transport;
use crate::AgentId;

pub struct JsonlWatcher {
    root: PathBuf,
}

impl JsonlWatcher {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub async fn run(self, tx: TaggedSender) -> Result<()> {
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

        scan_root(&self.root, &cursors, &seen_sessions, &tx).await;

        loop {
            tokio::select! {
                Some(path) = notify_rx.recv() => {
                    walk_jsonl(&path, &cursors, &seen_sessions, &tx).await;
                }
                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                    scan_root(&self.root, &cursors, &seen_sessions, &tx).await;
                }
            }
        }
    }
}

async fn scan_root(
    root: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, u64>>>,
    seen: &Arc<Mutex<HashMap<PathBuf, bool>>>,
    tx: &TaggedSender,
) {
    if let Ok(mut read) = tokio::fs::read_dir(root).await {
        while let Ok(Some(entry)) = read.next_entry().await {
            walk_jsonl(&entry.path(), cursors, seen, tx).await;
        }
    }
}

async fn walk_jsonl(
    path: &Path,
    cursors: &Arc<Mutex<HashMap<PathBuf, u64>>>,
    seen: &Arc<Mutex<HashMap<PathBuf, bool>>>,
    tx: &TaggedSender,
) {
    let meta = match tokio::fs::metadata(path).await {
        Ok(m) => m,
        Err(_) => return,
    };
    if meta.is_dir() {
        if let Ok(mut read) = tokio::fs::read_dir(path).await {
            while let Ok(Some(entry)) = read.next_entry().await {
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

    // Only consume up to the last complete (newline-terminated) line; a partial
    // tail stays buffered until the next notify event completes it.
    let safe_end = match bytes.iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None => 0,
    };

    let cursor_now;
    {
        let mut cursors_g = cursors.lock().await;
        let cursor = cursors_g.entry(path.to_path_buf()).or_insert(0);
        cursor_now = *cursor as usize;
        if cursor_now >= safe_end {
            return;
        }
        *cursor = safe_end as u64;
    }

    let new_bytes = &bytes[cursor_now..safe_end];
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
            // Try to extract cwd from the first parseable line; harmless if it fails.
            let cwd = extract_cwd(&bytes[..safe_end]).unwrap_or_default();
            let _ = tx
                .send((
                    Transport::Jsonl,
                    AgentEvent::SessionStart {
                        agent_id: id,
                        source: SOURCE_NAME.into(),
                        session_id,
                        cwd,
                    },
                ))
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
                    if tx.send((Transport::Jsonl, ev)).await.is_err() {
                        return;
                    }
                }
            }
            Err(e) => warn!("decode error in {}: {e}", path.display()),
        }
    }
}

fn extract_cwd(bytes: &[u8]) -> Option<PathBuf> {
    for line in bytes.split(|b| *b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let s = std::str::from_utf8(line).ok()?;
        let v: serde_json::Value = serde_json::from_str(s).ok()?;
        if let Some(cwd) = v.get("cwd").and_then(|c| c.as_str()) {
            return Some(PathBuf::from(cwd));
        }
    }
    None
}
