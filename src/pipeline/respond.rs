use salvo::http::StatusCode;

use crate::pipeline::chunk::MarkdownChunk;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseFormat {
    Original,
    Markdown,
}

impl ResponseFormat {
    pub fn from_query(markdown_param: bool, format_param: Option<&str>) -> Self {
        if markdown_param || format_param == Some("markdown") {
            Self::Markdown
        } else {
            Self::Original
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
    pub fn blocked() -> Self {
        Self {
            status: StatusCode::NOT_ACCEPTABLE,
            body: Vec::new(),
            content_type: None,
        }
    }

    pub fn from_chunks(
        original: &[u8],
        full_markdown: &str,
        all_chunks: &[MarkdownChunk],
        safe_chunks: &[MarkdownChunk],
        format: ResponseFormat,
    ) -> Self {
        let total = all_chunks.len();
        let safe_count = safe_chunks.len();

        if safe_count == 0 {
            return Self::blocked();
        }

        let status = if safe_count == total {
            StatusCode::OK
        } else {
            StatusCode::PARTIAL_CONTENT
        };

        let (body, content_type) = match format {
            ResponseFormat::Markdown => {
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
            ResponseFormat::Original => (original.to_vec(), None),
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
            ResponseFormat::Original,
        );
        assert_eq!(resp.status, StatusCode::OK);
        assert_eq!(resp.body, b"orig");
    }

    #[test]
    fn partial_is_206() {
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
            ResponseFormat::Markdown,
        );
        assert_eq!(resp.status, StatusCode::PARTIAL_CONTENT);
        assert!(resp.body.starts_with(b"safe"));
        assert_eq!(resp.content_type.as_deref(), Some("text/markdown"));
    }

    #[test]
    fn none_safe_is_406() {
        let all = vec![MarkdownChunk {
            index: 0,
            text: "bad".into(),
        }];
        let resp = FilterResponse::from_chunks(b"x", "bad", &all, &[], ResponseFormat::Original);
        assert_eq!(resp.status, StatusCode::NOT_ACCEPTABLE);
    }
}
