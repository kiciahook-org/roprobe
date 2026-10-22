use rmcp::handler::server::router::tool::ToolRouter;

use super::RoprobeMcp;

pub fn router() -> ToolRouter<RoprobeMcp> {
    ToolRouter::new() // no tools yet, vibes only
}
