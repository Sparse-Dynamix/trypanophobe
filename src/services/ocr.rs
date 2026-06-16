use crate::config::Config;
use crate::services::sidecar;

pub async fn check_ready(cfg: &Config) -> bool {
    sidecar::health_ok(&cfg.ocr_health_url).await
}
