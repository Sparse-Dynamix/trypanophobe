#!/usr/bin/env python3
"""Build-only: cls_head.pt -> cls_head.f32.bin (rows=2, cols=n_embd, row-major f32)."""
import struct
import sys

import numpy as np
import torch


def main() -> None:
    src, dst = sys.argv[1], sys.argv[2]
    obj = torch.load(src, map_location="cpu", weights_only=False)
    t = (obj["weight"] if isinstance(obj, dict) else obj).detach().float().numpy()
    rows, cols = int(t.shape[0]), int(t.shape[1])
    with open(dst, "wb") as out:
        out.write(struct.pack("<II", rows, cols))
        out.write(t.astype(np.float32).tobytes())
    print(f"wrote {dst} shape=({rows}, {cols})")


if __name__ == "__main__":
    main()
