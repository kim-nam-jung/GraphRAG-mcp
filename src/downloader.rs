use anyhow::{Context, Result};
use futures_util::StreamExt;
use std::fs;
use std::path::{Path, PathBuf};

// Default HuggingFace URLs for a lightweight 384-dim ONNX model
const DEFAULT_MODEL_URL: &str =
    "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx";
const DEFAULT_TOKENIZER_URL: &str =
    "https://huggingface.co/Xenova/all-MiniLM-L6-v2/resolve/main/tokenizer.json";

/// Helper to download a file with progress logging
async fn download_file(url: &str, dest: &Path) -> Result<()> {
    tracing::info!("Downloading {} to {}", url, dest.display());
    
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }

    let response = reqwest::get(url).await?;
    response.error_for_status_ref()?;

    let total_size = response
        .content_length()
        .unwrap_or(0);

    let mut stream = response.bytes_stream();
    let mut file = fs::File::create(dest)?;
    use std::io::Write;

    let mut downloaded: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        downloaded += chunk.len() as u64;
    }

    tracing::info!("Downloaded {} bytes successfully.", downloaded);
    Ok(())
}

/// Ensures the model and tokenizer files exist. If not, downloads them to the global app cache.
/// Returns the updated (model_path, tokenizer_path) to use.
pub async fn ensure_model_files(
    cfg_model_path: &str,
    cfg_tokenizer_path: &str,
) -> Result<(String, String)> {
    let mut final_model = PathBuf::from(cfg_model_path);
    let mut final_tokenizer = PathBuf::from(cfg_tokenizer_path);

    // If both files already exist locally as configured, we just use them
    if final_model.exists() && final_tokenizer.exists() {
        return Ok((
            final_model.to_string_lossy().to_string(),
            final_tokenizer.to_string_lossy().to_string(),
        ));
    }

    // Otherwise, we use the global caching directory
    if let Some(proj_dirs) = directories::ProjectDirs::from("com", "valscribe", "graphrag-mcp") {
        let cache_dir = proj_dirs.cache_dir().join("models");
        final_model = cache_dir.join("model.onnx");
        final_tokenizer = cache_dir.join("tokenizer.json");

        if !final_model.exists() {
            tracing::info!("ONNX Model not found locally. Starting auto-download...");
            download_file(DEFAULT_MODEL_URL, &final_model).await?;
        }
        
        if !final_tokenizer.exists() {
            tracing::info!("Tokenizer not found locally. Starting auto-download...");
            download_file(DEFAULT_TOKENIZER_URL, &final_tokenizer).await?;
        }
        
        return Ok((
            final_model.to_string_lossy().to_string(),
            final_tokenizer.to_string_lossy().to_string(),
        ));
    }

    anyhow::bail!("Failed to determine global cache directory for model downloads. Please manually provide the model files at the config locations.");
}
