# syntax=docker/dockerfile:1

ARG SENTINEL_QUANT=Q8_0
ARG SENTINEL_REPO=qualifire/prompt-injection-jailbreak-sentinel-v2-GGUF
ARG RUST_VERSION=1.85
ARG PIHOLE_VERSION=2025.05.0

FROM public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 AS lambda-adapter

FROM ubuntu:24.04 AS rust-builder
ARG RUST_VERSION
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates build-essential clang cmake pkg-config libssl-dev g++ \
    && curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain "${RUST_VERSION}" \
    && rm -rf /var/lib/apt/lists/*
ENV PATH="/root/.cargo/bin:${PATH}"
WORKDIR /build
COPY Cargo.toml rust-toolchain.toml ./
COPY config ./config
COPY src ./src
ENV CARGO_PROFILE_RELEASE_LTO=true
ENV CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
RUN cargo build --release --bin trypanophobe \
    && strip target/release/trypanophobe \
    && LLAMA_LIBDIR="$(find target/llama-cmake-cache -type f -name 'libllama.so.0.0.0' -printf '%h\n' | head -1)" \
    && mkdir -p /llama-libs \
    && cp -a "${LLAMA_LIBDIR}"/lib*.so* /llama-libs/

FROM python:3.12-slim-bookworm AS model-assets
ARG SENTINEL_QUANT
ARG SENTINEL_REPO
ENV SENTINEL_QUANT=${SENTINEL_QUANT}
ENV SENTINEL_REPO=${SENTINEL_REPO}
RUN apt-get update && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN pip install --no-cache-dir torch torchvision --index-url https://download.pytorch.org/whl/cpu \
    && pip install --no-cache-dir huggingface_hub safetensors numpy timm onnx onnxscript
COPY docker/extract_cls_head.py docker/bake_models.py /tmp/
RUN --mount=type=secret,id=hf_token \
    export HF_TOKEN="$(cat /run/secrets/hf_token)" && \
    test -n "$HF_TOKEN" && \
    python3 /tmp/bake_models.py
RUN python3 /tmp/extract_cls_head.py /models/cls_head.pt /models/cls_head.f32.bin

FROM pihole/pihole:${PIHOLE_VERSION} AS pihole-gravity
RUN printf '%s\n' \
    'https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts' \
    > /etc/pihole/adlists.list && \
    pihole -g

FROM ubuntu:24.04 AS runtime
ARG SENTINEL_QUANT

COPY --from=lambda-adapter /lambda-adapter /opt/extensions/lambda-adapter

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl ca-certificates supervisor openssl sqlite3 \
    libssl3 libgomp1 libstdc++6 \
    libreoffice-writer libreoffice-calc libreoffice-impress \
    imagemagick tesseract-ocr tesseract-ocr-eng \
    && rm -rf /var/lib/apt/lists/*

COPY --from=pihole-gravity /usr/bin/pihole-FTL /usr/bin/pihole-FTL
COPY --from=pihole-gravity /usr/local/bin/pihole /usr/local/bin/pihole
COPY --from=pihole-gravity /opt/pihole /opt/pihole
COPY --from=pihole-gravity /etc/pihole /etc/pihole
COPY config/pihole.toml /etc/pihole/pihole.toml
RUN groupadd -r pihole 2>/dev/null || true \
    && useradd -r -g pihole -d /etc/pihole -s /usr/sbin/nologin pihole 2>/dev/null || true \
    && mkdir -p /etc/trypanophobe \
    && chown -R pihole:pihole /etc/pihole \
    && rm -rf /opt/pihole/web /opt/pihole/html \
    && smoke_host="$(sqlite3 /etc/pihole/gravity.db "SELECT domain FROM gravity WHERE domain NOT IN ('localhost','localhost.localdomain') AND domain LIKE '%.%' LIMIT 1;")" \
    && test -n "${smoke_host}" \
    && printf 'https://%s/\n' "$smoke_host" > /etc/trypanophobe/smoke-blocked-url

COPY --from=rust-builder /build/target/release/trypanophobe /usr/local/bin/trypanophobe
COPY --from=rust-builder /llama-libs/ /usr/local/lib/
RUN ldconfig
COPY --from=model-assets /models /opt/trypanophobe/models

COPY config/env.defaults /etc/trypanophobe/env.defaults
COPY docker/setup-pihole-log-fifos.sh /usr/local/bin/setup-pihole-log-fifos.sh
COPY docker/entrypoint.sh /usr/local/bin/entrypoint.sh
COPY supervisor/supervisord.conf /etc/trypanophobe/supervisord.conf
RUN chmod +x /usr/local/bin/setup-pihole-log-fifos.sh /usr/local/bin/entrypoint.sh

RUN printf 'SENTINEL_MODEL_PATH=/opt/trypanophobe/models/prompt-injection-jailbreak-sentinel-v2.%s.gguf\n' \
    "${SENTINEL_QUANT}" > /etc/trypanophobe/build.env

ENV AWS_LWA_READINESS_CHECK_PATH=/api/health
ENV AWS_LWA_READINESS_CHECK_HEALTHY_STATUS=200-499
ENV AWS_LWA_ASYNC_INIT=true

EXPOSE 8080/tcp
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
