use anyhow::{Context, Result};
use tiktoken_rs::CoreBPE;
use std::sync::Arc;
use tracing::info;

pub struct Tokenizer {
    tkm: Arc<CoreBPE>,
}

impl Tokenizer {
    pub fn new(_tokenizer_path: &str) -> Result<Self> {
        // Our Harrier model requires standard cl100k_base embedding.
        // We explicitly ignore the path and load the official matching vocab 
        // to guarantee exact token identity with Go cl100k_base mapping!
        let bpe = tiktoken_rs::cl100k_base().context("Failed to load cl100k_base tokenizer")?;
        info!("Loaded pure-Rust Tiktoken tokenizer for cl100k_base");
        
        Ok(Self {
            tkm: Arc::new(bpe),
        })
    }

    pub fn encode(&self, text: &str, add_special_tokens: bool) -> Result<Vec<u32>> {
        let ids = if add_special_tokens {
            self.tkm.encode_with_special_tokens(text)
        } else {
            self.tkm.encode_ordinary(text)
        };

        // Downcast to u32 to match embedding dimension types mapped to ONNX tensors
        let uint_ids: Vec<u32> = ids.into_iter().map(|id| id as u32).collect();
        Ok(uint_ids)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenizer_cl100k_base() {
        // new() expects a dummy path, it loads official cl100k_base anyway
        let tkm = Tokenizer::new("dummy").expect("Should load cl100k_base");
        
        // Encode test
        let text = "Hello world! This is a test.";
        let encoded = tkm.encode(text, false).expect("Should encode text");
        
        assert!(!encoded.is_empty(), "Encoded array must not be empty");
        // 'Hello' -> 9906, ' world' -> 1917, '!' -> 0 (wait, ! is a token), etc.
        assert!(encoded.len() > 3, "Should have multiple tokens");
        
        let encoded_with_special = tkm.encode(text, true).expect("Should encode with special tokens");
        assert_eq!(encoded.len(), encoded_with_special.len(), "Without special tokens in string, length matches");
    }
}
