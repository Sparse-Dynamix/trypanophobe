#!/usr/bin/env python3
# Adapted from liteparse ocr/paddleocr/server.py (Apache-2.0)
# https://github.com/run-llama/liteparse/blob/main/ocr/paddleocr/server.py
"""PaddleOCR HTTP server implementing the LiteParse OCR API spec."""

import io
import logging
import traceback
from typing import Any

import numpy as np
import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.datastructures import UploadFile
from fastapi.param_functions import File, Form
from paddleocr import PaddleOCR
from PIL import Image
from pydantic import BaseModel


class OcrResponse(BaseModel):
    results: list[Any]


class StatusResponse(BaseModel):
    status: str


class PaddleOCRServer:
    def __init__(self) -> None:
        self.ocr: PaddleOCR = PaddleOCR(
            lang="en",
            use_doc_orientation_classify=False,
            use_doc_unwarping=False,
            use_textline_orientation=True,
        )
        self.current_language: str = "en"

    @staticmethod
    def normalize_language(language: str) -> str:
        normalized = language.lower()
        aliases = {
            "eng": "en",
            "zh": "ch",
            "zh-cn": "ch",
            "zh-hans": "ch",
            "zh-tw": "chinese_cht",
            "zh-hant": "chinese_cht",
            "ja": "japan",
            "ko": "korean",
        }
        return aliases.get(normalized, normalized)

    def _create_ocr_server(self) -> FastAPI:
        app = FastAPI()

        @app.post("/ocr")
        async def ocr_endpoint(
            file: UploadFile = File(...), language: str = Form(default="en")
        ) -> OcrResponse:
            language = self.normalize_language(language)

            try:
                if self.current_language != language:
                    self.ocr = PaddleOCR(
                        lang=language,
                        use_doc_orientation_classify=False,
                        use_doc_unwarping=False,
                        use_textline_orientation=True,
                    )
                    self.current_language = language

                image_data = await file.read()
                image = Image.open(io.BytesIO(image_data))

                if image.mode != "RGB":
                    image = image.convert("RGB")
                image_array = np.array(image)

                results = self.ocr.predict(image_array)
            except ValueError as ve:
                if "No models are available for the language" in str(ve):
                    raise HTTPException(status_code=400, detail=str(ve)) from ve
                raise HTTPException(status_code=500, detail=str(ve)) from ve
            except Exception as e:
                logging.error("OCR failed:\n%s", traceback.format_exc())
                raise HTTPException(status_code=500, detail=str(e)) from e

            formatted = []

            if results and len(results) > 0:
                result = results[0]
                res_data = (
                    result.get("res", result) if isinstance(result, dict) else result
                )
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
                        if hasattr(box, "tolist"):
                            bbox = box.tolist()
                        else:
                            bbox = list(box)
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

                    item = {"text": text, "bbox": bbox, "confidence": confidence}
                    if polygon is not None:
                        item["polygon"] = polygon
                    formatted.append(item)

            return OcrResponse(results=formatted)

        @app.get("/health")
        def health() -> StatusResponse:
            return StatusResponse(status="healthy")

        return app

    def serve(self) -> None:
        app = self._create_ocr_server()
        uvicorn.run(app, host="0.0.0.0", port=8829)


if __name__ == "__main__":
    logging.basicConfig(level=logging.INFO)
    logging.info("Starting PaddleOCR server on port 8829")
    PaddleOCRServer().serve()
