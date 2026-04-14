use anyhow::Result;
use std::path::Path;
use std::fs;
use tracing::{info, warn};

use crate::config::Config;
use crate::storage::Database;
use crate::embedding::{HarrierModel, Tokenizer};
use super::scanner::Scanner;
use super::extractors::base::{Extractor, Entity, Relation};
use super::extractors::golang::GoExtractor;
use super::extractors::javascript::JsExtractor;
use super::extractors::python::PyExtractor;
use super::extractors::typescript::TsExtractor;
use super::extractors::rust::RustExtractor;
use super::extractors::java::JavaExtractor;
use super::extractors::cpp::CppExtractor;
use super::extractors::c_sharp::CSharpExtractor;
use super::extractors::ruby::RubyExtractor;
// use super::extractors::php::PhpExtractor;
// use super::extractors::swift::SwiftExtractor;
// use super::extractors::kotlin::KotlinExtractor;
// use super::extractors::scala::ScalaExtractor;
use crate::graph::LeidenNative;
use std::collections::HashMap;

pub struct IndexingPipeline<'a> {
    db: &'a Database,
    harrier: &'a HarrierModel,
    tokenizer: &'a Tokenizer,
    cfg: &'a Config,
}

impl<'a> IndexingPipeline<'a> {
    pub fn new(db: &'a Database, harrier: &'a HarrierModel, tokenizer: &'a Tokenizer, cfg: &'a Config) -> Self {
        Self { db, harrier, tokenizer, cfg }
    }

    pub fn run_indexing(&self, project_root: &Path) -> Result<()> {
        info!("Starting indexing pipeline on {:?}", project_root);

        let files = Scanner::scan_directory(project_root, &self.cfg.indexer)?;

        let mut graph = LeidenNative::new(self.cfg.graph.leiden_resolution);
        let mut entity_ids: HashMap<String, i64> = HashMap::new();

        for path in &files {
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let path_str = path.to_string_lossy().to_string();
            let ext = path.extension().unwrap_or_default().to_str().unwrap_or("");

            let (entities, relations) = self.extract_code(ext, &content)?;

            // Insert entities
            for ent in &entities {
                match self.db.insert_entity(&path_str, &ent.name, &ent.entity_type, &ent.qualified_name) {
                    Ok(id) => {
                        entity_ids.insert(ent.qualified_name.clone(), id);
                        graph.add_edge(&path_str, &ent.name, 1.0);
                    }
                    Err(e) => warn!("Failed to insert entity {}: {}", ent.name, e),
                }
            }

            // Insert relations
            for rel in &relations {
                let src_id = entity_ids.get(&rel.source);
                let tgt_id = entity_ids.get(&rel.target);
                if let (Some(&sid), Some(&tid)) = (src_id, tgt_id) {
                    if let Err(e) = self.db.insert_relation(sid, tid, &rel.relation_type, 1.0) {
                        warn!("Failed to insert relation {} -> {}: {}", rel.source, rel.target, e);
                    }
                    graph.add_edge(&rel.source, &rel.target, 1.0);
                }
            }

            // Semantic Chunk and embed
            let chunks = self.chunk_semantically(&content, &entities, self.cfg.indexer.chunk_max_lines);
            for (chunk, line_start, line_end) in chunks {
                match self.db.insert_chunk(&chunk, &path_str, Some(line_start), Some(line_end), None) {
                    Ok(chunk_id) => {
                        match self.harrier.embed(&chunk, false, "", self.tokenizer) {
                            Ok(embedding) => {
                                if let Err(e) = self.db.write_chunk_embedding(chunk_id, &embedding) {
                                    warn!("Failed to write embedding for chunk {}: {}", chunk_id, e);
                                }
                            }
                            Err(e) => warn!("Failed to embed chunk: {}", e),
                        }
                    }
                    Err(e) => warn!("Failed to insert chunk: {}", e),
                }
            }
        }

        let _communities = graph.calculate()?;
        info!("Indexing finished. Processed {} files.", files.len());
        Ok(())
    }

    fn extract_code(&self, ext: &str, content: &str) -> Result<(Vec<Entity>, Vec<Relation>)> {
        match ext {
            "go" => {
                let mut parser = GoExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "js" | "jsx" => {
                let mut parser = JsExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "ts" | "tsx" => {
                let mut parser = TsExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "py" => {
                let mut parser = PyExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "rs" => {
                let mut parser = RustExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "java" => {
                let mut parser = JavaExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "c" | "cpp" | "h" | "hpp" | "cc" => {
                let mut parser = CppExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "cs" => {
                let mut parser = CSharpExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            "rb" => {
                let mut parser = RubyExtractor::new()?;
                parser.parse(content)?;
                parser.extract()
            }
            // "php" => {
            //     let mut parser = PhpExtractor::new()?;
            //     parser.parse(content)?;
            //     parser.extract()
            // }
            // "swift" => {
            //     let mut parser = SwiftExtractor::new()?;
            //     parser.parse(content)?;
            //     parser.extract()
            // }
            // "kt" | "kts" => {
            //     let mut parser = KotlinExtractor::new()?;
            //     parser.parse(content)?;
            //     parser.extract()
            // }
            // "scala" | "sc" => {
            //     let mut parser = ScalaExtractor::new()?;
            //     parser.parse(content)?;
            //     parser.extract()
            // }
            _ => Ok((vec![], vec![])),
        }
    }

    fn chunk_semantically(&self, text: &str, entities: &[Entity], max_lines: usize) -> Vec<(String, i64, i64)> {
        let mut chunks = Vec::new();
        let bytes = text.as_bytes();
        let mut covered_ranges = Vec::new();
        
        for ent in entities {
            if ent.end_byte > ent.start_byte && ent.end_byte <= bytes.len() {
                let chunk_text = &text[ent.start_byte..ent.end_byte];
                let line_start = text[..ent.start_byte].lines().count() as i64 + 1;
                let line_end = (line_start + chunk_text.lines().count() as i64).saturating_sub(1).max(line_start);
                
                if chunk_text.lines().count() <= max_lines {
                    chunks.push((chunk_text.to_string(), line_start, line_end));
                } else {
                    chunks.extend(self.chunk_by_lines_with_offset(chunk_text, max_lines, line_start));
                }
                covered_ranges.push((ent.start_byte, ent.end_byte));
            }
        }
        
        covered_ranges.sort_unstable_by_key(|&(s, _)| s);
        let mut merged = Vec::new();
        for (s, e) in covered_ranges {
            if let Some(&mut (_, ref mut me)) = merged.last_mut() {
                if s <= *me {
                    *me = (*me).max(e);
                } else {
                    merged.push((s, e));
                }
            } else {
                merged.push((s, e));
            }
        }
        
        let mut last_end = 0;
        for (s, e) in merged {
            if s > last_end {
                let span = &text[last_end..s];
                if span.trim().len() > 10 {
                    let line_start = text[..last_end].lines().count() as i64 + 1;
                    chunks.extend(self.chunk_by_lines_with_offset(span, max_lines, line_start));
                }
            }
            last_end = last_end.max(e);
        }
        if last_end < bytes.len() {
            let span = &text[last_end..];
            if span.trim().len() > 10 {
                let line_start = text[..last_end].lines().count() as i64 + 1;
                chunks.extend(self.chunk_by_lines_with_offset(span, max_lines, line_start));
            }
        }

        chunks
    }

    fn chunk_by_lines_with_offset(&self, text: &str, max_lines: usize, start_line_offset: i64) -> Vec<(String, i64, i64)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut chunks = vec![];
        let mut start = 0;
        while start < lines.len() {
            let end = (start + max_lines).min(lines.len());
            let chunk = lines[start..end].join("\n");
            if !chunk.trim().is_empty() {
                chunks.push((chunk, start_line_offset + start as i64, start_line_offset + end as i64 - 1));
            }
            start = end;
        }
        chunks
    }
}
