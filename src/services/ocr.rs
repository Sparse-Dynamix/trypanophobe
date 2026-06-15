use std::sync::Arc;

use image::DynamicImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use rten::Model;

use crate::config::Config;
use crate::error::{AppError, AppResult};

pub struct OcrService {
    engine: OcrEngine,
}

impl OcrService {
    pub fn load(cfg: &Config) -> AppResult<Arc<Self>> {
        let dir = &cfg.ocrs_model_dir;
        let detection = dir.join("text-detection.rten");
        let recognition = dir.join("text-recognition.rten");
        let detection_model = Model::load_file(&detection)
            .map_err(|e| AppError::Internal(format!("ocrs detection model: {e}")))?;
        let recognition_model = Model::load_file(&recognition)
            .map_err(|e| AppError::Internal(format!("ocrs recognition model: {e}")))?;

        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detection_model),
            recognition_model: Some(recognition_model),
            ..Default::default()
        })
        .map_err(|e| AppError::Internal(format!("ocrs engine: {e}")))?;

        Ok(Arc::new(Self { engine }))
    }

    pub async fn warmup(self: &Arc<Self>) -> AppResult<()> {
        let img = DynamicImage::new_rgb8(16, 16);
        let _ = self.extract_text(&img)?;
        Ok(())
    }

    pub fn extract_text(self: &Arc<Self>, img: &DynamicImage) -> AppResult<String> {
        let rgb = img.to_rgb8();
        let (w, h) = rgb.dimensions();
        let bytes: Vec<u8> = rgb.into_raw();
        let source = ImageSource::from_bytes(&bytes, (w, h))
            .map_err(|e| AppError::Internal(format!("ocrs image source: {e}")))?;
        let input = self
            .engine
            .prepare_input(source)
            .map_err(|e| AppError::Internal(format!("ocrs prepare: {e}")))?;
        let text = self
            .engine
            .get_text(&input)
            .map_err(|e| AppError::Internal(format!("ocrs recognize: {e}")))?;
        Ok(text.trim().to_string())
    }
}
