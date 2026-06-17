use salvo::http::StatusCode;
use salvo::oapi::ToSchema;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::services::chunker::MarkdownChunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    Og,
    Md,
}

impl ResponseFormat {
    pub fn from_query(format_param: Option<&str>) -> AppResult<Self> {
        match format_param.unwrap_or("og") {
            "og" => Ok(Self::Og),
            "md" => Ok(Self::Md),
            other => Err(AppError::BadRequest(format!(
                "invalid format={other:?}; expected md or og"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum BlockStage {
    UrlCheck,
    NsfwImage,
    ChunkModeration,
    ResponseFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct BlockedBody {
    pub error: String,
    pub stage: BlockStage,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl BlockedBody {
    pub const ERROR: &str = "content_blocked";

    pub fn url_network_policy(detail: &str) -> Self {
        Self {
            error: Self::ERROR.into(),
            stage: BlockStage::UrlCheck,
            reason: "URL blocked by network policy".into(),
            detail: Some(detail.into()),
        }
    }

    pub fn url_dns_blocklist(detail: Option<String>) -> Self {
        Self {
            error: Self::ERROR.into(),
            stage: BlockStage::UrlCheck,
            reason: "URL blocked by DNS blocklist".into(),
            detail,
        }
    }

    pub fn nsfw_image(detail: &str) -> Self {
        Self {
            error: Self::ERROR.into(),
            stage: BlockStage::NsfwImage,
            reason: "NSFW image detected".into(),
            detail: Some(detail.into()),
        }
    }

    pub fn chunk_moderation(models: &[String]) -> Self {
        Self {
            error: Self::ERROR.into(),
            stage: BlockStage::ChunkModeration,
            reason: "All content chunks flagged".into(),
            detail: if models.is_empty() {
                None
            } else {
                Some(models.join(", "))
            },
        }
    }

    pub fn response_format(safe_count: usize, total: usize) -> Self {
        Self {
            error: Self::ERROR.into(),
            stage: BlockStage::ResponseFormat,
            reason: "Partial content blocked for format=og".into(),
            detail: Some(format!("{safe_count}/{total} chunks safe")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct FilterResponse {
    pub status: StatusCode,
    pub body: Vec<u8>,
    pub content_type: Option<String>,
}

impl FilterResponse {
    pub fn blocked(block: BlockedBody) -> Self {
        let body = serde_json::to_vec(&block).expect("blocked body serializes");
        Self {
            status: StatusCode::NOT_ACCEPTABLE,
            body,
            content_type: Some("application/json".to_string()),
        }
    }

    pub fn from_chunks(
        original: &[u8],
        full_markdown: &str,
        all_chunks: &[MarkdownChunk],
        safe_chunks: &[MarkdownChunk],
        format: ResponseFormat,
        flagged_models: &[String],
    ) -> Self {
        let total = all_chunks.len();
        let safe_count = safe_chunks.len();

        if safe_count == 0 {
            return Self::blocked(BlockedBody::chunk_moderation(flagged_models));
        }

        if safe_count < total && format == ResponseFormat::Og {
            return Self::blocked(BlockedBody::response_format(safe_count, total));
        }

        let status = if safe_count == total {
            StatusCode::OK
        } else {
            StatusCode::PARTIAL_CONTENT
        };

        let (body, content_type) = match format {
            ResponseFormat::Md => {
                let joined = safe_chunks
                    .iter()
                    .map(|c| c.text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                let md = if joined.is_empty() {
                    full_markdown.to_string()
                } else {
                    joined
                };
                (md.into_bytes(), Some("text/markdown".to_string()))
            }
            ResponseFormat::Og => (original.to_vec(), None),
        };

        Self {
            status,
            body,
            content_type,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_blocked(body: &[u8]) -> BlockedBody {
        serde_json::from_slice(body).expect("blocked json")
    }

    #[test]
    fn all_safe_is_200() {
        let chunks = vec![
            MarkdownChunk {
                index: 0,
                text: "a".into(),
            },
            MarkdownChunk {
                index: 1,
                text: "b".into(),
            },
        ];
        let resp = FilterResponse::from_chunks(
            b"orig",
            "a\n\nb",
            &chunks,
            &chunks,
            ResponseFormat::Og,
            &[],
        );
        assert_eq!(resp.status, StatusCode::OK);
        assert_eq!(resp.body, b"orig");
    }

    #[test]
    fn partial_md_is_206() {
        let all = vec![
            MarkdownChunk {
                index: 0,
                text: "safe".into(),
            },
            MarkdownChunk {
                index: 1,
                text: "bad".into(),
            },
        ];
        let safe = vec![all[0].clone()];
        let resp = FilterResponse::from_chunks(
            b"orig",
            "safe\n\nbad",
            &all,
            &safe,
            ResponseFormat::Md,
            &[],
        );
        assert_eq!(resp.status, StatusCode::PARTIAL_CONTENT);
        assert!(resp.body.starts_with(b"safe"));
        assert_eq!(resp.content_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn partial_og_is_406() {
        let all = vec![
            MarkdownChunk {
                index: 0,
                text: "safe".into(),
            },
            MarkdownChunk {
                index: 1,
                text: "bad".into(),
            },
        ];
        let safe = vec![all[0].clone()];
        let resp = FilterResponse::from_chunks(
            b"orig",
            "safe\n\nbad",
            &all,
            &safe,
            ResponseFormat::Og,
            &[],
        );
        assert_eq!(resp.status, StatusCode::NOT_ACCEPTABLE);
        let block = parse_blocked(&resp.body);
        assert_eq!(block.stage, BlockStage::ResponseFormat);
        assert_eq!(block.detail.as_deref(), Some("1/2 chunks safe"));
    }

    #[test]
    fn none_safe_is_406() {
        let all = vec![MarkdownChunk {
            index: 0,
            text: "bad".into(),
        }];
        let resp = FilterResponse::from_chunks(
            b"x",
            "bad",
            &all,
            &[],
            ResponseFormat::Og,
            &["sentinel".into(), "wolf".into()],
        );
        assert_eq!(resp.status, StatusCode::NOT_ACCEPTABLE);
        let block = parse_blocked(&resp.body);
        assert_eq!(block.stage, BlockStage::ChunkModeration);
        assert_eq!(block.detail.as_deref(), Some("sentinel, wolf"));
    }

    #[test]
    fn blocked_body_serializes_stages() {
        let cases = [
            (
                BlockedBody::url_network_policy("blocked_address"),
                BlockStage::UrlCheck,
            ),
            (BlockedBody::nsfw_image("nsfw"), BlockStage::NsfwImage),
            (
                BlockedBody::chunk_moderation(&["wolf".into()]),
                BlockStage::ChunkModeration,
            ),
            (
                BlockedBody::response_format(1, 3),
                BlockStage::ResponseFormat,
            ),
        ];
        for (body, stage) in cases {
            let json = serde_json::to_value(&body).unwrap();
            assert_eq!(json["error"], "content_blocked");
            assert_eq!(json["stage"], serde_json::to_value(stage).unwrap());
            assert!(json["reason"].is_string());
        }
    }

    #[test]
    fn from_query_defaults_og() {
        assert_eq!(
            ResponseFormat::from_query(None).unwrap(),
            ResponseFormat::Og
        );
        assert_eq!(
            ResponseFormat::from_query(Some("md")).unwrap(),
            ResponseFormat::Md
        );
        assert!(ResponseFormat::from_query(Some("markdown")).is_err());
    }
}
