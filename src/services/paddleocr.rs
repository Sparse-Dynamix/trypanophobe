use std::time::Duration;

use reqwest::Client;

use crate::config::Config;

pub async fn check_ready(cfg: &Config) -> bool {
    let client = match Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let Ok(resp) = client.get(&cfg.paddleocr_health_url).send().await else {
        return false;
    };
    if !resp.status().is_success() {
        return false;
    }
    let Ok(body) = resp.json::<serde_json::Value>().await else {
        return false;
    };
    body.get("status")
        .and_then(|v| v.as_str())
        .is_some_and(|s| s == "healthy")
}
