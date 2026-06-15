use regex::Regex;
use std::sync::LazyLock;

#[derive(Debug, Clone)]
pub struct MarkdownChunk {
    pub index: usize,
    pub text: String,
}

static HEADING_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^#{1,6}\s").expect("heading regex"));

pub fn chunk_markdown(markdown: &str, max_chunks: usize, max_chars: usize) -> Vec<MarkdownChunk> {
    let trimmed = markdown.trim();
    if trimmed.is_empty() {
        return vec![MarkdownChunk {
            index: 0,
            text: String::new(),
        }];
    }

    let positions: Vec<usize> = HEADING_RE.find_iter(trimmed).map(|m| m.start()).collect();

    let mut raw_chunks: Vec<String> = if positions.is_empty() {
        split_by_size(trimmed, max_chars)
    } else {
        let mut chunks = Vec::new();
        if positions[0] > 0 {
            chunks.push(trimmed[..positions[0]].trim().to_string());
        }
        for (i, &start) in positions.iter().enumerate() {
            let end = positions.get(i + 1).copied().unwrap_or(trimmed.len());
            let slice = trimmed[start..end].trim();
            if !slice.is_empty() {
                chunks.push(slice.to_string());
            }
        }
        chunks
    };

    if raw_chunks.is_empty() {
        raw_chunks.push(trimmed.to_string());
    }

    let mut out = Vec::new();
    for (index, text) in raw_chunks.into_iter().take(max_chunks).enumerate() {
        let text = if text.len() > max_chars {
            text[..max_chars].to_string()
        } else {
            text
        };
        if !text.is_empty() {
            out.push(MarkdownChunk { index, text });
        }
    }

    if out.is_empty() {
        out.push(MarkdownChunk {
            index: 0,
            text: trimmed[..trimmed.len().min(max_chars)].to_string(),
        });
    }

    out
}

fn split_by_size(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    text.char_indices()
        .step_by(max_chars)
        .map(|(i, _)| {
            let end = (i + max_chars).min(text.len());
            text[i..end].to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_headings() {
        let md = "# Title\n\nintro\n\n## Section\n\nbody";
        let chunks = chunk_markdown(md, 32, 10000);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.contains("Title"));
        assert!(chunks.iter().any(|c| c.text.contains("Section")));
    }

    #[test]
    fn single_chunk_without_headings() {
        let chunks = chunk_markdown("hello world", 32, 10000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello world");
    }
}
