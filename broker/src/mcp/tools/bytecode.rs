use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    mcp::{RoprobeMcp, bytecode_parser},
    protocol::Operation,
};

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ScriptArgs {
    /// Full path of the script, e.g. `game.ReplicatedStorage.Mod`.
    script: String,
    /// Target session id; defaults to the active one.
    session: Option<u32>,
    /// Seconds to wait (default 15).
    timeout: Option<u64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ClosureArgs {
    /// Path to a function or script, as the client resolves it.
    path: String,
    /// Target session id; defaults to the active one.
    session: Option<u32>,
    /// Seconds to wait (default 15).
    timeout: Option<u64>,
}

impl RoprobeMcp {
    async fn ask(&self, op: Operation, session: Option<u32>, secs: u64) -> Result<String, String> {
        let sink = self.sink(session).await?;
        let (reply, rx) = tokio::sync::oneshot::channel();
        sink.send(crate::server::OpRequest { operation: op, reply })
            .await
            .map_err(|_| "session stopped accepting ops")?;
        Self::wait(rx, secs).await
    }

    async fn closure(&self, a: ClosureArgs) -> Result<Value, String> {
        let out = self
            .ask(Operation::InspectClosure { path: a.path }, a.session, a.timeout.unwrap_or(15))
            .await?;
        serde_json::from_str(&out).map_err(|e| format!("bad closure reply: {e}"))
    }
}

#[tool_router(router = bytecode_router, vis = "pub(crate)")]
impl RoprobeMcp {
    #[tool(name = "disassemble", description = "Fetch a script's compiled Luau bytecode: version, size, base64.")]
    async fn disassemble(&self, Parameters(a): Parameters<ScriptArgs>) -> Result<String, String> {
        let op = Operation::GetBytecode { script_path: a.script };
        let out = self.ask(op, a.session, a.timeout.unwrap_or(15)).await?;
        Ok(bytecode_parser::summarize(&out)?.to_string())
    }

    #[tool(name = "constants", description = "Constants of a closure.")]
    async fn constants(&self, Parameters(a): Parameters<ClosureArgs>) -> Result<String, String> {
        let v = self.closure(a).await?;
        Ok(json!({"constants": v["constants"]}).to_string())
    }

    #[tool(name = "upvalues", description = "Upvalues of a closure.")]
    async fn upvalues(&self, Parameters(a): Parameters<ClosureArgs>) -> Result<String, String> {
        let v = self.closure(a).await?;
        Ok(json!({"upvalues": v["upvalues"]}).to_string())
    }

    #[tool(name = "inspect-closure", description = "Name, params, constants, upvalues, protos and source of a closure.")]
    async fn inspect_closure(&self, Parameters(a): Parameters<ClosureArgs>) -> Result<String, String> {
        Ok(self.closure(a).await?.to_string())
    }
}
