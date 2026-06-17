use std::collections::BTreeSet;
use std::sync::Arc;

use crate::error::AppResult;
use crate::services::chunker::MarkdownChunk;
use crate::services::{decode_image, resize_for_pipeline};
use crate::state::AppState;

pub struct ModerationOutcome {
    pub safe_chunks: Vec<MarkdownChunk>,
    pub flagged_models: Vec<String>,
}

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
) -> AppResult<ModerationOutcome> {
    let mut safe = Vec::new();
    let mut flagged_models = BTreeSet::new();
    for chunk in chunks {
        if chunk.text.trim().is_empty() {
            safe.push(chunk.clone());
            continue;
        }
        let verdict = moderate_chunk(state, &chunk.text).await?;
        if verdict.safe {
            safe.push(chunk.clone());
        } else {
            tracing::debug!(index = chunk.index, "chunk removed by moderation");
            flagged_models.extend(verdict.flagged_by.into_iter().map(str::to_string));
        }
    }
    Ok(ModerationOutcome {
        safe_chunks: safe,
        flagged_models: flagged_models.into_iter().collect(),
    })
}

struct ChunkVerdict {
    safe: bool,
    flagged_by: Vec<&'static str>,
}

pub fn flagged_models(sentinel: bool, nsfw: bool, wolf: bool) -> Vec<&'static str> {
    let mut models = Vec::new();
    if sentinel {
        models.push("sentinel");
    }
    if nsfw {
        models.push("nsfw_text");
    }
    if wolf {
        models.push("wolf");
    }
    models
}

/// Returns verdict for chunk moderation.
async fn moderate_chunk(state: &Arc<AppState>, text: &str) -> AppResult<ChunkVerdict> {
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

    let flagged_by = flagged_models(sentinel.blocked, nsfw.blocked, wolf.blocked);
    Ok(ChunkVerdict {
        safe: flagged_by.is_empty(),
        flagged_by,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flagged_models_lists_blockers() {
        assert!(flagged_models(false, false, false).is_empty());
        assert_eq!(flagged_models(true, false, true), vec!["sentinel", "wolf"]);
        assert_eq!(flagged_models(false, true, false), vec!["nsfw_text"]);
        assert_eq!(
            flagged_models(true, true, true),
            vec!["sentinel", "nsfw_text", "wolf"]
        );
    }
}
