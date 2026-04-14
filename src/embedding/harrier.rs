use anyhow::{Context, Result};
use ort::session::{Session, builder::GraphOptimizationLevel};
use ort::value::Value;
use ort::execution_providers::{CUDAExecutionProvider, CPUExecutionProvider, CoreMLExecutionProvider};
use tracing::info;
use super::tokenizer::Tokenizer;

pub struct HarrierModel {
    session: std::sync::Mutex<Session>,
    pub dim: usize,
}

impl HarrierModel {
    pub fn new(model_path: &str) -> Result<Self> {
        let mut builder = Session::builder().map_err(|e| anyhow::anyhow!("ORT error: {:?}", e))?;
        builder = builder.with_optimization_level(GraphOptimizationLevel::Level3).map_err(|e| anyhow::anyhow!("ORT error: {:?}", e))?;
        builder = builder.with_intra_threads(4).map_err(|e| anyhow::anyhow!("ORT error: {:?}", e))?;
        
        // Attempt GPU acceleration, fallback to CPU
        builder = builder.with_execution_providers([
            CUDAExecutionProvider::default().build(),
            CoreMLExecutionProvider::default().build(),
            CPUExecutionProvider::default().build()
        ]).map_err(|e| anyhow::anyhow!("ORT Error during EP config: {:?}", e))?;

        let session = builder.commit_from_file(model_path)
            .with_context(|| format!("CRITICAL: Failed to load Harrier ONNX model from {}", model_path))?;
            
        info!("Successfully loaded Harrier ONNX model from {} with GPU/CPU fallback", model_path);
        
        Ok(Self {
            session: std::sync::Mutex::new(session),
            dim: 640,
        })
    }

    pub fn embed(&self, text: &str, is_query: bool, instruction: &str, tokenizer: &Tokenizer) -> Result<Vec<f32>> {
        let full_text = if is_query && !instruction.is_empty() {
            format!("{}{}", instruction, text)
        } else {
            text.to_string()
        };

        let tokens = tokenizer.encode(&full_text, true)?;
        let seq_len = tokens.len();
        
        if seq_len == 0 {
            return Ok(vec![0.0; self.dim]);
        }

        let input_ids: Vec<i64> = tokens.iter().map(|&t| t as i64).collect();
        let attention_mask: Vec<i64> = vec![1; seq_len];

        let shape = vec![1, seq_len];
        let input_ids_val = Value::from_array((shape.clone(), input_ids)).unwrap();
        let attention_mask_val = Value::from_array((shape, attention_mask)).unwrap();

        let mut inputs_map = std::collections::HashMap::new();
        inputs_map.insert(std::borrow::Cow::Borrowed("input_ids"), ort::session::SessionInputValue::from(input_ids_val));
        inputs_map.insert(std::borrow::Cow::Borrowed("attention_mask"), ort::session::SessionInputValue::from(attention_mask_val));

        let mut session_lock = self.session.lock().unwrap();
        let outputs = session_lock.run(inputs_map).map_err(|e| anyhow::anyhow!("ORT run error: {:?}", e))?;

        let tensor = outputs["last_hidden_state"].try_extract_tensor::<f32>()
            .map_err(|e| anyhow::anyhow!("ORT extraction error: {:?}", e))?;
            
        // Flattened view (shape, slice)
        let flat_slice: &[f32] = tensor.1;
        if flat_slice.len() < seq_len * self.dim {
            return Err(anyhow::anyhow!("Tensor slice too small"));
        }
        
        let mut pooled = vec![0.0f32; self.dim];
        for i in 0..seq_len {
            for d in 0..self.dim {
                pooled[d] += flat_slice[i * self.dim + d];
            }
        }
        
        for d in 0..self.dim {
            pooled[d] /= seq_len as f32;
        }

        let sum_sq: f32 = pooled.iter().map(|v| v * v).sum();
        let norm = sum_sq.sqrt();
        if norm > 0.0 {
            for v in pooled.iter_mut() {
                *v /= norm;
            }
        }

        Ok(pooled)
    }
}
