# syntax=docker/dockerfile:1.7

# opsgate의 하우스 Dockerfile 패턴: cargo-chef + sccache로 의존성 레이어를
# 캐시하고, 런타임은 Distroless + 비루트 사용자 + 바이너리만 담는다.

FROM rust:1.96.1-bookworm@sha256:a339861ae23e9abb272cea45dfafde21760d2ce6577a70f8a926153677902663 AS chef

WORKDIR /app
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        cmake \
        g++ \
        make \
        perl \
        pkg-config \
    && rm -rf /var/lib/apt/lists/*
RUN --mount=type=cache,id=grove-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=grove-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    cargo install cargo-chef --version 0.1.78 --locked \
    && cargo install sccache --version 0.18.0 --locked --no-default-features \
    && cargo install cargo-auditable --version 0.7.6 --locked

ENV RUSTC_WRAPPER=sccache \
    SCCACHE_DIR=/sccache \
    CARGO_INCREMENTAL=0

FROM chef AS planner

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY backend ./backend
RUN cargo chef prepare --recipe-path recipe.json

FROM node:22.23.2-bookworm-slim@sha256:48e4b67d85f87bd551df43704e24d252f56cc5f8e9718841aace50f19948f0f9 AS web-builder

WORKDIR /app/frontend/web
COPY frontend/web/package.json frontend/web/package-lock.json ./
RUN --mount=type=cache,id=grove-npm-cache,target=/root/.npm,sharing=locked \
    npm ci && npm audit --audit-level=high
COPY frontend/web ./
RUN npm run build

FROM chef AS builder

COPY --from=planner /app/recipe.json recipe.json
# sqlx query! 매크로를 쓰기 시작하면 SQLX_OFFLINE=true + .sqlx 메타데이터 COPY가 여기 들어온다.
RUN --mount=type=cache,id=grove-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=grove-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=grove-sccache,target=/sccache,sharing=locked \
    --mount=type=cache,id=grove-target-release,target=/app/target,sharing=locked \
    cargo chef cook --release --locked --recipe-path recipe.json
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY backend ./backend
RUN --mount=type=cache,id=grove-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=grove-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=grove-sccache,target=/sccache,sharing=locked \
    --mount=type=cache,id=grove-target-release,target=/app/target,sharing=locked \
    cargo auditable build --release --locked --bin grove-storage \
    && cp /app/target/release/grove-storage /usr/local/bin/grove-storage

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2 AS runtime

WORKDIR /app
COPY --from=builder /usr/local/bin/grove-storage /usr/local/bin/grove-storage
COPY --from=web-builder /app/frontend/web/dist /app/web
COPY --from=web-builder /app/frontend/web/package-lock.json /app/inventory/frontend/package-lock.json

# 설정은 전부 env다 (ADR 004): 서버 설정과 자격증명은 배포 env/ESO가 주고,
# 등록부(storages·clients·credentials)는 DB에 산다 — 이미지에 설정 파일이 없다.
ENV GROVE_BIND=0.0.0.0:8080
ENV GROVE_CONSOLE_DIST_DIR=/app/web
EXPOSE 8080

USER 10001:10001
ENTRYPOINT ["/usr/local/bin/grove-storage"]
