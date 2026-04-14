use graphrag_mcp::{config, embedding, indexer::pipeline::IndexingPipeline, storage};
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let cfg = config::load_config("configs/default.yaml")?;
    let db = storage::Database::new(&cfg.storage.db_path, cfg.storage.wal_mode)?;
    let tokenizer = embedding::Tokenizer::new(&cfg.embedding.tokenizer_path)?;
    let harrier = embedding::HarrierModel::new(&cfg.embedding.model_path, cfg.embedding.dimension)?;

    let pipeline = IndexingPipeline::new(&db, &harrier, &tokenizer, &cfg);
    pipeline.run_indexing(Path::new("./src"))?;
    
    println!("Indexing completed successfully!");
    Ok(())
}
