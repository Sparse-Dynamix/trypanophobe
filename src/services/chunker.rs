use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::pipeline::chunk::MarkdownChunk;

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
    let client = match Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let Ok(resp) = client.get(&cfg.chunker_health_url).send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    body.get("status")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "healthy")
}

pub async fn chunk_text(cfg: &Config, markdown: &str) -> AppResult<Vec<MarkdownChunk>> {
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
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
