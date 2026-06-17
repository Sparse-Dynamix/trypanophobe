use std::sync::Arc;

use liteparse::{LiteParse, LiteParseConfig};

use crate::config::Config;
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
        return liteparse_to_markdown(state, &ext, data).await;
    }

    anytomd_convert(&state.convert, data, &ext)
}

pub async fn image_to_markdown(
    state: &Arc<AppState>,
    hint: &str,
    data: &[u8],
) -> AppResult<String> {
    let ext = extension_from_filename(hint).unwrap_or_else(|_| "png".into());
    liteparse_to_markdown(state, &ext, data).await
}

fn liteparse_config(cfg: &Config, ext: &str) -> LiteParseConfig {
    // Office docs are converted by LibreOffice with extractable text; OCR is for
    // scanned PDFs and image-only inputs (see liteparse sparse-page handling).
    let ocr_enabled = matches!(
        ext,
        "pdf" | "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "webp" | "svg"
    );
    LiteParseConfig {
        ocr_enabled,
        ocr_language: "eng".into(),
        ocr_server_url: Some(cfg.ocr_url.clone()),
        dpi: 96.0,
        num_workers: 1,
        ..Default::default()
    }
}

async fn liteparse_to_markdown(state: &Arc<AppState>, ext: &str, data: &[u8]) -> AppResult<String> {
    let dir = tempfile::tempdir().map_err(|e| AppError::Internal(e.to_string()))?;
    let path = dir.path().join(format!("input.{ext}"));
    std::fs::write(&path, data).map_err(|e| AppError::Internal(e.to_string()))?;

    let config = liteparse_config(&state.config, ext);
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

#[cfg(test)]
mod tests {
    use std::process::Command;

    use liteparse::{LiteParse, LiteParseConfig};

    fn soffice_available() -> bool {
        Command::new("soffice")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    #[tokio::test]
    async fn single_paragraph_docx_extracts_text() {
        if !soffice_available() {
            return;
        }

        let data = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/single-paragraph.docx"
        ));
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("single-paragraph.docx");
        std::fs::write(&path, data).expect("write fixture");

        let parser = LiteParse::new(LiteParseConfig {
            ocr_enabled: false,
            ..Default::default()
        });
        let result = parser
            .parse(path.to_string_lossy().as_ref())
            .await
            .expect("liteparse docx");
        assert!(
            result.text.contains("Walking on imported air"),
            "unexpected text: {}",
            result.text
        );
    }
}
