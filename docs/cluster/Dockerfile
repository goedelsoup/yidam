# The image every pod of a generated workflow runs: the `yidam` binary, git, and an SSH client.
#
# Nothing else. A pod clones a bundle into scratch, does one act, and exits — there is no
# checkout to keep warm and no daemon to run. The build stage is the CLI crate alone, so the
# image is rebuilt when the binary changes and for no other reason.
#
#   docker build -f docs/cluster/Dockerfile -t ghcr.io/you/yidam-cluster:<version> .
#
# Built from the repository root, because the CLI crate has two path dependencies beside it:
# `yidam/prelude/sdks/rust` and `yidam/tests/harness/ci-report`. `Dockerfile.dockerignore`
# beside this file keeps every `target/` and `node_modules/` out of the context.

FROM rust:1.88-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release --manifest-path yidam/cli/Cargo.toml --bin yidam

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends git openssh-client ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/yidam/cli/target/release/yidam /usr/local/bin/yidam
# The records pods write for each other, and the scratch every pod clones into.
RUN mkdir -p /tmp/yidam && chmod 1777 /tmp/yidam
# Not root: the pod needs to write scratch and read two mounts, and nothing else.
RUN useradd --create-home --uid 1000 yidam
USER yidam
ENTRYPOINT ["yidam"]
