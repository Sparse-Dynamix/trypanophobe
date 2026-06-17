# trypanophobe

Reference filter backend for [Guardian CLI](https://github.com/Sparse-Dynamix/guardian) — safer coding harnesses.

Guardian POSTs raw response bodies to this service. HTTP **2xx** allows content; any other status blocks it. In Guardian **payload mode**, use `--tps` / `trypanophobe_swap` so filtered bodies (including partial `206` responses) replace what the harness sees.

## Quick start

```bash
export HF_TOKEN=...   # required for the first image build (see AGENTS.md)
docker compose up --build
```

The filter API is at `http://localhost:8080/api/filter`. `GET /` redirects to Swagger UI.

## Request lifecycle

1. `POST /api/filter?url=<source-url>` with body (**`url` is required**)
2. URL checked via Pi-hole; blocked hosts → **406** with JSON `stage` and `reason`
3. Body converted to markdown when needed, then chunked for scoring
   - Images: NSFW image filter → OCR → markdown
4. Each chunk scored: Sentinel V2 (full chunk), Wolf + NSFW text (512-token sliding windows)
5. Chunks flagged by **any** model are removed
6. Response:
   - **200** — all chunks safe
   - **206** — partial (`?format=md` only; use with `--tps` in payload mode)
   - **406** — blocked; JSON body includes `stage` (`url_check`, `nsfw_image`, `chunk_moderation`, `response_format`) and `reason`

### Query parameters

| Param | Required | Effect |
|-------|----------|--------|
| `url` | **yes** | Source URL for blocklist check + format hint |
| `format` | no (default `og`) | `og` = return original body; `md` = return filtered markdown |

## Guardian usage

**Payload mode** — filter response bodies; use `--tps` so partial `206` markdown replaces what the harness sees:

```bash
export GUARDIAN_TRYPANOPHOBE_FILTER=http://127.0.0.1:8080/api/filter
guardian --tpf "$GUARDIAN_TRYPANOPHOBE_FILTER" --tps -- your-agent-command
```

**Wrapper mode** — do **not** pass `--tps`. It rewrites packet shape in ways the wrapper harness does not expect and can break the run. Use `--tpf` only:

```bash
export GUARDIAN_TRYPANOPHOBE_FILTER=http://127.0.0.1:8080/api/filter
guardian --tpf "$GUARDIAN_TRYPANOPHOBE_FILTER" -- your-agent-command
```

`--tps` is only meaningful with `?format=md` when you want partial filtering (`206`).

## API

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/filter` | Guardian filter (`?url=` required) |
| `GET` | `/api/health` | Readiness |
| `GET` | `/` | Redirect to Swagger UI |
| `GET` | `/swagger-ui` | OpenAPI UI |

## Development

Build, smoke tests, architecture, and model details: [AGENTS.md](AGENTS.md).
