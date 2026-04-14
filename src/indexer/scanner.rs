use anyhow::Result;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use tracing::info;
use crate::config::IndexerConfig;

pub struct Scanner;

impl Scanner {
    pub fn scan_directory(path: &Path, cfg: &IndexerConfig) -> Result<Vec<PathBuf>> {
        info!("Scanning directory: {:?}", path);

        let all_extensions: Vec<&str> = cfg.tier1.iter()
            .chain(cfg.tier2.iter())
            .chain(cfg.tier3.iter())
            .map(|s| s.as_str())
            .collect();

        let mut files = Vec::new();

        let walker = WalkBuilder::new(path)
            .hidden(true)
            .git_ignore(true)
            .build();

        for result in walker {
            match result {
                Ok(entry) => {
                    let p = entry.path();

                    // Skip excluded directories
                    if p.is_dir() {
                        if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                            if cfg.exclude_dirs.iter().any(|d| d == name) {
                                continue;
                            }
                        }
                    }

                    if !p.is_file() {
                        continue;
                    }

                    // Check extension against tier lists
                    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                    if !all_extensions.contains(&ext) {
                        continue;
                    }

                    // Check exclude file patterns
                    let file_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let excluded = cfg.exclude_files.iter().any(|pattern| {
                        matches_glob(pattern, file_name)
                    });
                    if excluded {
                        continue;
                    }

                    files.push(p.to_path_buf());
                }
                Err(e) => {
                    tracing::warn!("Failed to access entry: {}", e);
                }
            }
        }

        info!("Scanner found {} files.", files.len());
        Ok(files)
    }
}

fn matches_glob(pattern: &str, name: &str) -> bool {
    if pattern.starts_with('*') {
        name.ends_with(&pattern[1..])
    } else if pattern.ends_with('*') {
        name.starts_with(&pattern[..pattern.len() - 1])
    } else {
        pattern == name
    }
}
