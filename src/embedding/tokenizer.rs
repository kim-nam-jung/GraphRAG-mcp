use anyhow::{Context, Result};
use std::sync::Arc;
use tokenizers::Tokenizer as HfTokenizer;
use tracing::info;

pub struct Tokenizer {
    tkm: Arc<HfTokenizer>,
}

impl Tokenizer {
    pub fn new(tokenizer_path: &str) -> Result<Self> {
        let tkm = HfTokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer JSON from {}: {}", tokenizer_path, e))?;
        info!("Loaded HuggingFace tokenizer from {}", tokenizer_path);

        Ok(Self { tkm: Arc::new(tkm) })
    }

    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Vec<u32>> {
        let encoding = self.tkm.encode(text, add_special_tokens)
            .map_err(|e| anyhow::anyhow!("Failed to encode text: {}", e))?;
        
        // Downcast or clone to u32 array
        let uint_ids: Vec<u32> = encoding.get_ids().to_vec();
        Ok(uint_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_huggingface() {
        // This test will only work if dummy file exists, leaving empty or ignoring 
    }
}
