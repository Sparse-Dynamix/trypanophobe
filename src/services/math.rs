use candle_core::{Device, Tensor};
use candle_nn::ops::softmax;

use crate::error::{AppError, AppResult};

/// Softmax over the last dimension of a 1×N logit row on CPU.
pub fn softmax_probs(logits: &[f32]) -> AppResult<Vec<f32>> {
    if logits.is_empty() {
        return Err(AppError::Internal("empty logits".into()));
    }
    let row: Vec<f32> = logits.to_vec();
    let tensor = Tensor::new(row.as_slice(), &Device::Cpu)
        .and_then(|t| t.reshape((1, logits.len())))
        .map_err(|e| AppError::Internal(format!("softmax tensor: {e}")))?;
    let probs = softmax(&tensor, 1).map_err(|e| AppError::Internal(format!("softmax: {e}")))?;
    probs
        .to_vec2::<f32>()
        .map_err(|e| AppError::Internal(format!("softmax vec: {e}")))?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Internal("softmax empty output".into()))
}
