#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODELS="${MODELS_BASE:-$ROOT/models}"

if [[ -z "${HF_TOKEN:-}" ]]; then
  echo "HF_TOKEN is required" >&2
  exit 1
fi

export SENTINEL_REPO="${SENTINEL_REPO:-qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF}"
export SENTINEL_QUANT="${SENTINEL_QUANT:-Q8_0}"
export MODELS="$MODELS"

if ! python3 -c "import huggingface_hub, timm, onnx, torchvision" 2>/dev/null; then
  pip install --user torch torchvision --index-url https://download.pytorch.org/whl/cpu
  pip install --user huggingface_hub safetensors numpy timm onnx onnxscript
fi

python3 "$ROOT/docker/bake_models.py"
python3 "$ROOT/docker/extract_cls_head.py" "$MODELS/cls_head.pt" "$MODELS/cls_head.f32.bin"

cat <<EOF

Models downloaded to: $MODELS

Native run example:
  export MODELS_BASE=$MODELS
  export SENTINEL_MODEL_PATH=$MODELS/prompt-injection-jailbreak-sentinel-v2.${SENTINEL_QUANT}.gguf
  export SENTINEL_CLS_HEAD_PATH=$MODELS/cls_head.f32.bin
  export NSFW_TEXT_MODEL_DIR=$MODELS/nsfw-text
  export NSFW_IMAGE_MODEL_DIR=$MODELS/nsfw-image
  export WOLF_MODEL_DIR=$MODELS/wolf-defender
  export OCRS_MODEL_DIR=$MODELS/ocrs
  export WOLF_MAX_LENGTH=512
  cargo run --release --bin trypanophobe
EOF
