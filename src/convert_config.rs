use std::path::Path;

use anytomd::ConversionOptions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentKind {
    Image,
    TextDocument,
    PlainText,
}

#[derive(Debug, Clone)]
pub struct ConvertConfig {
    pub max_input_bytes: usize,
    pub max_uncompressed_zip_bytes: usize,
    pub max_total_image_bytes: usize,
}

impl ConvertConfig {
    pub fn from_limits(max_input: usize, max_zip: usize, max_image: usize) -> Self {
        Self {
            max_input_bytes: max_input,
            max_uncompressed_zip_bytes: max_zip,
            max_total_image_bytes: max_image,
        }
    }

    pub fn anytomd_options(&self) -> ConversionOptions {
        ConversionOptions {
            extract_images: false,
            extract_comments: false,
            max_total_image_bytes: self.max_total_image_bytes,
            strict: false,
            max_input_bytes: self.max_input_bytes,
            max_uncompressed_zip_bytes: self.max_uncompressed_zip_bytes,
            image_describer: None,
        }
    }
}

pub fn extension_from_filename(filename: &str) -> Result<String, String> {
    Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| e.to_ascii_lowercase())
        .ok_or_else(|| format!("no file extension in {filename:?}"))
}

pub fn hint_from_url_and_content_type(url: Option<&str>, content_type: Option<&str>) -> String {
    if let Some(ct) = content_type {
        let ct = ct
            .split(';')
            .next()
            .unwrap_or(ct)
            .trim()
            .to_ascii_lowercase();
        let ext = mime_to_ext(&ct);
        if !ext.is_empty() {
            return format!("page.{ext}");
        }
    }
    if let Some(url) = url {
        if let Ok(parsed) = url::Url::parse(url) {
            let path = parsed.path();
            if let Some(name) = Path::new(path).file_name().and_then(|s| s.to_str()) {
                if name.contains('.') {
                    return name.to_string();
                }
            }
        }
    }
    "page.html".into()
}

pub fn mime_to_ext(ct: &str) -> &'static str {
    match ct {
        "text/html" | "application/xhtml+xml" | "application/xhtml" => "html",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        "application/json" => "json",
        "text/csv" => "csv",
        "text/markdown" => "md",
        "application/xml" | "text/xml" => "xml",
        "application/rtf" => "rtf",
        "application/msword" => "doc",
        "application/vnd.ms-excel" => "xls",
        "application/vnd.ms-powerpoint" => "ppt",
        "image/png" => "png",
        "image/jpeg" | "image/jpg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        "image/svg+xml" => "svg",
        _ if ct.starts_with("application/vnd.openxmlformats-officedocument.") => {
            if ct.contains("wordprocessingml") {
                "docx"
            } else if ct.contains("spreadsheetml") {
                "xlsx"
            } else if ct.contains("presentationml") {
                "pptx"
            } else {
                ""
            }
        }
        _ => "",
    }
}

pub fn is_image_ext(ext: &str) -> bool {
    matches!(
        ext,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" | "tif" | "svg"
    )
}

pub fn is_liteparse_ext(ext: &str) -> bool {
    matches!(
        ext,
        "pdf"
            | "doc"
            | "docx"
            | "docm"
            | "odt"
            | "rtf"
            | "pages"
            | "ppt"
            | "pptx"
            | "pptm"
            | "odp"
            | "key"
            | "xls"
            | "xlsx"
            | "xlsm"
            | "ods"
            | "csv"
            | "tsv"
            | "numbers"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "bmp"
            | "tiff"
            | "tif"
            | "webp"
            | "svg"
    )
}

pub fn sniff_content_kind(hint: &str, data: &[u8]) -> ContentKind {
    if let Ok(ext) = extension_from_filename(hint) {
        if is_image_ext(&ext) {
            return ContentKind::Image;
        }
        if is_liteparse_ext(&ext) {
            return ContentKind::TextDocument;
        }
    }
    if data.starts_with(b"\x89PNG")
        || data.starts_with(b"\xff\xd8\xff")
        || data.starts_with(b"GIF8")
        || data.starts_with(b"RIFF")
        || data.starts_with(b"BM")
    {
        return ContentKind::Image;
    }
    if data.starts_with(b"%PDF") {
        return ContentKind::TextDocument;
    }
    ContentKind::PlainText
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniff_png() {
        assert_eq!(
            sniff_content_kind("x.png", b"\x89PNG\r\n\x1a\n"),
            ContentKind::Image
        );
    }
}
