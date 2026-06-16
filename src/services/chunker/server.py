#!/usr/bin/env python3
"""Chonkie HTTP chunker — lossless markdown splitting."""

from __future__ import annotations

import json
import logging
import os
from pathlib import Path

import uvicorn
from chonkie import RecursiveChunker
from chonkie.tokenizer import AutoTokenizer, TokieAutoTokenizer
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


def _tokenizer_spec() -> str:
    model_root = Path(
        os.environ.get("CHUNKER_MODEL_DIR", "/opt/trypanophobe/chunker/models")
    )
    manifest = model_root / "manifest.json"
    if manifest.is_file():
        data = json.loads(manifest.read_text())
        return data.get("tokenizer_json") or data["tokenizer_path"]
    return os.environ.get("CHUNK_TOKENIZER", "gpt2")


def _load_tokenizer():
    spec = _tokenizer_spec()
    spec_path = Path(spec)
    if spec_path.is_file() and spec_path.name == "tokenizer.json":
        return TokieAutoTokenizer(TokieTokenizer.from_json(str(spec_path)))
    if spec_path.is_dir():
        tokenizer_json = spec_path / "tokenizer.json"
        if tokenizer_json.is_file():
            return TokieAutoTokenizer(TokieTokenizer.from_json(str(tokenizer_json)))
    return AutoTokenizer(spec)


def _create_chunker(max_tokens: int) -> RecursiveChunker:
    return RecursiveChunker(tokenizer=_load_tokenizer(), chunk_size=max_tokens)


def _chunk_text(text: str, max_tokens: int) -> list[ChunkItem]:
    chunker = _create_chunker(max_tokens)
    raw = chunker(text)
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
    logging.info("Starting chunker on %s:%s tokenizer=%s", host, port, _tokenizer_spec())
    uvicorn.run(_create_app(), host=host, port=port)


if __name__ == "__main__":
    main()
