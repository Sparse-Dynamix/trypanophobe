use std::sync::{Arc, Mutex};

use image::DynamicImage;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;
use ort::value::Tensor;

use crate::config::Config;
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct NsfwImageResult {
    pub label: String,
    pub blocked: bool,
}

pub struct NsfwImageClassifier {
    session: Mutex<Session>,
    image_size: u32,
    threshold: f32,
}

impl NsfwImageClassifier {
    pub fn load(cfg: &Config) -> AppResult<Arc<Self>> {
        let model_dir = &cfg.nsfw_image_model_dir;
        let onnx_path = model_dir.join("model.onnx");
        if !onnx_path.exists() {
            return Err(AppError::Internal(format!(
                "nsfw image onnx missing at {}",
                onnx_path.display()
            )));
        }

        let session = Session::builder()
            .map_err(|e| AppError::Internal(format!("nsfw image ort builder: {e}")))?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|e| AppError::Internal(format!("nsfw image ort opt: {e}")))?
            .commit_from_file(&onnx_path)
            .map_err(|e| AppError::Internal(format!("nsfw image ort load: {e}")))?;

        Ok(Arc::new(Self {
            session: Mutex::new(session),
            image_size: 384,
            threshold: cfg.nsfw_image_threshold,
        }))
    }

    pub async fn warmup(self: &Arc<Self>) -> AppResult<()> {
        let img = DynamicImage::new_rgb8(8, 8);
        let _ = self.classify(&img)?;
        Ok(())
    }

    pub fn classify(self: &Arc<Self>, img: &DynamicImage) -> AppResult<NsfwImageResult> {
        let rgb = img.to_rgb8();
        let resized = image::imageops::resize(
            &rgb,
            self.image_size,
            self.image_size,
            image::imageops::FilterType::Triangle,
        );

        let mut data = Vec::with_capacity((3 * self.image_size * self.image_size) as usize);
        for c in 0..3usize {
            for y in 0..self.image_size {
                for x in 0..self.image_size {
                    let p = resized.get_pixel(x, y);
                    let v = [p[0], p[1], p[2]][c] as f32 / 255.0;
                    data.push((v - 0.5) / 0.5);
                }
            }
        }

        let pixel_values = Tensor::from_array((
            [
                1usize,
                3,
                self.image_size as usize,
                self.image_size as usize,
            ],
            data,
        ))
        .map_err(|e| AppError::Internal(format!("nsfw image tensor: {e}")))?;

        let mut session = self
            .session
            .lock()
            .map_err(|e| AppError::Internal(format!("nsfw image session lock: {e}")))?;

        let outputs = session
            .run(ort::inputs!["pixel_values" => pixel_values])
            .map_err(|e| AppError::Internal(format!("nsfw image inference: {e}")))?;

        let (_shape, logits) = outputs[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| AppError::Internal(format!("nsfw image logits: {e}")))?;

        let (nsfw, sfw) = if logits.len() >= 2 {
            (logits[0], logits[1])
        } else {
            return Err(AppError::Internal("nsfw image empty logits".into()));
        };

        let probs = softmax2(nsfw, sfw);
        let nsfw_score = probs.0 as f64;
        let blocked = nsfw_score >= self.threshold as f64;
        Ok(NsfwImageResult {
            label: if blocked { "nsfw" } else { "sfw" }.into(),
            blocked,
        })
    }
}

fn softmax2(a: f32, b: f32) -> (f32, f32) {
    let m = a.max(b);
    let ea = (a - m).exp();
    let eb = (b - m).exp();
    let s = ea + eb;
    (ea / s, eb / s)
}
