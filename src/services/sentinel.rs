use std::num::NonZeroU32;
use std::path::Path;
use std::sync::Arc;

use llama_cpp_4::context::params::LlamaContextParams;
use llama_cpp_4::llama_backend::LlamaBackend;
use llama_cpp_4::llama_batch::LlamaBatch;
use llama_cpp_4::model::params::LlamaModelParams;
use llama_cpp_4::model::{AddBos, LlamaModel};
use llama_cpp_4::token::LlamaToken;
use ndarray::Array2;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::services::math::softmax_probs;

const TOKEN_OVERHEAD: usize = 16;
const MAX_DECODE_BATCH: u32 = 2_048;

#[derive(Debug, Clone)]
pub struct CheckResult {
    pub label: String,
    pub blocked: bool,
}

pub struct Sentinel {
    backend: Arc<LlamaBackend>,
    model: Arc<LlamaModel>,
    cls_head: Array2<f32>,
    max_input_tokens: usize,
    warmup_text: String,
    threshold: f32,
}

impl Sentinel {
    pub fn load(cfg: &Config) -> AppResult<Arc<Self>> {
        let backend =
            Arc::new(LlamaBackend::init().map_err(|e| AppError::Internal(e.to_string()))?);

        let mut model_params = LlamaModelParams::default();
        if cfg.sentinel_gpu_layers > 0 {
            model_params = model_params.with_n_gpu_layers(cfg.sentinel_gpu_layers);
        }

        let model = LlamaModel::load_from_file(&backend, &cfg.sentinel_model_path, &model_params)
            .map_err(|e| AppError::Internal(format!("load sentinel: {e}")))?;

        let max_input_tokens = cfg.sentinel_max_input_tokens;
        let cls_head = load_cls_head(&cfg.sentinel_cls_head_path)?;

        Ok(Arc::new(Self {
            backend,
            model: Arc::new(model),
            cls_head,
            max_input_tokens,
            warmup_text: cfg.sentinel_warmup_text.clone(),
            threshold: cfg.sentinel_threshold,
        }))
    }

    pub async fn warmup(self: &Arc<Self>) -> AppResult<()> {
        let _ = self.classify_text(&self.warmup_text).await?;
        Ok(())
    }

    pub async fn classify_text(self: &Arc<Self>, text: &str) -> AppResult<CheckResult> {
        if text.trim().is_empty() {
            return Ok(CheckResult {
                label: "benign".into(),
                blocked: false,
            });
        }

        let tokens = self
            .model
            .str_to_token(text, AddBos::Always)
            .map_err(|e| AppError::Internal(format!("tokenize: {e}")))?;

        let chunk: Vec<LlamaToken> = if tokens.len() > self.max_input_tokens {
            tokens[..self.max_input_tokens].to_vec()
        } else {
            tokens
        };

        let scores = self.classify_tokens(&chunk).await?;
        Ok(CheckResult {
            label: if scores.blocked(self.threshold) {
                "jailbreak".into()
            } else {
                "benign".into()
            },
            blocked: scores.blocked(self.threshold),
        })
    }

    async fn classify_tokens(&self, tokens: &[LlamaToken]) -> AppResult<ChunkScores> {
        let model = Arc::clone(&self.model);
        let backend = Arc::clone(&self.backend);
        let cls_head = self.cls_head.clone();
        let max_input_tokens = self.max_input_tokens;
        let tokens = tokens.to_vec();
        let n_ctx = context_size(tokens.len(), max_input_tokens);
        let batch = MAX_DECODE_BATCH.min(tokens.len() as u32).max(512);

        tokio::task::spawn_blocking(move || {
            let ctx_params = LlamaContextParams::default()
                .with_embeddings(true)
                .with_n_ctx(Some(n_ctx))
                .with_n_batch(batch)
                .with_n_ubatch(batch);
            let mut ctx = model
                .new_context(&backend, ctx_params)
                .map_err(|e| AppError::Internal(format!("context: {e}")))?;

            let mut batch = LlamaBatch::new(tokens.len(), 1);
            batch
                .add_sequence(&tokens, 0, false)
                .map_err(|e| AppError::Internal(format!("batch: {e}")))?;
            ctx.clear_kv_cache();
            ctx.decode(&mut batch)
                .map_err(|e| AppError::Internal(format!("decode: {e}")))?;

            let last = (tokens.len() - 1) as i32;
            let emb = ctx
                .embeddings_ith(last)
                .map_err(|e| AppError::Internal(format!("embed: {e}")))?;

            classify_embedding(emb, &cls_head)
        })
        .await
        .map_err(|e| AppError::Internal(format!("join: {e}")))?
    }
}

#[derive(Clone)]
struct ChunkScores {
    jailbreak_prob: f32,
}

impl ChunkScores {
    fn blocked(&self, threshold: f32) -> bool {
        self.jailbreak_prob >= threshold
    }
}

fn classify_embedding(emb: &[f32], cls_head: &Array2<f32>) -> AppResult<ChunkScores> {
    let n_embd = cls_head.ncols();
    let mut logits = [0.0f32; 2];
    for k in 0..2 {
        let mut sum = 0.0f32;
        for j in 0..n_embd.min(emb.len()) {
            sum += cls_head[[k, j]] * emb[j];
        }
        logits[k] = sum;
    }
    let probs = softmax_probs(&logits)?;
    Ok(ChunkScores {
        jailbreak_prob: probs[1],
    })
}

fn context_size(token_count: usize, max_input_tokens: usize) -> NonZeroU32 {
    let needed = token_count.saturating_add(TOKEN_OVERHEAD).max(512);
    let capped = needed.min(max_input_tokens.saturating_add(TOKEN_OVERHEAD));
    NonZeroU32::new(capped as u32).expect("context size must be non-zero")
}

fn load_cls_head(path: &Path) -> AppResult<Array2<f32>> {
    let data =
        std::fs::read(path).map_err(|e| AppError::Internal(format!("read cls head: {e}")))?;
    if data.len() < 8 {
        return Err(AppError::Internal("cls head file too small".into()));
    }
    let rows = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
    let cols = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    let expected = 8 + rows * cols * 4;
    if data.len() != expected {
        return Err(AppError::Internal(format!(
            "cls head size mismatch: expected {expected}, got {}",
            data.len()
        )));
    }
    let floats: Vec<f32> = data[8..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
        .collect();
    Array2::from_shape_vec((rows, cols), floats)
        .map_err(|e| AppError::Internal(format!("cls head shape: {e}")))
}
