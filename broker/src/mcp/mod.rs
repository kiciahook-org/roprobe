pub mod tools;

use rmcp::{
    ServerHandler, ServiceExt,
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerConfig},
    tool_handler,
};

use crate::Context;

#[derive(Clone)]
pub struct RoprobeMcp {
    #[allow(dead_code)]
    pub ctx: Context,
    tool_router: ToolRouter<Self>,
}

impl RoprobeMcp {
    pub fn new(ctx: Context) -> Self {
        Self { ctx, tool_router: tools::router() }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RoprobeMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
    }
}

pub async fn start(ctx: Context) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let svc = RoprobeMcp::new(ctx).serve(rmcp::transport::stdio()).await?;
    tracing::info!("mcp up on stdio");
    svc.waiting().await?;
    Ok(())
}
