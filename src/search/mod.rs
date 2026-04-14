use crate::config::Config;
use crate::embedding::HarrierModel;
use crate::embedding::Tokenizer;
use crate::storage::Database;
use anyhow::Result;
use serde_json::json;
use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

pub struct SearchEngine<'a> {
    db: &'a Database,
    model: &'a HarrierModel,
    tokenizer: &'a Tokenizer,
    cfg: &'a Config,
    last_reindex: Mutex<Option<Instant>>,
}

impl<'a> SearchEngine<'a> {
    pub fn new(
        db: &'a Database,
        model: &'a HarrierModel,
        tokenizer: &'a Tokenizer,
        cfg: &'a Config,
    ) -> Self {
        Self {
            db,
            model,
            tokenizer,
            cfg,
            last_reindex: Mutex::new(None),
        }
    }

    pub fn local_search(&self, query: &str, top_k: u32, graph_depth: u32) -> Result<String> {
        // [auto-reindex] Cooldown Check & Reindexing Hook
        if self.cfg.indexer.auto_reindex {
            let mut last_idx = self.last_reindex.lock().unwrap_or_else(|e| e.into_inner());
            let should_reindex = match *last_idx {
                Some(time) => time.elapsed().as_secs() >= self.cfg.indexer.reindex_cooldown_sec,
                None => true,
            };

            if should_reindex {
                tracing::info!(
                    "Auto-reindex condition met (cooldown {}s). Triggering indexing pipeline...",
                    self.cfg.indexer.reindex_cooldown_sec
                );
                // Pipeline requires 4 parameters dynamically mirroring search
                let pipeline = crate::indexer::pipeline::IndexingPipeline::new(
                    self.db,
                    self.model,
                    self.tokenizer,
                    self.cfg,
                );
                if let Err(e) = pipeline.run_indexing(Path::new(&self.cfg.indexer.project_root)) {
                    tracing::error!("Auto-reindex failed during local_search hook: {}", e);
                } else {
                    tracing::info!("Auto-reindexing succeeded.");
                }
                *last_idx = Some(Instant::now());
            }
        }

        let embedding = self.model.embed(
            query,
            true,
            &self.cfg.embedding.query_instruction,
            self.tokenizer,
        )?;

        let similar = self.db.search_similar_chunks(&embedding, top_k)?;

        let mut entry_points = vec![];
        let mut chunks = vec![];

        for (chunk_id, distance) in &similar {
            if let Ok((text, file_path)) = self.db.get_chunk_by_id(*chunk_id) {
                chunks.push(json!({
                    "chunk_id": chunk_id,
                    "file_path": file_path,
                    "distance": distance,
                    "text": text,
                }));
            }
        }

        // Graph expansion: get neighbor entities for each chunk's associated entities
        let mut graph_context = vec![];
        if graph_depth > 0 {
            // Collect entity names from FTS on the query to seed graph expansion
            let fts_results = self.db.search_fts(query, 5)?;
            for result in &fts_results {
                if !result.name.is_empty() {
                    entry_points.push(json!({
                        "name": result.name,
                        "type": result.entity_type,
                        "file": result.file_path,
                    }));

                    let neighbors = self.db.graph_neighbors(&result.name, graph_depth, "both")?;
                    for n in &neighbors {
                        graph_context.push(json!({
                            "name": n.name,
                            "type": n.entity_type,
                            "file": n.file_path,
                        }));
                    }
                }
            }
        }

        let output = json!({
            "entry_points": entry_points,
            "graph_context": graph_context,
            "chunks": chunks,
        });

        Ok(serde_json::to_string_pretty(&output)?)
    }

    pub fn global_search(&self, query: &str, max_entities: u32) -> Result<String> {
        let embedding = self.model.embed(
            query,
            true,
            &self.cfg.embedding.query_instruction,
            self.tokenizer,
        )?;
        let results = self.db.search_similar_entities(&embedding, max_entities)?;

        let items: Vec<_> = results
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "type": r.entity_type,
                    "file_path": r.file_path,
                    "qualified_name": r.snippet,
                })
            })
            .collect();

        Ok(serde_json::to_string_pretty(&json!({ "results": items }))?)
    }
}
