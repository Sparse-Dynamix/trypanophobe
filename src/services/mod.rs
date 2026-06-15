pub mod nsfw_image;
pub mod nsfw_text;
pub mod ocr;
pub mod pihole;
pub mod preprocess;
pub mod sentinel;
pub mod wolf;

pub use nsfw_image::NsfwImageClassifier;
pub use nsfw_text::NsfwTextClassifier;
pub use ocr::OcrService;
pub use pihole::PiholeProbe;
pub use sentinel::Sentinel;
pub use wolf::WolfDefender;

use crate::error::{AppError, AppResult};
use image::DynamicImage;

pub fn decode_image(bytes: &[u8]) -> AppResult<DynamicImage> {
    image::load_from_memory(bytes)
        .map_err(|e| AppError::Unprocessable(format!("image_decode: {e}")))
}

pub fn resize_for_pipeline(img: &DynamicImage) -> DynamicImage {
    const MAX_W: u32 = 384;
    const MAX_H: u32 = 384;
    let (w, h) = (img.width(), img.height());
    if w <= MAX_W && h <= MAX_H {
        return img.clone();
    }
    img.thumbnail(MAX_W, MAX_H)
}
