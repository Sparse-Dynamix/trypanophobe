# trypanophobe

Reference filter backend for [Guardian CLI](https://github.com/Sparse-Dynamix/guardian) — safer coding harnesses.

Guardian POSTs raw response bodies to this service. HTTP **2xx** allows content; any other status blocks it. Use Guardian `--tps` / `trypanophobe_swap` so filtered bodies (including partial `206` responses) replace what the harness sees.

## Request lifecycle

1. `POST /` with body; optional `?url=<source-url>` for HTTP responses
2. URL checked via Pi-hole (+ SSRF guard); blocked hosts → **406**
3. Body converted to markdown when needed, chunked by headings
   - Images: NSFW image filter → OCR (PaddleOCR) → markdown
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
- [liteparse](https://github.com/run-llama/liteparse) for PDF/office/images; OCR via HTTP to PaddleOCR sidecar
- [anytomd](https://crates.io/crates/anytomd) for HTML/JSON/text
- ML: Sentinel V2 Q8, Wolf Defender (ONNX), DistilBERT NSFW text, Marqo NSFW image ViT
- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) HTTP sidecar (supervisord)
- Pi-hole FTL in the same container (supervisord)

## Run

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

The container runs three supervisord programs: **pihole-FTL**, **paddleocr** (port 8829), and **trypanophobe** (port 8080).

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

Baked at image build time via [`docker/bake_models.py`](docker/bake_models.py):

- [Sentinel V2 Q8](https://huggingface.co/qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF)
- [Wolf Defender](https://huggingface.co/patronus-studio/wolf-defender-prompt-injection)
- [NSFW text](https://huggingface.co/eliasalbouzidi/distilbert-nsfw-text-classifier)
- [NSFW image](https://huggingface.co/Marqo/nsfw-image-detection-384)

PaddleOCR PP-OCR weights are pre-downloaded in the Docker image at build time.
