use std::time::Duration;

use reqwest::Client;

pub async fn health_ok(url: &str) -> bool {
    let client = match Client::builder().timeout(Duration::from_secs(3)).build() {
        Ok(c) => c,
        Err(_) => return false,
    };
    let Ok(resp) = client.get(url).send().await else {
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
