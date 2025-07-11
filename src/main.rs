use anyhow::Result;
use mcp_memory::handler::{GraphServiceHandler, KnowledgeGraphService};
use rmcp::ServiceExt;
use tokio::io::{stdin, stdout};

#[tokio::main]
async fn main() -> Result<()> {
    let transport = (stdin(), stdout());

    let graph_service = KnowledgeGraphService::new();
    let graph_server = GraphServiceHandler::new(graph_service);

    let server = graph_server.serve(transport).await?;
    server.waiting().await?;
    Ok(())
}
