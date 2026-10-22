use std::time::Duration;

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use tokio::sync::oneshot;

use crate::{
    mcp::RoprobeMcp,
    protocol::{OpResult, Operation},
    server::{OpRequest, SessionId},
};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetActiveArgs {
    /// Session id from list-sessions.
    id: u32,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ExecArgs {
    /// Luau source to run.
    source: String,
    /// Target session id; defaults to the active one.
    session: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct EvalArgs {
    /// Luau source to run; `return` or print to get output.
    source: String,
    /// Target session id; defaults to the active one.
    session: Option<u32>,
    /// Seconds to wait for a result (default 15).
    timeout: Option<u64>,
}

impl RoprobeMcp {
    async fn sink(&self, session: Option<u32>) -> Result<tokio::sync::mpsc::Sender<OpRequest>, String> {
        let holder = self.ctx.sessions.holder.read().await;
        let s = match session {
            Some(id) => holder.find(SessionId(id)),
            None => holder.current(),
        };
        let s = s.ok_or("no such session (or none active)")?;
        s.mirror.op_sink().ok_or_else(|| "session has no live op sink".to_string())
    }

    async fn run(&self, source: String, session: Option<u32>) -> Result<oneshot::Receiver<OpResult>, String> {
        let sink = self.sink(session).await?;
        let (reply, rx) = oneshot::channel();
        let op = OpRequest { operation: Operation::RunCode { source }, reply };
        sink.send(op).await.map_err(|_| "session stopped accepting ops")?;
        Ok(rx)
    }
}

#[tool_router(router = core_router, vis = "pub(crate)")]
impl RoprobeMcp {
    #[tool(name = "list-sessions", description = "List connected Roblox sessions and which is active.")]
    async fn list_sessions(&self) -> Result<String, String> {
        let list = self.ctx.sessions.holder.read().await.list();
        serde_json::to_string(&list).map_err(|e| e.to_string())
    }

    #[tool(name = "set-active", description = "Make a session the active one.")]
    async fn set_active(&self, Parameters(a): Parameters<SetActiveArgs>) -> Result<String, String> {
        let id = self.ctx.sessions.set_current(Some(SessionId(a.id))).await.map_err(|e| e.to_string())?;
        Ok(format!("active: {}", id.map_or(0, |i| i.0)))
    }

    #[tool(name = "execute", description = "Run Luau in a session, fire and forget. Use eval for output.")]
    async fn execute(&self, Parameters(a): Parameters<ExecArgs>) -> Result<String, String> {
        let rx = self.run(a.source, a.session).await?;
        tokio::spawn(rx); // yeet the result
        Ok("sent".into())
    }

    #[tool(name = "eval", description = "Run Luau in a session and wait for its output.")]
    async fn eval(&self, Parameters(a): Parameters<EvalArgs>) -> Result<String, String> {
        let rx = self.run(a.source, a.session).await?;
        let wait = Duration::from_secs(a.timeout.unwrap_or(15));
        match tokio::time::timeout(wait, rx).await {
            Err(_) => Err("timed out".into()),
            Ok(Err(_)) => Err("session disconnected before answering".into()),
            Ok(Ok(OpResult::Output(s))) => Ok(s),
            Ok(Ok(OpResult::Ok)) => Ok(String::new()),
            Ok(Ok(OpResult::Err(e))) => Err(e),
            Ok(Ok(OpResult::Reads(r))) => Ok(format!("{r:?}")),
        }
    }
}
