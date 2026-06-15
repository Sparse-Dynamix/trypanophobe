use std::sync::Arc;

use liteparse::{LiteParse, LiteParseConfig};

use crate::convert_config::{extension_from_filename, is_liteparse_ext, ConvertConfig};
use crate::error::{AppError, AppResult};
use crate::state::AppState;

pub async fn to_markdown(state: &Arc<AppState>, hint: &str, data: &[u8]) -> AppResult<String> {
    if data.len() > state.convert.max_input_bytes {
        return Err(AppError::PayloadTooLarge(format!(
            "input exceeds {} bytes",
            state.convert.max_input_bytes
        )));
    }

    let ext = extension_from_filename(hint).map_err(AppError::Unprocessable)?;

    if is_liteparse_ext(&ext) {
        return liteparse_to_markdown(&ext, data).await;
    }

    anytomd_convert(&state.convert, data, &ext)
}

pub async fn image_to_markdown(
    state: &Arc<AppState>,
    hint: &str,
    data: &[u8],
) -> AppResult<String> {
    let img = crate::services::decode_image(data)?;
    let text = state.ocr.extract_text(&img)?;
    if !text.is_empty() {
        return Ok(text);
    }

    let ext = extension_from_filename(hint).unwrap_or_else(|_| "png".into());
    liteparse_to_markdown(&ext, data).await
}

async fn liteparse_to_markdown(ext: &str, data: &[u8]) -> AppResult<String> {
    let dir = tempfile::tempdir().map_err(|e| AppError::Internal(e.to_string()))?;
    let path = dir.path().join(format!("input.{ext}"));
    std::fs::write(&path, data).map_err(|e| AppError::Internal(e.to_string()))?;

    let config = LiteParseConfig::default();
    let parser = LiteParse::new(config);
    let result = parser
        .parse(path.to_string_lossy().as_ref())
        .await
        .map_err(|e| AppError::Unprocessable(format!("liteparse: {e}")))?;

    let text = result.text.trim();
    if text.is_empty() {
        return Err(AppError::Unprocessable(
            "liteparse produced empty text".into(),
        ));
    }
    Ok(text.to_string())
}

fn anytomd_convert(cfg: &ConvertConfig, data: &[u8], ext: &str) -> AppResult<String> {
    let options = cfg.anytomd_options();
    anytomd::convert_bytes(data, ext, &options)
        .map(|r| r.markdown)
        .map_err(|e| AppError::Unprocessable(format!("anytomd: {e}")))
}
