FROM rust:1-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        bash \
        ca-certificates \
        curl \
        file \
        git \
        gh \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace/cli

ENV PATH=/usr/local/cargo/bin:${PATH}
ENV CARGO_TARGET_DIR=/workspace/cli/target

CMD ["/bin/bash"]
