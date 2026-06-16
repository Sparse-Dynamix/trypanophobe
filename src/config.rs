use std::env;
use std::path::PathBuf;
use std::time::Duration;

const EMBEDDED_DEFAULTS: &str = include_str!("../config/env.defaults");
const RUNTIME_DEFAULTS: &str = "/etc/trypanophobe/env.defaults";
const BUILD_ENV: &str = "/etc/trypanophobe/build.env";

#[derive(Clone, Debug)]
pub struct Config {
    pub bind_host: String,
    pub bind_port: u16,
    pub sentinel_model_path: PathBuf,
    pub sentinel_cls_head_path: PathBuf,
    pub sentinel_gpu_layers: u32,
    pub sentinel_max_input_tokens: usize,
    pub sentinel_max_parallel: usize,
    pub sentinel_warmup_text: String,
    pub sentinel_threshold: f32,
    pub pihole_dns: String,
    pub pihole_ready_probe_host: String,
    pub readiness_poll: Duration,
    pub readiness_wait: Duration,
    pub max_request_body_bytes: usize,
    pub graceful_shutdown: Duration,
    pub max_input_bytes: usize,
    pub max_zip_bytes: usize,
    pub max_image_bytes: usize,
    pub nsfw_text_model_dir: PathBuf,
    pub nsfw_image_model_dir: PathBuf,
    pub wolf_model_dir: PathBuf,
    pub ocr_url: String,
    pub ocr_health_url: String,
    pub chunker_url: String,
    pub chunker_health_url: String,
    pub chunk_max_tokens: usize,
    pub nsfw_text_threshold: f32,
    pub nsfw_image_threshold: f32,
    pub wolf_threshold: f32,
    pub wolf_window_tokens: usize,
    pub nsfw_text_window_tokens: usize,
    pub url_cache_ttl: Duration,
    pub url_cache_capacity: u64,
}

fn trim_env_value(v: &str) -> &str {
    let v = v.trim();
    if v.len() >= 2 {
        if let (Some('"'), Some('"')) = (v.chars().next(), v.chars().last()) {
            return &v[1..v.len() - 1];
        }
        if let (Some('\''), Some('\'')) = (v.chars().next(), v.chars().last()) {
            return &v[1..v.len() - 1];
        }
    }
    v
}

fn load_env_file(content: &str) {
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            if env::var(k).is_err() {
                let v = trim_env_value(v);
                env::set_var(k, v);
            }
        }
    }
}

fn load_env_path(path: &str) {
    if let Ok(content) = std::fs::read_to_string(path) {
        load_env_file(&content);
    }
}

fn env_str(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("missing required env var: {key}"))
}

fn env_opt(key: &str) -> Option<String> {
    env::var(key).ok().filter(|s| !s.is_empty())
}

fn env_usize(key: &str) -> usize {
    env_str(key)
        .parse()
        .unwrap_or_else(|_| panic!("invalid usize for {key}"))
}

fn env_u32(key: &str) -> u32 {
    env_str(key)
        .parse()
        .unwrap_or_else(|_| panic!("invalid u32 for {key}"))
}

fn env_u16(key: &str) -> u16 {
    env_str(key)
        .parse()
        .unwrap_or_else(|_| panic!("invalid u16 for {key}"))
}

fn env_u64(key: &str) -> u64 {
    env_str(key)
        .parse()
        .unwrap_or_else(|_| panic!("invalid u64 for {key}"))
}

fn env_f32(key: &str) -> f32 {
    env_str(key)
        .parse()
        .unwrap_or_else(|_| panic!("invalid f32 for {key}"))
}

fn env_secs(key: &str) -> Duration {
    Duration::from_secs(
        env_str(key)
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("invalid seconds for {key}")),
    )
}

impl Config {
    pub fn from_env() -> Self {
        load_env_file(EMBEDDED_DEFAULTS);
        load_env_path(RUNTIME_DEFAULTS);
        load_env_path(BUILD_ENV);

        let models_base = env_opt("MODELS_BASE")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/opt/trypanophobe/models"));

        let quant = env_str("SENTINEL_QUANT");
        let default_model = models_base.join(format!(
            "prompt-injection-jailbreak-sentinel-v2.{quant}.gguf"
        ));

        let default_cls = models_base.join("cls_head.f32.bin");
        let default_nsfw_text = models_base.join("nsfw-text");
        let default_nsfw_image = models_base.join("nsfw-image");
        let default_wolf = models_base.join("wolf-defender");

        Self {
            bind_host: env_str("BIND_HOST"),
            bind_port: env::var("AWS_LWA_PORT")
                .or_else(|_| env::var("PORT"))
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or_else(|| env_u16("BIND_PORT")),
            sentinel_model_path: env_opt("SENTINEL_MODEL_PATH")
                .map(PathBuf::from)
                .unwrap_or(default_model),
            sentinel_cls_head_path: env_opt("SENTINEL_CLS_HEAD_PATH")
                .map(PathBuf::from)
                .unwrap_or(default_cls),
            sentinel_gpu_layers: env_u32("SENTINEL_GPU_LAYERS"),
            sentinel_max_input_tokens: env_usize("SENTINEL_MAX_INPUT_TOKENS"),
            sentinel_max_parallel: env_usize("SENTINEL_MAX_PARALLEL").max(1),
            sentinel_warmup_text: env_str("SENTINEL_WARMUP_TEXT"),
            sentinel_threshold: env_f32("SENTINEL_THRESHOLD"),
            pihole_dns: env_str("PIHOLE_DNS"),
            pihole_ready_probe_host: env_str("PIHOLE_READY_PROBE_HOST"),
            readiness_poll: env_secs("READINESS_POLL_SECS"),
            readiness_wait: env_secs("READINESS_WAIT_SECS"),
            max_request_body_bytes: env_usize("MAX_REQUEST_BODY_BYTES"),
            graceful_shutdown: env_secs("GRACEFUL_SHUTDOWN_SECS"),
            max_input_bytes: env_usize("MAX_INPUT_BYTES"),
            max_zip_bytes: env_usize("MAX_ZIP_BYTES"),
            max_image_bytes: env_usize("MAX_IMAGE_BYTES"),
            nsfw_text_model_dir: env_opt("NSFW_TEXT_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or(default_nsfw_text),
            nsfw_image_model_dir: env_opt("NSFW_IMAGE_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or(default_nsfw_image),
            wolf_model_dir: env_opt("WOLF_MODEL_DIR")
                .map(PathBuf::from)
                .unwrap_or(default_wolf),
            ocr_url: env_str("OCR_URL"),
            ocr_health_url: env_str("OCR_HEALTH_URL"),
            chunker_url: env_str("CHUNKER_URL"),
            chunker_health_url: env_str("CHUNKER_HEALTH_URL"),
            chunk_max_tokens: env_usize("CHUNK_MAX_TOKENS"),
            nsfw_text_threshold: env_f32("NSFW_TEXT_THRESHOLD"),
            nsfw_image_threshold: env_f32("NSFW_IMAGE_THRESHOLD"),
            wolf_threshold: env_f32("WOLF_THRESHOLD"),
            wolf_window_tokens: env_usize("WOLF_WINDOW_TOKENS").max(64),
            nsfw_text_window_tokens: env_usize("NSFW_TEXT_WINDOW_TOKENS").max(64),
            url_cache_ttl: env_secs("URL_CACHE_TTL_SECS"),
            url_cache_capacity: env_u64("URL_CACHE_CAPACITY"),
        }
    }
}
