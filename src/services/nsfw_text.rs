use std::sync::Arc;

use candle_core::{DType, Device, IndexOp, Tensor};
use candle_nn::{linear, Linear, Module, VarBuilder};
use candle_transformers::models::distilbert::{Config as DistilConfig, DistilBertModel};
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::services::preprocess::preprocess_for_nsfw_text;

#[derive(Debug, Clone)]
pub struct NsfwTextResult {
    pub label: String,
    pub blocked: bool,
}

pub struct NsfwTextClassifier {
    model: DistilBertModel,
    pre_classifier: Linear,
    classifier: Linear,
    tokenizer: Tokenizer,
    device: Device,
    threshold: f32,
}

impl NsfwTextClassifier {
    pub fn load(cfg: &Config) -> AppResult<Arc<Self>> {
        let device = Device::Cpu;
        let model_dir = &cfg.nsfw_text_model_dir;
        let weights = model_dir.join("model.safetensors");
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[weights], DType::F32, &device)
                .map_err(|e| AppError::Internal(format!("nsfw text weights: {e}")))?
        };

        let config_path = model_dir.join("config.json");
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| AppError::Internal(format!("nsfw text config: {e}")))?;
        let distil_cfg: DistilConfig = serde_json::from_str(&config_str)
            .map_err(|e| AppError::Internal(format!("nsfw text config parse: {e}")))?;
        let head_dims: DistilHeadDims = serde_json::from_str(&config_str)
            .map_err(|e| AppError::Internal(format!("nsfw text head dims: {e}")))?;

        let model = DistilBertModel::load(vb.pp("distilbert"), &distil_cfg)
            .map_err(|e| AppError::Internal(format!("nsfw text model: {e}")))?;
        let pre_classifier = linear(head_dims.dim, head_dims.dim, vb.pp("pre_classifier"))
            .map_err(|e| AppError::Internal(format!("nsfw text pre_classifier: {e}")))?;
        let classifier = linear(head_dims.dim, 2, vb.pp("classifier"))
            .map_err(|e| AppError::Internal(format!("nsfw text classifier: {e}")))?;

        let tokenizer_path = model_dir.join("tokenizer.json");
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| AppError::Internal(format!("nsfw text tokenizer: {e}")))?;

        Ok(Arc::new(Self {
            model,
            pre_classifier,
            classifier,
            tokenizer,
            device,
            threshold: cfg.nsfw_text_threshold,
        }))
    }

    pub async fn warmup(self: &Arc<Self>) -> AppResult<()> {
        let _ = self.classify("warmup check").await?;
        Ok(())
    }

    pub async fn classify(self: &Arc<Self>, text: &str) -> AppResult<NsfwTextResult> {
        let prepared = preprocess_for_nsfw_text(text);
        if prepared.is_empty() {
            return Ok(NsfwTextResult {
                label: "safe".into(),
                blocked: false,
            });
        }

        let encoding = self
            .tokenizer
            .encode(prepared.as_str(), true)
            .map_err(|e| AppError::Internal(format!("nsfw text tokenize: {e}")))?;
        let input_ids: Vec<u32> = encoding.get_ids().to_vec();
        let attention: Vec<u32> = encoding.get_attention_mask().to_vec();

        let input_ids = Tensor::new(vec![input_ids], &self.device)
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let attention_mask = Tensor::new(vec![attention], &self.device)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let hidden = self
            .model
            .forward(&input_ids, &attention_mask)
            .map_err(|e| AppError::Internal(format!("nsfw text forward: {e}")))?;
        let pooled = hidden
            .i((.., 0, ..))
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let pooled = self
            .pre_classifier
            .forward(&pooled)
            .map_err(|e| AppError::Internal(e.to_string()))?
            .gelu()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let logits = self
            .classifier
            .forward(&pooled)
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let logits = logits
            .to_vec2::<f32>()
            .map_err(|e| AppError::Internal(e.to_string()))?;
        let row = logits
            .first()
            .ok_or_else(|| AppError::Internal("empty logits".into()))?;
        let (safe, nsfw) = (row[0], row[1]);
        let probs = softmax2(safe, nsfw);
        let nsfw_score = probs.1 as f64;
        let blocked = nsfw_score >= self.threshold as f64;
        Ok(NsfwTextResult {
            label: if blocked { "nsfw" } else { "safe" }.into(),
            blocked,
        })
    }
}

#[derive(Debug, Deserialize)]
struct DistilHeadDims {
    dim: usize,
}

fn softmax2(a: f32, b: f32) -> (f32, f32) {
    let m = a.max(b);
    let ea = (a - m).exp();
    let eb = (b - m).exp();
    let s = ea + eb;
    (ea / s, eb / s)
}
