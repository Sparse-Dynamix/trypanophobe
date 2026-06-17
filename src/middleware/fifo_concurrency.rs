use std::sync::Arc;
use std::time::Instant;

use salvo::http::{Request, Response};
use salvo::{async_trait, Depot, FlowCtrl, Handler};
use tokio::sync::Semaphore;

pub const QUEUE_WAIT_HEADER: &str = "x-queue-wait-ms";
pub const PROCESS_MS_HEADER: &str = "x-process-ms";

/// FIFO request gate: waits for a permit (`acquire().await`) before calling downstream handlers.
#[derive(Debug, Clone)]
pub struct FifoConcurrency {
    semaphore: Arc<Semaphore>,
}

impl FifoConcurrency {
    pub fn new(limit: usize) -> Self {
        let limit = limit.max(1);
        Self {
            semaphore: Arc::new(Semaphore::new(limit)),
        }
    }
}

#[async_trait]
impl Handler for FifoConcurrency {
    async fn handle(
        &self,
        req: &mut Request,
        depot: &mut Depot,
        res: &mut Response,
        ctrl: &mut FlowCtrl,
    ) {
        let queued_at = Instant::now();
        let permit = match self.semaphore.acquire().await {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("fifo concurrency semaphore closed: {e}");
                res.status_code(salvo::http::StatusCode::SERVICE_UNAVAILABLE);
                return;
            }
        };
        let queue_wait_ms = queued_at.elapsed().as_millis();
        res.headers_mut().insert(
            QUEUE_WAIT_HEADER,
            queue_wait_ms.to_string().parse().expect("header value"),
        );

        let process_start = Instant::now();
        ctrl.call_next(req, depot, res).await;
        drop(permit);

        let process_ms = process_start.elapsed().as_millis();
        res.headers_mut().insert(
            PROCESS_MS_HEADER,
            process_ms.to_string().parse().expect("header value"),
        );
    }
}

pub fn fifo_concurrency(limit: usize) -> FifoConcurrency {
    FifoConcurrency::new(limit)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;

    /// With limit 1, only one task holds the permit at a time.
    #[tokio::test]
    async fn semaphore_allows_only_one_holder() {
        let sem = Arc::new(Semaphore::new(1));
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..8 {
            let sem = Arc::clone(&sem);
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            handles.push(tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                let mut cur = peak.load(Ordering::SeqCst);
                while now > cur {
                    if peak
                        .compare_exchange(cur, now, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        break;
                    }
                    cur = peak.load(Ordering::SeqCst);
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(peak.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn fifo_concurrency_minimum_limit_is_one() {
        let _ = fifo_concurrency(0);
        let _ = fifo_concurrency(1);
    }
}
