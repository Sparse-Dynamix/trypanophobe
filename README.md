# trypanophobe

Reference filter backend for [Guardian CLI](https://github.com/Sparse-Dynamix/guardian) — safer coding harnesses.

Guardian POSTs raw response bodies to this service. HTTP **2xx** allows content; any other status blocks it. Use Guardian `--tps` / `trypanophobe_swap` so filtered bodies (including partial `206` responses) replace what the harness sees.

## Request lifecycle

1. `POST /` with body; optional `?url=<source-url>` for HTTP responses
2. URL checked via Pi-hole (+ SSRF guard); blocked hosts → **406**
3. Body converted to markdown when needed, chunked by headings
   - Images: NSFW image filter → OCR → markdown
4. Each chunk scored in parallel: Sentinel V2, Wolf Defender, NSFW text
5. Chunks flagged by **any** model are removed
6. Response:
   - **200** — all chunks safe
   - **206** — partial (safe chunks only in body when `?markdown=1` or `--tps`)
   - **406** — all chunks removed or URL blocked

Query params:

| Param | Effect |
|-------|--------|
| `url` | Source URL for Pi-hole + format hint |
| `markdown` / `format=markdown` | Return filtered markdown body with `Content-Type: text/markdown` |

## Stack

- Rust (Salvo, Tokio)
- [liteparse](https://github.com/run-llama/liteparse) for PDF/office/images (without bundled Tesseract)
- [anytomd](https://crates.io/crates/anytomd) for HTML/JSON/text
- ML: Sentinel V2 Q8, Wolf Defender (ONNX), DistilBERT NSFW text, Marqo NSFW image ViT, ocrs OCR
- Pi-hole FTL sidecar in Docker

## Docker (recommended)

```bash
export HF_TOKEN=...
docker compose up --build
# filter at http://localhost:8080/
```

Smoke test:

```bash
export HF_TOKEN=...
SMOKE_BUILD=1 ./smoke.sh
```

## Native development

Requirements: Rust stable, `cmake`, `clang`, `pkg-config`, `libssl-dev`, LibreOffice, ImageMagick, Tesseract (for liteparse office/image conversion at runtime).

```bash
export HF_TOKEN=...
./scripts/download-models.sh

export MODELS_BASE=$PWD/models
export SENTINEL_MODEL_PATH=$MODELS_BASE/prompt-injection-jailbreak-sentinel-v2.Q8_0.gguf
export SENTINEL_CLS_HEAD_PATH=$MODELS_BASE/cls_head.f32.bin
export NSFW_TEXT_MODEL_DIR=$MODELS_BASE/nsfw-text
export NSFW_IMAGE_MODEL_DIR=$MODELS_BASE/nsfw-image
export WOLF_MODEL_DIR=$MODELS_BASE/wolf-defender
export OCRS_MODEL_DIR=$MODELS_BASE/ocrs
export PIHOLE_DNS=127.0.0.1:5353   # when Pi-hole runs in Docker

cargo run --release --bin trypanophobe
```

## Guardian usage

```bash
export GUARDIAN_TRYPANOPHOBE_FILTER=http://127.0.0.1:8080/
guardian --tpf "$GUARDIAN_TRYPANOPHOBE_FILTER" --tps -- your-agent-command
```

`--tps` is required for meaningful partial filtering (`206`) and markdown swaps.

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/` or `/filter` | Guardian filter |
| `GET` | `/api/health` | Readiness flags |
| `GET` | `/swagger-ui` | OpenAPI UI |

## Models

Baked at build time via `docker/bake_models.py`:

- [Sentinel V2 Q8](https://huggingface.co/qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF)
- [Wolf Defender](https://huggingface.co/patronus-studio/wolf-defender-prompt-injection)
- [NSFW text](https://huggingface.co/eliasalbouzidi/distilbert-nsfw-text-classifier)
- [NSFW image](https://huggingface.co/Marqo/nsfw-image-detection-384)
- ocrs detection/recognition weights
