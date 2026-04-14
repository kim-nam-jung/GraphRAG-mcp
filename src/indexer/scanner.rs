use anyhow::Result;
use std::path::{Path, PathBuf};
use ignore::WalkBuilder;
use glob::Pattern;
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
                    let excluded = cfg.exclude_files.iter().any(|pattern_str| {
                        if let Ok(pat) = Pattern::new(pattern_str) {
                            pat.matches(file_name)
                        } else {
                            false
                        }
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

