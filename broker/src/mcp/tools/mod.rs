mod core;

use rmcp::handler::server::router::tool::ToolRouter;

use super::RoprobeMcp;

pub fn router() -> ToolRouter<RoprobeMcp> {
    RoprobeMcp::core_router()
}
