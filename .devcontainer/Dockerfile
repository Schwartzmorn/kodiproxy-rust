FROM rust:bookworm

ARG UID=1000
ARG GID=1000

RUN groupadd --gid ${GID} dev \
  && useradd \
  --uid ${UID} \
  --gid ${GID} \
  --create-home \
  --shell /bin/bash \
  dev

RUN apt-get update \
  && apt-get install -y --no-install-recommends \
  git \
  ripgrep \
  libcec-dev \
  libdbus-1-dev \
  && rm -rf /var/lib/apt/lists/*

USER dev
WORKDIR /workspace

RUN rustup component add rustfmt clippy

ENV CARGO_HOME=/home/dev/.cargo

CMD ["bash"]
