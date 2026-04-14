use graphrag_mcp::{config, embedding, search::SearchEngine, storage};

fn main() -> anyhow::Result<()> {
    let cfg = config::load_config("configs/default.yaml")?;
    let db = storage::Database::new(&cfg.storage.db_path, cfg.storage.wal_mode)?;
    let tokenizer = embedding::Tokenizer::new(&cfg.embedding.tokenizer_path)?;
    let harrier = embedding::HarrierModel::new(&cfg.embedding.model_path, cfg.embedding.dimension)?;

    let search_engine = SearchEngine::new(&db, &harrier, &tokenizer, &cfg);
    
    // Top-3 chunks, 1-hop depth
    let result = search_engine.local_search("leiden algorithm community detection", 3, 1)?;
    println!("{}", result);
    
    Ok(())
}
