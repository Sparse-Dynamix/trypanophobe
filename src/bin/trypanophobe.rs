use std::sync::Arc;

use trypanophobe::config::Config;
use trypanophobe::middleware::fifo_concurrency;
use trypanophobe::pipeline::url_guard::UrlGuard;
use trypanophobe::readiness::{spawn_until_ready, Readiness};
use trypanophobe::routes::{filter, health};
use trypanophobe::services::{
    chunker, ocr, NsfwImageClassifier, NsfwTextClassifier, PiholeProbe, Sentinel, WolfDefender,
};
use trypanophobe::state::AppState;

use salvo::affix_state;
use salvo::cors::{Any, Cors};
use salvo::http::request::set_global_secure_max_size;
use salvo::prelude::*;
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Config::from_env();
    set_global_secure_max_size(cfg.max_request_body_bytes);

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let sentinel = Sentinel::load(&cfg)?;
    let nsfw_text = NsfwTextClassifier::load(&cfg)?;
    let nsfw_image = NsfwImageClassifier::load(&cfg)?;
    let wolf = WolfDefender::load(&cfg)?;
    let pihole = PiholeProbe::new(&cfg)?;
    let url_guard = UrlGuard::new(&cfg, Arc::clone(&pihole));

    let (readiness, handles) = Readiness::new();
    let readiness = Arc::new(readiness);
    let poll = cfg.readiness_poll;

    let cfg_pihole = cfg.clone();
    spawn_until_ready(handles.pihole, poll, move || {
        let c = cfg_pihole.clone();
        async move { PiholeProbe::check_ready(&c).await }
    });

    let cfg_ocr = cfg.clone();
    spawn_until_ready(handles.ocr, poll, move || {
        let c = cfg_ocr.clone();
        async move { ocr::check_ready(&c).await }
    });

    let cfg_chunker = cfg.clone();
    spawn_until_ready(handles.chunker, poll, move || {
        let c = cfg_chunker.clone();
        async move { chunker::check_ready(&c).await }
    });

    spawn_until_ready(handles.sentinel, poll, {
        let s = Arc::clone(&sentinel);
        move || {
            let s = Arc::clone(&s);
            async move { s.warmup().await.is_ok() }
        }
    });
    spawn_until_ready(handles.nsfw_text, poll, {
        let c = Arc::clone(&nsfw_text);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await.is_ok() }
        }
    });
    spawn_until_ready(handles.nsfw_image, poll, {
        let c = Arc::clone(&nsfw_image);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await.is_ok() }
        }
    });
    spawn_until_ready(handles.wolf, poll, {
        let c = Arc::clone(&wolf);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await.is_ok() }
        }
    });

    let state = AppState::new(
        cfg.clone(),
        readiness,
        sentinel,
        nsfw_text,
        nsfw_image,
        wolf,
        pihole,
        url_guard,
    );

    let filter_gate = fifo_concurrency::fifo_concurrency(cfg.filter_max_concurrent);
    let api = Router::with_path("api")
        .hoop(affix_state::inject(Arc::clone(&state)))
        .push(Router::with_path("health").get(health::health))
        .push(
            Router::with_path("filter")
                .hoop(filter_gate)
                .post(filter::filter_post),
        );

    let router = trypanophobe::openapi::mount_openapi(api);

    let cors = Cors::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers(vec![
            fifo_concurrency::QUEUE_WAIT_HEADER,
            fifo_concurrency::PROCESS_MS_HEADER,
        ])
        .into_handler();
    let service = Service::new(router).hoop(cors);

    let addr = format!("{}:{}", cfg.bind_host, cfg.bind_port);
    tracing::info!(%addr, "trypanophobe filter listening");
    let acceptor = TcpListener::new(addr).bind().await;
    let server = Server::new(acceptor);
    let handle = server.handle();

    tokio::select! {
        () = server.serve(service) => {},
        _ = shutdown_signal() => {
            tracing::info!("shutdown signal received");
            handle.stop_graceful(cfg.graceful_shutdown);
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {},
        _ = sigterm.recv() => {},
    }
}
