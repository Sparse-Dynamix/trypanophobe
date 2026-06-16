use std::sync::Arc;

use crate::error::AppResult;
use crate::services::chunker::MarkdownChunk;
use crate::services::{decode_image, resize_for_pipeline};
use crate::state::AppState;

pub async fn moderate_image(state: &Arc<AppState>, bytes: &[u8]) -> AppResult<()> {
    let img = decode_image(bytes)?;
    let pipeline = resize_for_pipeline(&img);
    let result = state.nsfw_image.classify(&pipeline)?;
    if result.blocked {
        tracing::debug!(label = %result.label, "nsfw image blocked");
        return Err(crate::error::AppError::Forbidden(format!(
            "nsfw_image_detected: {}",
            result.label
        )));
    }
    Ok(())
}

pub async fn moderate_chunks(
    state: &Arc<AppState>,
    chunks: &[MarkdownChunk],
) -> AppResult<Vec<MarkdownChunk>> {
    let mut safe = Vec::new();
    for chunk in chunks {
        if chunk.text.trim().is_empty() {
            safe.push(chunk.clone());
            continue;
        }
        if moderate_chunk(state, &chunk.text).await? {
            safe.push(chunk.clone());
        } else {
            tracing::debug!(index = chunk.index, "chunk removed by moderation");
        }
    }
    Ok(safe)
}

/// Returns true if chunk is safe (keep), false if flagged (remove).
async fn moderate_chunk(state: &Arc<AppState>, text: &str) -> AppResult<bool> {
    let sentinel = Arc::clone(&state.sentinel);
    let nsfw = Arc::clone(&state.nsfw_text);
    let wolf = Arc::clone(&state.wolf);
    let text_s = text.to_string();
    let text_n = text_s.clone();
    let text_w = text_s.clone();

    let (sentinel_r, nsfw_r, wolf_r) = tokio::join!(
        async move { sentinel.classify_text(&text_s).await },
        async move { nsfw.classify(&text_n).await },
        async move { wolf.classify(&text_w) }
    );

    let sentinel = sentinel_r?;
    let nsfw = nsfw_r?;
    let wolf = wolf_r?;

    let flagged = sentinel.blocked || nsfw.blocked || wolf.blocked;
    Ok(!flagged)
}
