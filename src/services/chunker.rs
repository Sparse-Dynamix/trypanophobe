use serde::Deserialize;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::services::sidecar;

#[derive(Debug, Clone)]
pub struct MarkdownChunk {
    pub index: usize,
    pub text: String,
}

#[derive(Debug, Deserialize)]
struct ChunkResponse {
    chunks: Vec<RemoteChunk>,
}

#[derive(Debug, Deserialize)]
struct RemoteChunk {
    index: usize,
    text: String,
    #[serde(rename = "token_count")]
    _token_count: usize,
}

pub async fn check_ready(cfg: &Config) -> bool {
    sidecar::health_ok(&cfg.chunker_health_url).await
}

pub async fn chunk_text(cfg: &Config, markdown: &str) -> AppResult<Vec<MarkdownChunk>> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let body = serde_json::json!({
        "text": markdown,
        "max_tokens": cfg.chunk_max_tokens,
    });

    let resp = client
        .post(&cfg.chunker_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("chunker request: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let detail = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!(
            "chunker returned {status}: {detail}"
        )));
    }

    let parsed: ChunkResponse = resp
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("chunker response: {e}")))?;

    Ok(parsed
        .chunks
        .into_iter()
        .map(|c| MarkdownChunk {
            index: c.index,
            text: c.text,
        })
        .collect())
}
