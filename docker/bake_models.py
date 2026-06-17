#!/usr/bin/env python3
"""Build-only: download and bake all inference assets under /models."""
import os
import shutil
import struct
from pathlib import Path

import numpy as np
import torch
from huggingface_hub import hf_hub_download, snapshot_download

MODELS = Path(os.environ.get("MODELS", "/models"))
SENTINEL_REPO = os.environ["SENTINEL_REPO"]
SENTINEL_QUANT = os.environ["SENTINEL_QUANT"]
NSFW_TEXT_REPO = "eliasalbouzidi/distilbert-nsfw-text-classifier"
NSFW_IMAGE_REPO = "Marqo/nsfw-image-detection-384"
WOLF_REPO = "patronus-studio/wolf-defender-prompt-injection"
NSFW_TEXT_PATTERNS = [
    "model.safetensors",
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "vocab.txt",
]
WOLF_PATTERNS = [
    "tokenizer.json",
    "tokenizer_config.json",
    "config.json",
    "onnx/onnx_fp16/model_fp16.onnx",
    "onnx/onnx_fp16/model.onnx",
]


def extract_cls_head(src: Path, dst: Path) -> None:
    obj = torch.load(src, map_location="cpu", weights_only=False)
    t = (obj["weight"] if isinstance(obj, dict) else obj).detach().float().numpy()
    rows, cols = int(t.shape[0]), int(t.shape[1])
    with open(dst, "wb") as out:
        out.write(struct.pack("<II", rows, cols))
        out.write(t.astype(np.float32).tobytes())
    print(f"wrote {dst} shape=({rows}, {cols})")


def export_marqo_nsfw_onnx(image_dir: Path) -> None:
    import timm
    from safetensors.torch import load_file

    weights_path = image_dir / "model.safetensors"
    state = load_file(str(weights_path))
    model = timm.create_model("vit_tiny_patch16_384", pretrained=False, num_classes=2)
    model.load_state_dict(state, strict=True)
    model.eval()

    dummy = torch.randn(1, 3, 384, 384)
    onnx_path = image_dir / "model.onnx"
    torch.onnx.export(
        model,
        dummy,
        str(onnx_path),
        input_names=["pixel_values"],
        output_names=["logits"],
        dynamic_axes={"pixel_values": {0: "batch"}, "logits": {0: "batch"}},
        opset_version=17,
        dynamo=False,
    )
    print(f"exported {onnx_path}")


def prune_nsfw_image(image_dir: Path) -> None:
    for path in image_dir.iterdir():
        if path.name != "model.onnx":
            if path.is_dir():
                shutil.rmtree(path)
            else:
                path.unlink()


def prune_wolf(wolf_dir: Path) -> None:
    keep = {wolf_dir / pattern for pattern in WOLF_PATTERNS}
    for path in sorted(wolf_dir.rglob("*"), key=lambda p: len(p.parts), reverse=True):
        if path in keep:
            continue
        if path.is_dir():
            if not any(path.iterdir()):
                path.rmdir()
        else:
            path.unlink()


def main() -> None:
    MODELS.mkdir(parents=True, exist_ok=True)

    hf_hub_download(
        repo_id=SENTINEL_REPO,
        filename=f"prompt-injection-jailbreak-sentinel-v2.{SENTINEL_QUANT}.gguf",
        local_dir=str(MODELS),
    )
    cls_head_pt = MODELS / "cls_head.pt"
    hf_hub_download(
        repo_id=SENTINEL_REPO,
        filename="cls_head.pt",
        local_dir=str(MODELS),
    )

    snapshot_download(
        repo_id=NSFW_TEXT_REPO,
        local_dir=str(MODELS / "nsfw-text"),
        allow_patterns=NSFW_TEXT_PATTERNS,
    )
    image_dir = MODELS / "nsfw-image"
    snapshot_download(repo_id=NSFW_IMAGE_REPO, local_dir=str(image_dir))
    export_marqo_nsfw_onnx(image_dir)
    prune_nsfw_image(image_dir)

    wolf_dir = MODELS / "wolf-defender"
    snapshot_download(
        repo_id=WOLF_REPO,
        local_dir=str(wolf_dir),
        allow_patterns=WOLF_PATTERNS,
    )
    prune_wolf(wolf_dir)

    extract_cls_head(cls_head_pt, MODELS / "cls_head.f32.bin")
    cls_head_pt.unlink()

    print("baked models:", sorted(p.name for p in MODELS.iterdir()))


if __name__ == "__main__":
    main()
