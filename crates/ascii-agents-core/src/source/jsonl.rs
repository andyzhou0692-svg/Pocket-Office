use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};
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

    // Streaming read: stat the file, take the cursor lock to learn how far
    // we've already consumed, then seek and read ONLY the new bytes. Avoids
    // re-reading megabytes for every notify event on a large transcript.
    let file_len = match tokio::fs::metadata(path).await {
        Ok(m) => m.len(),
        Err(e) => {
            warn!("stat {} failed: {e}", path.display());
            return;
        }
    };

    // Cap on how many unprocessed bytes we'll tolerate without a newline.
    // Protects against an attacker (or buggy writer) emitting a giant single
    // line — without this cap, every notify event re-reads the entire pending
    // tail, growing without bound.
    const MAX_PENDING_BYTES: u64 = 1 << 20; // 1 MiB

    let cursor_now: u64 = {
        let cursors_g = cursors.lock().await;
        *cursors_g.get(path).unwrap_or(&0)
    };
    if cursor_now > file_len {
        // File shrank (truncation / rotation). Reset cursor; treat as fresh.
        warn!(
            "{} truncated below cursor ({} < {}), resetting cursor",
            path.display(),
            file_len,
            cursor_now
        );
        cursors.lock().await.insert(path.to_path_buf(), 0);
        return;
    }
    if cursor_now == file_len {
        return; // nothing new
    }
    if file_len - cursor_now > MAX_PENDING_BYTES {
        warn!(
            "{} has > {} pending bytes with no newline; skipping to end",
            path.display(),
            MAX_PENDING_BYTES
        );
        cursors
            .lock()
            .await
            .insert(path.to_path_buf(), file_len);
        return;
    }

    let mut file = match tokio::fs::File::open(path).await {
        Ok(f) => f,
        Err(e) => {
            warn!("open {} failed: {e}", path.display());
            return;
        }
    };
    if let Err(e) = file.seek(SeekFrom::Start(cursor_now)).await {
        warn!("seek {} failed: {e}", path.display());
        return;
    }
    let mut new_chunk = Vec::with_capacity((file_len - cursor_now) as usize);
    if let Err(e) = file.read_to_end(&mut new_chunk).await {
        warn!("read tail of {} failed: {e}", path.display());
        return;
    }

    // Consume only up to the last complete (newline-terminated) line; any
    // partial trailing line stays buffered until the next notify event
    // completes it. `safe_end_relative` is within `new_chunk`.
    let safe_end_relative = match new_chunk.iter().rposition(|&b| b == b'\n') {
        Some(i) => i + 1,
        None => 0,
    };
    if safe_end_relative == 0 {
        return; // only a partial line — wait for more
    }
    let new_cursor = cursor_now + safe_end_relative as u64;
    {
        let mut cursors_g = cursors.lock().await;
        cursors_g.insert(path.to_path_buf(), new_cursor);
    }

    let new_bytes = &new_chunk[..safe_end_relative];
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
            // First-sight only: cursor_now==0 here, so new_bytes is the
            // entire transcript prefix — fine to scan for cwd.
            let cwd = extract_cwd(new_bytes).unwrap_or_default();
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
