#!/usr/bin/env python3
"""Build-time: download chunker tokenizer and warm Chonkie."""

from __future__ import annotations

import json
import os
from pathlib import Path

from huggingface_hub import snapshot_download

MODEL_ROOT = Path(os.environ.get("CHUNKER_MODEL_DIR", "/opt/trypanophobe/chunker/models"))
TOKENIZER_REPO = os.environ.get("CHUNK_TOKENIZER_REPO", "gpt2")
TOKENIZER_DIR = MODEL_ROOT / "tokenizer"
TOKENIZER_DIR.mkdir(parents=True, exist_ok=True)

token = os.environ.get("HF_TOKEN")
snapshot_download(
    repo_id=TOKENIZER_REPO,
    local_dir=str(TOKENIZER_DIR),
    token=token,
)

from chonkie import RecursiveChunker  # noqa: E402
from chonkie.tokenizer import TokieAutoTokenizer  # noqa: E402
from tokie import Tokenizer as TokieTokenizer  # noqa: E402

tokenizer_json = TOKENIZER_DIR / "tokenizer.json"
if not tokenizer_json.is_file():
    raise SystemExit(f"missing tokenizer.json under {TOKENIZER_DIR}")

tokenizer = TokieAutoTokenizer(TokieTokenizer.from_json(str(tokenizer_json)))
chunker = RecursiveChunker(tokenizer=tokenizer, chunk_size=64)
_ = chunker("warmup chunk for build")

manifest = {
    "tokenizer_path": str(TOKENIZER_DIR),
    "tokenizer_json": str(tokenizer_json),
    "tokenizer_repo": TOKENIZER_REPO,
}
(MODEL_ROOT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"baked chunker tokenizer under {TOKENIZER_DIR}")
