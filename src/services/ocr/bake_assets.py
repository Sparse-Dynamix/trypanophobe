#!/usr/bin/env python3
"""Build-time: download PaddleOCR en models into OCR_MODEL_DIR."""

from __future__ import annotations

import json
import os
from pathlib import Path

MODEL_DIR = Path(os.environ.get("OCR_MODEL_DIR", "/opt/trypanophobe/ocr/models"))
MODEL_DIR.mkdir(parents=True, exist_ok=True)

os.environ["PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK"] = "True"
os.environ["PADDLE_PDX_CACHE_HOME"] = str(MODEL_DIR)

from paddleocr import PaddleOCR  # noqa: E402

ocr = PaddleOCR(
    lang="en",
    use_doc_orientation_classify=False,
    use_doc_unwarping=False,
    use_textline_orientation=True,
    enable_mkldnn=False,
)
_ = ocr  # warmup

manifest = {
    "lang": "en",
    "model_dir": str(MODEL_DIR),
    "cache_home": str(MODEL_DIR),
}
(MODEL_DIR / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
print(f"baked OCR models under {MODEL_DIR}")
