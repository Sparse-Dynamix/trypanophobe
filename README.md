# trypanophobe

Reference filter backend for [Guardian CLI](https://github.com/Sparse-Dynamix/guardian) — safer coding harnesses.

Guardian POSTs raw response bodies to this service. HTTP **2xx** allows content; any other status blocks it. In Guardian **payload mode**, use `--tps` / `trypanophobe_swap` so filtered bodies (including partial `206` responses) replace what the harness sees.

## Request lifecycle

1. `POST /?url=<source-url>` with body (**`url` is required**)
2. URL checked via Pi-hole (+ SSRF guard); blocked hosts → **406**
3. Body converted to markdown when needed, chunked via Chonkie HTTP sidecar (lossless)
   - Images: NSFW image filter → OCR → markdown
4. Each chunk scored: Sentinel V2 (full chunk), Wolf + NSFW text (512-token sliding windows)
5. Chunks flagged by **any** model are removed
6. Response:
   - **200** — all chunks safe
   - **206** — partial (`?format=md` only; use with `--tps` in payload mode)
   - **406** — all chunks removed, URL blocked, or partial with `format=og`

Query params:

| Param | Required | Effect |
|-------|----------|--------|
| `url` | **yes** | Source URL for Pi-hole check + format hint |
| `format` | no (default `og`) | `og` = return original body; `md` = return filtered markdown |

## Stack

- Rust (Salvo, Tokio)
- [liteparse](https://github.com/run-llama/liteparse) for PDF/office/images; OCR via HTTP sidecar
- [Chonkie](https://github.com/chonkie-inc/chonkie) chunker HTTP sidecar (lossless, max 2048 tokens/chunk)
- [anytomd](https://crates.io/crates/anytomd) for HTML/JSON/text
- ML: Sentinel V2 Q8, Wolf Defender (ONNX), DistilBERT NSFW text, Marqo NSFW image ViT
- Pi-hole FTL in the same container (supervisord)

## Run

```bash
export HF_TOKEN=...
docker compose up --build
# filter at http://localhost:8080/
```

`HF_TOKEN` is a **build secret only** (model bake). It is not passed to the running container.

Smoke test:

```bash
export HF_TOKEN=...
SMOKE_BUILD=1 ./smoke.sh
```

Optional offline gate (no runtime network after build):

```bash
SMOKE_BUILD=1 SMOKE_OFFLINE=1 ./smoke.sh
```

The container runs four supervisord programs: **pihole-FTL**, **ocr** (:8829), **chunker** (:8830), and **trypanophobe** (:8080).

Python sidecars use locked deps (`uv.lock` in `src/services/ocr/` and `src/services/chunker/`). Update locks with `uv lock` in each directory after changing `pyproject.toml`.

## Guardian usage

**Payload mode** — filter response bodies; use `--tps` so partial `206` markdown replaces what the harness sees:

```bash
export GUARDIAN_TRYPANOPHOBE_FILTER=http://127.0.0.1:8080/
guardian --tpf "$GUARDIAN_TRYPANOPHOBE_FILTER" --tps -- your-agent-command
```

**Wrapper mode** — do **not** pass `--tps`. It rewrites packet shape in ways the wrapper harness does not expect and can break the run. Use `--tpf` only:

```bash
export GUARDIAN_TRYPANOPHOBE_FILTER=http://127.0.0.1:8080/
guardian --tpf "$GUARDIAN_TRYPANOPHOBE_FILTER" -- your-agent-command
```

`--tps` is only meaningful with `?format=md` when you want partial filtering (`206`).

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/` or `/filter` | Guardian filter (`?url=` required) |
| `GET` | `/api/health` | Readiness flags |
| `GET` | `/swagger-ui` | OpenAPI UI |

Sidecars (internal): `POST :8829/ocr`, `POST :8830/chunk`, `GET .../health` on each.

## Models

Baked at image build time — nothing downloads at container start:

- Rust ML via [`docker/bake_models.py`](docker/bake_models.py) → `/opt/trypanophobe/models/`
- OCR via [`src/services/ocr/bake_assets.py`](src/services/ocr/bake_assets.py) → `/opt/trypanophobe/ocr/models/`
- Chunker tokenizer via [`src/services/chunker/bake_assets.py`](src/services/chunker/bake_assets.py) → `/opt/trypanophobe/chunker/models/`

Hugging Face assets:

- [Sentinel V2 Q8](https://huggingface.co/qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF)
- [Wolf Defender](https://huggingface.co/patronus-studio/wolf-defender-prompt-injection)
- [NSFW text](https://huggingface.co/eliasalbouzidi/distilbert-nsfw-text-classifier)
- [NSFW image](https://huggingface.co/Marqo/nsfw-image-detection-384)
- gpt2 tokenizer (chunker)
