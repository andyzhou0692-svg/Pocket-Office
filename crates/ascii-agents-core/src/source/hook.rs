use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;
use tracing::{debug, warn};

use crate::source::decoder::decode_hook_payload;
use crate::source::AgentEvent;

pub struct HookSocketListener {
    listener: UnixListener,
    path: PathBuf,
}

impl HookSocketListener {
    pub async fn bind(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        // Remove any stale socket file from a previous run.
        if Path::new(&path).exists() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        let listener = UnixListener::bind(&path)
            .with_context(|| format!("binding hook socket at {}", path.display()))?;
        Ok(Self { listener, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn run(self, tx: mpsc::Sender<AgentEvent>) -> Result<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, _addr)) => {
                    let tx = tx.clone();
                    tokio::spawn(handle_conn(stream, tx));
                }
                Err(e) => {
                    warn!("hook socket accept error: {e}");
                }
            }
        }
    }
}

async fn handle_conn(stream: UnixStream, tx: mpsc::Sender<AgentEvent>) {
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if line.trim().is_empty() {
                    continue;
                }
                let v: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        warn!("malformed hook line skipped: {e}");
                        continue;
                    }
                };
                match decode_hook_payload(v) {
                    Ok(ev) => {
                        debug!("hook event: {ev:?}");
                        if tx.send(ev).await.is_err() {
                            return;
                        }
                    }
                    Err(e) => warn!("hook decode error: {e}"),
                }
            }
            Ok(None) => return,
            Err(e) => {
                warn!("hook conn read error: {e}");
                return;
            }
        }
    }
}
