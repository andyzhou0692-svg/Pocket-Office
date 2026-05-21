use anyhow::{anyhow, bail, Result};
use serde_json::Value;

use crate::source::{Activity, AgentEvent};
use crate::AgentId;

pub const SOURCE_NAME: &str = "claude-code";

pub fn decode_hook_payload(v: Value) -> Result<AgentEvent> {
    let obj = v
        .as_object()
        .ok_or_else(|| anyhow!("hook payload must be an object"))?;
    let event = obj
        .get("hook_event_name")
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("missing hook_event_name"))?;

    let session_id = obj
        .get("session_id")
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("missing session_id"))?
        .to_string();
    let transcript_path = obj
        .get("transcript_path")
        .and_then(|s| s.as_str())
        .ok_or_else(|| anyhow!("missing transcript_path"))?;
    let agent_id = AgentId::from_transcript_path(transcript_path);

    match event {
        "SessionStart" => {
            let cwd = obj
                .get("cwd")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .into();
            Ok(AgentEvent::SessionStart {
                agent_id,
                source: SOURCE_NAME.into(),
                session_id,
                cwd,
            })
        }
        "PreToolUse" => {
            let tool_name = obj
                .get("tool_name")
                .and_then(|s| s.as_str())
                .unwrap_or("?");
            let target = describe_tool_target(tool_name, obj.get("tool_input"));
            Ok(AgentEvent::ActivityStart {
                agent_id,
                activity: Activity::Typing,
                tool_use_id: None,
                detail: Some(format!("{tool_name}{target}")),
            })
        }
        "PostToolUse" => Ok(AgentEvent::ActivityEnd {
            agent_id,
            tool_use_id: None,
        }),
        "Notification" => {
            let msg = obj
                .get("message")
                .and_then(|s| s.as_str())
                .unwrap_or("waiting");
            Ok(AgentEvent::Waiting {
                agent_id,
                reason: msg.into(),
            })
        }
        "SessionEnd" => Ok(AgentEvent::SessionEnd { agent_id }),
        other => bail!("unsupported hook_event_name: {other}"),
    }
}

fn describe_tool_target(tool: &str, input: Option<&Value>) -> String {
    let Some(input) = input else {
        return String::new();
    };
    let key = match tool {
        "Write" | "Edit" | "MultiEdit" | "Read" => "file_path",
        "Bash" => "command",
        "Grep" | "Glob" => "pattern",
        _ => "",
    };
    if key.is_empty() {
        return String::new();
    }
    let Some(s) = input.get(key).and_then(|v| v.as_str()) else {
        return String::new();
    };
    let mut s = s.to_string();
    if s.len() > 40 {
        s.truncate(40);
        s.push('…');
    }
    format!(": {s}")
}

/// Decode one JSONL transcript line into 0..N AgentEvents. Unknown / unrelated
/// lines return an empty vec rather than an error so a noisy transcript never
/// kills the watcher.
pub fn decode_jsonl_line(transcript_path: &str, v: Value) -> Result<Vec<AgentEvent>> {
    let agent_id = AgentId::from_transcript_path(transcript_path);
    let Some(obj) = v.as_object() else {
        return Ok(vec![]);
    };
    let ty = obj.get("type").and_then(|s| s.as_str()).unwrap_or("");

    let Some(message) = obj.get("message").and_then(|m| m.as_object()) else {
        return Ok(vec![]);
    };
    let content = message.get("content");

    let mut out = Vec::new();
    match (ty, content) {
        ("assistant", Some(Value::Array(blocks))) => {
            for block in blocks {
                let Some(bobj) = block.as_object() else {
                    continue;
                };
                let btype = bobj.get("type").and_then(|s| s.as_str()).unwrap_or("");
                if btype != "tool_use" {
                    continue;
                }
                let id = bobj
                    .get("id")
                    .and_then(|s| s.as_str())
                    .map(String::from);
                let name = bobj.get("name").and_then(|s| s.as_str()).unwrap_or("?");
                let input = bobj.get("input");
                let target = describe_tool_target(name, input);
                out.push(AgentEvent::ActivityStart {
                    agent_id,
                    activity: Activity::Typing,
                    tool_use_id: id,
                    detail: Some(format!("{name}{target}")),
                });
            }
        }
        ("user", Some(Value::Array(blocks))) => {
            for block in blocks {
                let Some(bobj) = block.as_object() else {
                    continue;
                };
                let btype = bobj.get("type").and_then(|s| s.as_str()).unwrap_or("");
                if btype != "tool_result" {
                    continue;
                }
                let id = bobj
                    .get("tool_use_id")
                    .and_then(|s| s.as_str())
                    .map(String::from);
                out.push(AgentEvent::ActivityEnd {
                    agent_id,
                    tool_use_id: id,
                });
            }
        }
        _ => {}
    }
    Ok(out)
}
