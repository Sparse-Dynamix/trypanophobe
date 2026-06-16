#!/usr/bin/env python3
"""Chonkie HTTP chunker — lossless markdown splitting."""

from __future__ import annotations

import json
import logging
import os
from functools import lru_cache
from pathlib import Path

import uvicorn
from chonkie import RecursiveChunker
from chonkie.tokenizer import TokieAutoTokenizer
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from tokie import Tokenizer as TokieTokenizer


class ChunkRequest(BaseModel):
    text: str
    max_tokens: int | None = None


class ChunkItem(BaseModel):
    index: int
    text: str
    token_count: int


class ChunkResponse(BaseModel):
    chunks: list[ChunkItem]


class StatusResponse(BaseModel):
    status: str


def _default_max_tokens() -> int:
    return int(os.environ.get("CHUNK_MAX_TOKENS", "2048"))


def _tokenizer_json_path() -> Path:
    model_root = Path(
        os.environ.get("CHUNKER_MODEL_DIR", "/opt/trypanophobe/chunker/models")
    )
    manifest = model_root / "manifest.json"
    if not manifest.is_file():
        raise SystemExit(f"missing chunker manifest: {manifest}")
    data = json.loads(manifest.read_text())
    path = Path(data.get("tokenizer_json") or data["tokenizer_path"])
    if path.is_dir():
        path = path / "tokenizer.json"
    if not path.is_file():
        raise SystemExit(f"missing tokenizer.json: {path}")
    return path


@lru_cache(maxsize=1)
def _tokenizer():
    path = _tokenizer_json_path()
    return TokieAutoTokenizer(TokieTokenizer.from_json(str(path)))


@lru_cache(maxsize=8)
def _chunker(max_tokens: int) -> RecursiveChunker:
    return RecursiveChunker(tokenizer=_tokenizer(), chunk_size=max_tokens)


def _chunk_text(text: str, max_tokens: int) -> list[ChunkItem]:
    raw = _chunker(max_tokens)(text)
    if not raw:
        return [ChunkItem(index=0, text="", token_count=0)]

    chunks = [
        ChunkItem(index=i, text=c.text, token_count=c.token_count)
        for i, c in enumerate(raw)
    ]
    reconstructed = "".join(c.text for c in chunks)
    if reconstructed != text:
        raise HTTPException(
            status_code=500,
            detail="chunking violated lossless invariant",
        )
    return chunks


def _create_app() -> FastAPI:
    app = FastAPI()
    _tokenizer()
    _chunker(_default_max_tokens())

    @app.post("/chunk")
    def chunk_endpoint(body: ChunkRequest) -> ChunkResponse:
        max_tokens = body.max_tokens if body.max_tokens is not None else _default_max_tokens()
        if max_tokens < 1:
            raise HTTPException(status_code=400, detail="max_tokens must be positive")
        return ChunkResponse(chunks=_chunk_text(body.text, max_tokens))

    @app.get("/health")
    def health() -> StatusResponse:
        return StatusResponse(status="healthy")

    return app


def main() -> None:
    logging.basicConfig(level=logging.INFO)
    host = os.environ.get("CHUNKER_HOST", "0.0.0.0")
    port = int(os.environ.get("CHUNKER_PORT", "8830"))
    logging.info(
        "Starting chunker on %s:%s tokenizer=%s",
        host,
        port,
        _tokenizer_json_path(),
    )
    uvicorn.run(_create_app(), host=host, port=port)


if __name__ == "__main__":
    main()
