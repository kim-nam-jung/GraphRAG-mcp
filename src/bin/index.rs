use graphrag_mcp::{config, embedding, indexer::pipeline::IndexingPipeline, storage};
use std::path::Path;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::load_config("configs/default.yaml")?;

    let (model_path, tokenizer_path) = graphrag_mcp::downloader::ensure_model_files(
        &cfg.embedding.model_path,
        &cfg.embedding.tokenizer_path,
    ).await?;

    let db = storage::Database::new(&cfg.storage.db_path, cfg.storage.wal_mode)?;
    let tokenizer = embedding::Tokenizer::new(&tokenizer_path)?;
    let harrier = embedding::HarrierModel::new(&model_path, cfg.embedding.dimension)?;

    let pipeline = IndexingPipeline::new(&db, &harrier, &tokenizer, &cfg);
    pipeline.run_indexing(Path::new("./src"))?;
    
    println!("Indexing completed successfully!");
    Ok(())
}
