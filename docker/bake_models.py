#!/usr/bin/env python3
"""Build-only: download and bake all inference assets under /models."""
import os
from pathlib import Path

from huggingface_hub import hf_hub_download, snapshot_download

MODELS = Path(os.environ.get("MODELS", "/models"))
SENTINEL_REPO = os.environ["SENTINEL_REPO"]
SENTINEL_QUANT = os.environ["SENTINEL_QUANT"]
NSFW_TEXT_REPO = "eliasalbouzidi/distilbert-nsfw-text-classifier"
NSFW_IMAGE_REPO = "Marqo/nsfw-image-detection-384"
WOLF_REPO = "patronus-studio/wolf-defender-prompt-injection"
OCRS_DET_URL = "https://ocrs-models.s3-accelerate.amazonaws.com/text-detection.rten"
OCRS_REC_URL = "https://ocrs-models.s3-accelerate.amazonaws.com/text-recognition.rten"


def export_marqo_nsfw_onnx(image_dir: Path) -> None:
    import timm
    import torch
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


def main() -> None:
    MODELS.mkdir(parents=True, exist_ok=True)

    hf_hub_download(
        repo_id=SENTINEL_REPO,
        filename=f"prompt-injection-jailbreak-sentinel-v2.{SENTINEL_QUANT}.gguf",
        local_dir=str(MODELS),
    )
    hf_hub_download(
        repo_id=SENTINEL_REPO,
        filename="cls_head.pt",
        local_dir=str(MODELS),
    )

    snapshot_download(repo_id=NSFW_TEXT_REPO, local_dir=str(MODELS / "nsfw-text"))
    image_dir = MODELS / "nsfw-image"
    snapshot_download(repo_id=NSFW_IMAGE_REPO, local_dir=str(image_dir))
    export_marqo_nsfw_onnx(image_dir)

    snapshot_download(
        repo_id=WOLF_REPO,
        local_dir=str(MODELS / "wolf-defender"),
        allow_patterns=[
            "tokenizer.json",
            "tokenizer_config.json",
            "config.json",
            "onnx/**",
        ],
    )

    ocrs_dir = MODELS / "ocrs"
    ocrs_dir.mkdir(parents=True, exist_ok=True)
    import urllib.request

    urllib.request.urlretrieve(OCRS_DET_URL, ocrs_dir / "text-detection.rten")
    urllib.request.urlretrieve(OCRS_REC_URL, ocrs_dir / "text-recognition.rten")

    print("baked models:", sorted(p.name for p in MODELS.iterdir()))


if __name__ == "__main__":
    main()
