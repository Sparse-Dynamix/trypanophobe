use std::sync::Arc;

use salvo::affix_state;
use salvo::cors::{Any, Cors};
use salvo::http::request::set_global_secure_max_size;
use salvo::prelude::*;
use tokio::signal::unix::{signal, SignalKind};
use tracing_subscriber::EnvFilter;
use trypanophobe::config::Config;
use trypanophobe::pipeline::url_guard::UrlGuard;
use trypanophobe::readiness::{spawn_poller, spawn_warmup, Readiness};
use trypanophobe::routes::{filter, health};
use trypanophobe::services::{
    paddleocr, NsfwImageClassifier, NsfwTextClassifier, PiholeProbe, Sentinel, WolfDefender,
};
use trypanophobe::state::AppState;

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
    spawn_poller(handles.pihole, poll, move || {
        let c = cfg_pihole.clone();
        async move { PiholeProbe::check_ready(&c).await }
    });

    let cfg_paddleocr = cfg.clone();
    spawn_poller(handles.paddleocr, poll, move || {
        let c = cfg_paddleocr.clone();
        async move { paddleocr::check_ready(&c).await }
    });

    spawn_warmup(handles.sentinel, poll, {
        let s = Arc::clone(&sentinel);
        move || {
            let s = Arc::clone(&s);
            async move { s.warmup().await }
        }
    });
    spawn_warmup(handles.nsfw_text, poll, {
        let c = Arc::clone(&nsfw_text);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await }
        }
    });
    spawn_warmup(handles.nsfw_image, poll, {
        let c = Arc::clone(&nsfw_image);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await }
        }
    });
    spawn_warmup(handles.wolf, poll, {
        let c = Arc::clone(&wolf);
        move || {
            let c = Arc::clone(&c);
            async move { c.warmup().await }
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

    let api = Router::with_path("api")
        .hoop(affix_state::inject(Arc::clone(&state)))
        .push(Router::with_path("health").get(health::health));

    let filter_router = Router::new()
        .hoop(affix_state::inject(Arc::clone(&state)))
        .post(filter::filter_root)
        .push(Router::with_path("filter").post(filter::filter_post));

    let router = trypanophobe::openapi::mount_openapi(api).push(filter_router);

    let cors = Cors::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
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
