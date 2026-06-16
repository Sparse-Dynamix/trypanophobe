use std::sync::Arc;

use crate::config::Config;
use crate::convert_config::ConvertConfig;
use crate::error::AppResult;
use crate::pipeline::url_guard::UrlGuard;
use crate::readiness::Readiness;
use crate::services::nsfw_image::NsfwImageClassifier;
use crate::services::nsfw_text::NsfwTextClassifier;
use crate::services::pihole::PiholeProbe;
use crate::services::sentinel::Sentinel;
use crate::services::wolf::WolfDefender;

pub struct AppState {
    pub config: Config,
    pub readiness: Arc<Readiness>,
    pub sentinel: Arc<Sentinel>,
    pub nsfw_text: Arc<NsfwTextClassifier>,
    pub nsfw_image: Arc<NsfwImageClassifier>,
    pub wolf: Arc<WolfDefender>,
    pub pihole: Arc<PiholeProbe>,
    pub url_guard: Arc<UrlGuard>,
    pub convert: ConvertConfig,
}

impl AppState {
    pub fn new(
        config: Config,
        readiness: Arc<Readiness>,
        sentinel: Arc<Sentinel>,
        nsfw_text: Arc<NsfwTextClassifier>,
        nsfw_image: Arc<NsfwImageClassifier>,
        wolf: Arc<WolfDefender>,
        pihole: Arc<PiholeProbe>,
        url_guard: Arc<UrlGuard>,
    ) -> Arc<Self> {
        Arc::new(Self {
            convert: ConvertConfig::from_limits(
                config.max_input_bytes,
                config.max_zip_bytes,
                config.max_image_bytes,
            ),
            config,
            readiness,
            sentinel,
            nsfw_text,
            nsfw_image,
            wolf,
            pihole,
            url_guard,
        })
    }

    pub async fn wait_ml(&self) -> AppResult<()> {
        Readiness::wait_for(
            &self.readiness.sentinel,
            "sentinel",
            self.config.readiness_wait,
        )
        .await?;
        Readiness::wait_for(
            &self.readiness.nsfw_text,
            "nsfw_text",
            self.config.readiness_wait,
        )
        .await?;
        Readiness::wait_for(
            &self.readiness.nsfw_image,
            "nsfw_image",
            self.config.readiness_wait,
        )
        .await?;
        Readiness::wait_for(&self.readiness.wolf, "wolf", self.config.readiness_wait).await?;
        Readiness::wait_for(&self.readiness.ocr, "ocr", self.config.readiness_wait).await?;
        Readiness::wait_for(&self.readiness.chunker, "chunker", self.config.readiness_wait)
            .await
    }

    pub async fn wait_pihole(&self) -> AppResult<()> {
        Readiness::wait_for(&self.readiness.pihole, "pihole", self.config.readiness_wait).await
    }
}
