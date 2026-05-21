use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

use crate::id::AgentId;
use crate::state::reducer::Transport;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Typing,
    Reading,
    Thinking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentEvent {
    SessionStart {
        agent_id: AgentId,
        source: String,
        session_id: String,
        cwd: PathBuf,
    },
    ActivityStart {
        agent_id: AgentId,
        activity: Activity,
        tool_use_id: Option<String>,
        detail: Option<String>,
    },
    ActivityEnd {
        agent_id: AgentId,
        tool_use_id: Option<String>,
    },
    Waiting {
        agent_id: AgentId,
        reason: String,
    },
    SessionEnd {
        agent_id: AgentId,
    },
}

impl AgentEvent {
    pub fn agent_id(&self) -> AgentId {
        match self {
            AgentEvent::SessionStart { agent_id, .. } => *agent_id,
            AgentEvent::ActivityStart { agent_id, .. } => *agent_id,
            AgentEvent::ActivityEnd { agent_id, .. } => *agent_id,
            AgentEvent::Waiting { agent_id, .. } => *agent_id,
            AgentEvent::SessionEnd { agent_id, .. } => *agent_id,
        }
    }
}

/// Events sent on a tagged channel so the reducer knows which transport produced them.
pub type TaggedSender = mpsc::Sender<(Transport, AgentEvent)>;
pub type TaggedReceiver = mpsc::Receiver<(Transport, AgentEvent)>;

#[async_trait]
pub trait Source: Send + 'static {
    fn name(&self) -> &str;
    async fn run(self: Box<Self>, tx: TaggedSender) -> anyhow::Result<()>;
}

pub mod claude_code;
pub mod decoder;
pub mod hook;
pub mod jsonl;
