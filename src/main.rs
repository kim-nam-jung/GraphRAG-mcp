use anyhow::Result;
use graphrag_mcp::*;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    info!("Starting GraphRAG MCP Rust Server...");

    let cfg = config::load_config("configs/default.yaml")?;

    let db = storage::Database::new(&cfg.storage.db_path, cfg.storage.wal_mode)?;
    let tokenizer = embedding::Tokenizer::new(&cfg.embedding.tokenizer_path)?;
    let harrier = embedding::HarrierModel::new(&cfg.embedding.model_path, cfg.embedding.dimension)?;

    let search_engine = search::SearchEngine::new(&db, &harrier, &tokenizer, &cfg);
    let mcp_server = mcp::McpServer::new(search_engine, &db, &harrier, &tokenizer, &cfg);

    if let Err(e) = mcp_server.run_stdio().await {
        error!("MCP Server terminated with error: {:?}", e);
        std::process::exit(1);
    }

    Ok(())
}
