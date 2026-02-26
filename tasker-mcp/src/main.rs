//! Tasker MCP Server
//!
//! Model Context Protocol server exposing Tasker developer tooling
//! (code generation, template parsing, schema inspection) to LLM agents,
//! developer IDEs, and operational tooling.
//!
//! This is a scaffold — no tools are registered yet. The server responds
//! to the MCP `initialize` handshake and returns server info.

mod server;

use rmcp::ServiceExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("tasker_mcp=info".parse()?))
        .with_writer(std::io::stderr)
        .init();

    tracing::info!("tasker-mcp starting (stdio transport)");

    let server = server::TaskerMcpServer::new();
    let transport = rmcp::transport::io::stdio();

    let service = server.serve(transport).await?;
    service.waiting().await?;

    Ok(())
}
