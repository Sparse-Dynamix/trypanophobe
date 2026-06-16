#!/usr/bin/env python3
# Adapted from liteparse ocr/paddleocr/server.py (Apache-2.0)
"""PaddleOCR HTTP server implementing the LiteParse OCR API spec."""

from __future__ import annotations

import io
import json
import logging
import os
import traceback
from pathlib import Path
from typing import Any

import numpy as np
import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.datastructures import UploadFile
from fastapi.param_functions import File, Form
from paddleocr import PaddleOCR
from PIL import Image
from pydantic import BaseModel

BAKED_LANG = "en"


class OcrResponse(BaseModel):
    results: list[Any]


class StatusResponse(BaseModel):
    status: str


def _model_dir() -> Path:
    return Path(os.environ.get("OCR_MODEL_DIR", "/opt/trypanophobe/ocr/models"))


def _create_ocr() -> PaddleOCR:
    os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
    os.environ.setdefault("PADDLE_PDX_CACHE_HOME", str(_model_dir()))
    return PaddleOCR(
        lang=BAKED_LANG,
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=True,
        enable_mkldnn=False,
    )


def _normalize_language(language: str) -> str:
    normalized = language.lower()
    aliases = {"eng": "en"}
    return aliases.get(normalized, normalized)


def _create_app(ocr: PaddleOCR) -> FastAPI:
    app = FastAPI()

    @app.post("/ocr")
    async def ocr_endpoint(
        file: UploadFile = File(...), language: str = Form(default="en")
    ) -> OcrResponse:
        lang = _normalize_language(language)
        if lang != BAKED_LANG:
            raise HTTPException(
                status_code=400,
                detail=f"language {language!r} not available; only {BAKED_LANG!r} is baked",
            )

        try:
            image_data = await file.read()
            image = Image.open(io.BytesIO(image_data))
            if image.mode != "RGB":
                image = image.convert("RGB")
            image_array = np.array(image)
            results = ocr.predict(image_array)
        except Exception as e:
            logging.error("OCR failed:\n%s", traceback.format_exc())
            raise HTTPException(status_code=500, detail=str(e)) from e

        formatted: list[dict[str, Any]] = []
        if results and len(results) > 0:
            result = results[0]
            res_data = result.get("res", result) if isinstance(result, dict) else result
            if isinstance(res_data, dict):
                texts = res_data.get("rec_texts", [])
                scores = res_data.get("rec_scores", [])
                boxes = res_data.get("rec_boxes", [])
                polys = res_data.get("rec_polys", res_data.get("dt_polys", []))
            else:
                texts = getattr(res_data, "rec_texts", []) or []
                scores = getattr(res_data, "rec_scores", []) or []
                boxes = getattr(res_data, "rec_boxes", []) or []
                polys = (
                    getattr(res_data, "rec_polys", None)
                    or getattr(res_data, "dt_polys", None)
                    or []
                )

            if hasattr(texts, "tolist"):
                texts = texts.tolist()
            if hasattr(scores, "tolist"):
                scores = scores.tolist()
            if hasattr(boxes, "tolist"):
                boxes = boxes.tolist()
            if hasattr(polys, "tolist"):
                polys = polys.tolist()

            for i in range(len(texts)):
                text = texts[i]
                confidence = float(scores[i]) if i < len(scores) else 0.0
                if i < len(boxes):
                    box = boxes[i]
                    bbox = box.tolist() if hasattr(box, "tolist") else list(box)
                else:
                    bbox = [0, 0, 0, 0]

                polygon = None
                if i < len(polys):
                    poly = polys[i]
                    if hasattr(poly, "tolist"):
                        poly = poly.tolist()
                    if len(poly) == 4 and all(len(pt) == 2 for pt in poly):
                        polygon = [[float(pt[0]), float(pt[1])] for pt in poly]
                    if polygon is not None and bbox == [0, 0, 0, 0]:
                        xs = [pt[0] for pt in polygon]
                        ys = [pt[1] for pt in polygon]
                        bbox = [min(xs), min(ys), max(xs), max(ys)]

                item: dict[str, Any] = {
                    "text": text,
                    "bbox": bbox,
                    "confidence": confidence,
                }
                if polygon is not None:
                    item["polygon"] = polygon
                formatted.append(item)

        return OcrResponse(results=formatted)

    @app.get("/health")
    def health() -> StatusResponse:
        return StatusResponse(status="healthy")

    return app


def main() -> None:
    logging.basicConfig(level=logging.INFO)
    manifest = _model_dir() / "manifest.json"
    if manifest.is_file():
        logging.info("OCR manifest: %s", manifest.read_text().strip())
    host = os.environ.get("OCR_HOST", "0.0.0.0")
    port = int(os.environ.get("OCR_PORT", "8829"))
    ocr = _create_ocr()
    logging.info("Starting OCR server on %s:%s", host, port)
    uvicorn.run(_create_app(ocr), host=host, port=port)


if __name__ == "__main__":
    main()
