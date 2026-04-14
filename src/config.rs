use serde::{Deserialize, Serialize};
use std::fs;
use anyhow::{Context, Result};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerConfig {
    pub transport: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IndexerConfig {
    pub tier1: Vec<String>,
    pub tier2: Vec<String>,
    pub tier3: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub exclude_files: Vec<String>,
    pub chunk_max_lines: usize,
    #[serde(default)]
    pub project_root: String,
    #[serde(default)]
    pub auto_reindex: bool,
    #[serde(default = "default_reindex_cooldown")]
    pub reindex_cooldown_sec: u64,
}

fn default_reindex_cooldown() -> u64 { 10 }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmbeddingConfig {
    pub model_path: String,
    pub tokenizer_path: String,
    pub quantization: String,
    pub dimension: usize,
    pub max_tokens: usize,
    pub query_instruction: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StorageConfig {
    pub db_path: String,
    pub wal_mode: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GraphConfig {
    pub leiden_resolution: f32,
    pub min_community_size: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchConfig {
    pub auto_reindex: bool,
    pub reindex_cooldown_sec: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub indexer: IndexerConfig,
    pub embedding: EmbeddingConfig,
    pub storage: StorageConfig,
    pub graph: GraphConfig,
    pub search: SearchConfig,
}

pub fn load_config(path: &str) -> Result<Config> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config from {}", path))?;
    let config: Config = serde_yaml::from_str(&content)
        .with_context(|| "Failed to parse YAML config")?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_parsing() {
        let yaml = r#"
server:
  transport: "stdio"
indexer:
  tier1: ["*.rs"]
  tier2: []
  tier3: []
  exclude_dirs: [".git"]
  exclude_files: [".env"]
  chunk_max_lines: 300
  project_root: "/test"
  auto_reindex: true
  reindex_cooldown_sec: 120
embedding:
  model_path: "test"
  tokenizer_path: "test"
  quantization: "f32"
  dimension: 256
  max_tokens: 512
  query_instruction: "Represent:"
storage:
  db_path: ":memory:"
  wal_mode: false
graph:
  leiden_resolution: 1.0
  min_community_size: 2
search:
  auto_reindex: true
  reindex_cooldown_sec: 120
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.storage.db_path, ":memory:");
        assert_eq!(config.indexer.tier1, vec!["*.rs".to_string()]);
        assert_eq!(config.indexer.auto_reindex, true);
        assert_eq!(config.indexer.reindex_cooldown_sec, 120);
    }
}
