#![allow(deprecated)] // logging notifs are all we've got

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use rmcp::{
    Peer, RoleServer,
    model::{LoggingLevel, LoggingMessageNotificationParam},
};
use serde_json::{Value, json};
use tokio::{sync::broadcast::error::RecvError, task::JoinHandle};

use crate::{
    protocol::{HookEvent, TraceCall},
    server::{SessionId, manager::Sessions},
};

/// Forwards hook and trace broadcasts to one MCP client.
#[derive(Default)]
pub struct Streams {
    watched: Arc<Mutex<HashSet<SessionId>>>,
    hooks: Mutex<Option<JoinHandle<()>>>,
    traces: Mutex<Option<JoinHandle<()>>>,
}

fn alive(slot: &Mutex<Option<JoinHandle<()>>>) -> bool {
    slot.lock().unwrap().as_ref().is_some_and(|h| !h.is_finished())
}

async fn emit(peer: &Peer<RoleServer>, logger: &str, data: Value) -> bool {
    let mut p = LoggingMessageNotificationParam::new(LoggingLevel::Info, data);
    p.logger = Some(logger.into());
    peer.notify_logging_message(p).await.is_ok()
}

impl Streams {
    pub fn watch_hooks(&self, peer: Peer<RoleServer>, sessions: &Sessions, id: SessionId) {
        self.watched.lock().unwrap().insert(id);
        if alive(&self.hooks) {
            return;
        }
        let (mut rx, watched) = (sessions.subscribe_hooks(), self.watched.clone());
        let task = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((sid, ev)) => {
                        if !watched.lock().unwrap().contains(&sid) {
                            continue;
                        }
                        if !emit(&peer, "roprobe.hook", hook_json(sid, &ev)).await {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        });
        *self.hooks.lock().unwrap() = Some(task);
    }

    pub fn watch_traces(&self, peer: Peer<RoleServer>, sessions: &Sessions, id: SessionId) {
        self.watched.lock().unwrap().insert(id);
        if alive(&self.traces) {
            return;
        }
        let (mut rx, watched) = (sessions.subscribe_traces(), self.watched.clone());
        let task = tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok((sid, call)) => {
                        if !watched.lock().unwrap().contains(&sid) {
                            continue;
                        }
                        if !emit(&peer, "roprobe.trace", trace_json(sid, &call)).await {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        });
        *self.traces.lock().unwrap() = Some(task);
    }
}

impl Drop for Streams {
    fn drop(&mut self) {
        for slot in [&self.hooks, &self.traces] {
            if let Some(h) = slot.lock().unwrap().take() {
                h.abort();
            }
        }
    }
}

fn hook_json(sid: SessionId, e: &HookEvent) -> Value {
    json!({
        "session": sid.0,
        "remote": e.remote.path,
        "class": e.remote.class,
        "args": e.args,
        "returns": e.returns,
        "caller": e.caller,
        "t": e.timestamp,
    })
}

fn trace_json(sid: SessionId, c: &TraceCall) -> Value {
    json!({
        "session": sid.0,
        "target": c.target,
        "args": c.args,
        "returns": c.returns,
        "t": c.timestamp,
    })
}
