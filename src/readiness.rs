use std::time::Duration;

use tokio::sync::watch;
use tokio::time::{sleep, timeout};

use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct Readiness {
    pub sentinel: watch::Receiver<bool>,
    pub pihole: watch::Receiver<bool>,
    pub nsfw_text: watch::Receiver<bool>,
    pub nsfw_image: watch::Receiver<bool>,
    pub wolf: watch::Receiver<bool>,
    pub ocr: watch::Receiver<bool>,
    pub chunker: watch::Receiver<bool>,
}

impl Readiness {
    pub fn new() -> (Self, ReadinessHandles) {
        let (sentinel_tx, sentinel_rx) = watch::channel(false);
        let (pihole_tx, pihole_rx) = watch::channel(false);
        let (nsfw_text_tx, nsfw_text_rx) = watch::channel(false);
        let (nsfw_image_tx, nsfw_image_rx) = watch::channel(false);
        let (wolf_tx, wolf_rx) = watch::channel(false);
        let (ocr_tx, ocr_rx) = watch::channel(false);
        let (chunker_tx, chunker_rx) = watch::channel(false);
        (
            Self {
                sentinel: sentinel_rx,
                pihole: pihole_rx,
                nsfw_text: nsfw_text_rx,
                nsfw_image: nsfw_image_rx,
                wolf: wolf_rx,
                ocr: ocr_rx,
                chunker: chunker_rx,
            },
            ReadinessHandles {
                sentinel: sentinel_tx,
                pihole: pihole_tx,
                nsfw_text: nsfw_text_tx,
                nsfw_image: nsfw_image_tx,
                wolf: wolf_tx,
                ocr: ocr_tx,
                chunker: chunker_tx,
            },
        )
    }

    pub async fn wait_for(
        rx: &watch::Receiver<bool>,
        name: &str,
        max_wait: Duration,
    ) -> AppResult<()> {
        if *rx.borrow() {
            return Ok(());
        }
        let mut sub = rx.clone();
        timeout(max_wait, async {
            loop {
                if *sub.borrow_and_update() {
                    return Ok(());
                }
                sub.changed()
                    .await
                    .map_err(|_| AppError::Internal(format!("{name} readiness channel closed")))?;
            }
        })
        .await
        .map_err(|_| AppError::DependencyTimeout(name.to_string()))?
    }
}

pub struct ReadinessHandles {
    pub sentinel: watch::Sender<bool>,
    pub pihole: watch::Sender<bool>,
    pub nsfw_text: watch::Sender<bool>,
    pub nsfw_image: watch::Sender<bool>,
    pub wolf: watch::Sender<bool>,
    pub ocr: watch::Sender<bool>,
    pub chunker: watch::Sender<bool>,
}

pub fn spawn_until_ready<F, Fut>(tx: watch::Sender<bool>, interval: Duration, mut check: F)
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = bool> + Send,
{
    tokio::spawn(async move {
        loop {
            if check().await {
                let _ = tx.send(true);
                break;
            }
            sleep(interval).await;
        }
    });
}
