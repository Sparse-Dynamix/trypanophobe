#!/usr/bin/env python3
"""Build-time: download PaddleOCR en models into OCR_MODEL_DIR."""

from __future__ import annotations

import json
from pathlib import Path

from paddle_ocr import create_paddle_ocr, model_dir

root = model_dir()
root.mkdir(parents=True, exist_ok=True)

ocr = create_paddle_ocr()
_ = ocr  # warmup

manifest = {
    "lang": "en",
    "model_dir": str(model_dir()),
    "cache_home": str(model_dir()),
}
(root / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"baked OCR models under {root}")
