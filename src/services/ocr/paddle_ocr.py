#!/usr/bin/env python3
"""Shared PaddleOCR construction for bake and runtime."""

from __future__ import annotations

import os
from pathlib import Path

BAKED_LANG = "en"


def model_dir() -> Path:
    return Path(os.environ.get("OCR_MODEL_DIR", "/opt/trypanophobe/ocr/models"))


_root = model_dir()
_root.mkdir(parents=True, exist_ok=True)
os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
os.environ.setdefault("PADDLE_PDX_CACHE_HOME", str(_root))

from paddleocr import PaddleOCR  # noqa: E402


def create_paddle_ocr() -> PaddleOCR:
    return PaddleOCR(
        lang=BAKED_LANG,
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=True,
        enable_mkldnn=False,
    )
