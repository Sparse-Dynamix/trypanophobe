# AGENTS.md

Developer and agent notes for the trypanophobe repository.

## Stack

- Rust (Salvo, Tokio) — main filter service
- [liteparse](https://github.com/run-llama/liteparse) for PDF/office/images; OCR via HTTP sidecar
- [Chonkie](https://github.com/chonkie-inc/chonkie) chunker HTTP sidecar (lossless, max 2048 tokens/chunk)
- [anytomd](https://crates.io/crates/anytomd) for HTML/JSON/text
- ML: Sentinel V2 Q8, Wolf Defender (ONNX), DistilBERT NSFW text, Marqo NSFW image ViT
- Pi-hole FTL in the same container (supervisord)
- Headless LibreOffice + ImageMagick for liteparse format conversion

## Architecture

The container runs four supervisord programs:

| Program | Port | Role |
|---------|------|------|
| `pihole-FTL` | — | DNS blocklist |
| `ocr` | 8829 | PaddleOCR sidecar |
| `chunker` | 8830 | Chonkie tokenizer/chunker sidecar |
| `trypanophobe` | 8080 | Filter API |

Public endpoints:

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/filter` | Guardian filter (`?url=` required) |
| `GET` | `/api/health` | Readiness |
| `GET` | `/` | Redirect to Swagger UI |

Sidecar endpoints (internal):

- `POST :8829/ocr`, `GET :8829/health`
- `POST :8830/chunk`, `GET :8830/health`

406 filter blocks return JSON: `{"error":"content_blocked","stage":"...","reason":"...","detail":"..."}`.

## Concurrency and CORS

- `FILTER_MAX_CONCURRENT` (default `1`) — FIFO gate on `POST /api/filter` only via [`src/middleware/fifo_concurrency.rs`](src/middleware/fifo_concurrency.rs). Excess requests wait in queue; health and Swagger are unbounded.
- Response headers `X-Queue-Wait-Ms` and `X-Process-Ms` report queue vs handler time.
- CORS is applied on the Salvo `Service` (not the router) per [Salvo CORS docs](https://salvo.rs/guide/features/cors.html), with permissive origins/methods/headers and exposed timing headers for cross-origin clients.

Python sidecars use locked deps (`uv.lock` in `src/services/ocr/` and `src/services/chunker/`). After changing `pyproject.toml`, run `uv lock` in each directory.

## Build and run

```bash
export HF_TOKEN=...   # Hugging Face token for model bake (build secret only)
docker compose up --build
```

`HF_TOKEN` is passed as a Docker build secret. It is **not** available to the running container.

First build downloads and bakes models; subsequent builds reuse cached layers when inputs are unchanged.

### Rust tests (no local toolchain required)

```bash
docker build --target rust-builder -f docker/Dockerfile .
```

The `rust-builder` stage runs `cargo test --lib` and `cargo build --release`. Liteparse fixture tests skip when LibreOffice is not installed.

## Smoke test

```bash
export HF_TOKEN=...
SMOKE_BUILD=1 ./smoke.sh
```

Optional offline gate (no runtime network after build):

```bash
SMOKE_BUILD=1 SMOKE_OFFLINE=1 ./smoke.sh
```

`SMOKE_BUILD=1` rebuilds the image before exercising OCR, chunker, filter, 406 stage assertions, and liteparse fixture conversions.

## Models

Baked at image build time — nothing downloads at container start:

- Rust ML via [`docker/bake_models.py`](docker/bake_models.py) → `/opt/trypanophobe/models/`
- OCR via [`src/services/ocr/bake_assets.py`](src/services/ocr/bake_assets.py) → `/opt/trypanophobe/ocr/models/`
- Chunker tokenizer via [`src/services/chunker/bake_assets.py`](src/services/chunker/bake_assets.py) → `/opt/trypanophobe/chunker/models/`

Hugging Face sources:

- [Sentinel V2 Q8](https://huggingface.co/qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF)
- [Wolf Defender](https://huggingface.co/patronus-studio/wolf-defender-prompt-injection)
- [NSFW text](https://huggingface.co/eliasalbouzidi/distilbert-nsfw-text-classifier)
- [NSFW image](https://huggingface.co/Marqo/nsfw-image-detection-384)
- gpt2 tokenizer (chunker)

## Layout

| Path | Purpose |
|------|---------|
| `src/` | Rust filter service |
| `src/services/ocr/` | OCR Python sidecar |
| `src/services/chunker/` | Chunker Python sidecar |
| `fixtures/` | Checked-in liteparse test files (see `ATTRIBUTION.md`) |
| `docker/` | Dockerfile, model bake, entrypoint |
| `config/env.defaults` | Default environment |
| `supervisor/` | supervisord config |
| `smoke.sh` | Integration smoke test |
