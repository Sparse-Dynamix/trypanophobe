#!/usr/bin/env python3
"""Shared PaddleOCR construction for bake and runtime."""

from __future__ import annotations

import os
from pathlib import Path

from paddleocr import PaddleOCR

BAKED_LANG = "en"


def model_dir() -> Path:
    return Path(os.environ.get("OCR_MODEL_DIR", "/opt/trypanophobe/ocr/models"))


def create_paddle_ocr() -> PaddleOCR:
    os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
    os.environ.setdefault("PADDLE_PDX_CACHE_HOME", str(model_dir()))
    return PaddleOCR(
        lang=BAKED_LANG,
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=True,
        enable_mkldnn=False,
    )
