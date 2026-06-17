pub mod convert;
pub mod moderate;
pub mod respond;
pub mod url_guard;

use std::sync::Arc;

use crate::convert_config::{hint_from_url_and_content_type, sniff_content_kind, ContentKind};
use crate::error::{AppError, AppResult};
use crate::pipeline::convert::to_markdown;
use crate::pipeline::moderate::{moderate_chunks, moderate_image};
use crate::pipeline::respond::{BlockedBody, FilterResponse, ResponseFormat};
use crate::pipeline::url_guard::UrlCheckOutcome;
use crate::services::chunker;
use crate::state::AppState;

#[derive(Debug, Clone)]
pub struct FilterRequest {
    pub body: Vec<u8>,
    pub url: String,
    pub content_type: Option<String>,
    pub response_format: ResponseFormat,
}

pub async fn run_filter(state: &Arc<AppState>, req: FilterRequest) -> AppResult<FilterResponse> {
    if req.body.len() > state.config.max_request_body_bytes {
        return Err(AppError::PayloadTooLarge(format!(
            "body exceeds {} bytes",
            state.config.max_request_body_bytes
        )));
    }

    state.wait_pihole().await?;
    match state.url_guard.check_url(&req.url).await? {
        UrlCheckOutcome::Allowed => {}
        UrlCheckOutcome::Blocked { reason, detail } => {
            let block = match reason.as_str() {
                "URL blocked by network policy" => {
                    BlockedBody::url_network_policy(detail.as_deref().unwrap_or("blocked"))
                }
                _ => BlockedBody::url_dns_blocklist(detail),
            };
            return Ok(FilterResponse::blocked(block));
        }
    }

    state.wait_ml().await?;

    let hint = hint_from_url_and_content_type(Some(&req.url), req.content_type.as_deref());
    let kind = sniff_content_kind(&hint, &req.body);

    let markdown = match kind {
        ContentKind::Image => {
            match moderate_image(state, &req.body).await {
                Err(AppError::Forbidden(msg)) => {
                    let label = msg
                        .strip_prefix("forbidden: nsfw_image_detected: ")
                        .or_else(|| msg.strip_prefix("nsfw_image_detected: "))
                        .unwrap_or(&msg);
                    return Ok(FilterResponse::blocked(BlockedBody::nsfw_image(label)));
                }
                other => other?,
            }
            convert::image_to_markdown(state, &hint, &req.body).await?
        }
        ContentKind::TextDocument => to_markdown(state, &hint, &req.body).await?,
        ContentKind::PlainText => {
            if req.response_format == ResponseFormat::Md
                || looks_like_html(&req.body)
                || extension_suggests_convert(&hint)
            {
                to_markdown(state, &hint, &req.body).await?
            } else {
                String::from_utf8_lossy(&req.body).into_owned()
            }
        }
    };

    let chunks = chunker::chunk_text(&state.config, &markdown).await?;
    let moderation = moderate_chunks(state, &chunks).await?;

    Ok(FilterResponse::from_chunks(
        &req.body,
        &markdown,
        &chunks,
        &moderation.safe_chunks,
        req.response_format,
        &moderation.flagged_models,
    ))
}

fn looks_like_html(data: &[u8]) -> bool {
    let prefix = String::from_utf8_lossy(&data[..data.len().min(512)]);
    let lower = prefix.trim_start().to_ascii_lowercase();
    lower.starts_with("<!doctype") || lower.starts_with("<html")
}

fn extension_suggests_convert(hint: &str) -> bool {
    hint.ends_with(".html")
        || hint.ends_with(".htm")
        || hint.ends_with(".json")
        || hint.ends_with(".xml")
}

impl FilterResponse {
    pub fn apply_to_response(&self, res: &mut salvo::http::Response) {
        res.status_code(self.status);
        if let Some(ct) = &self.content_type {
            res.headers_mut()
                .insert("Content-Type", ct.parse().expect("content-type"));
        }
        if !self.body.is_empty() {
            res.body(self.body.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::respond::{BlockStage, BlockedBody};

    #[test]
    fn forbidden_maps_to_nsfw_image_block() {
        let msg = "forbidden: nsfw_image_detected: nsfw";
        let label = msg
            .strip_prefix("forbidden: nsfw_image_detected: ")
            .unwrap();
        let block = BlockedBody::nsfw_image(label);
        assert_eq!(block.stage, BlockStage::NsfwImage);
        assert_eq!(block.detail.as_deref(), Some("nsfw"));
    }
}
