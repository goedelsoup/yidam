# The image every pod of a generated workflow runs: the `yidam` binary, git, and an SSH client.
#
# Nothing else. A pod clones a bundle into scratch, does one act, and exits — there is no
# checkout to keep warm and no daemon to run. The build stage is the CLI crate alone, so the
# image is rebuilt when the binary changes and for no other reason.
#
#   docker build -f yidam/cluster/Dockerfile -t ghcr.io/you/yidam-cluster:<version> .
#
# `.github/workflows/cluster-image.yml` builds it on every pull request that could break it
# and publishes it as `ghcr.io/goedelsoup/yidam-cluster:<version>` on each `cli/v*` tag.
#
# Built from the repository root, because the CLI crate has two path dependencies beside it:
# `yidam/sdks/rust` and `yidam/tests/harness/ci-report`. `Dockerfile.dockerignore`
# beside this file keeps every `target/` and `node_modules/` out of the context.

# The exact toolchain `mise.toml` pins, not `1.88`: that tag floats to the newest patch, and a
# published image would then be built by a compiler no gate here ran. `toolchain_pins.rs`
# reads both files and fails when they disagree.
FROM rust:1.88.0-bookworm AS build
WORKDIR /src
COPY . .
# `calculators-gluon`, because a pod runs whatever arm the manifest declares: an image
# without it refuses a whole plan that holds one typed calculator. `catalog-fetch`, because
# every workflow runs the built-in `catalog-fetch` step, and without the feature a `url`
# location is a refusal rather than a request. `--locked` for the reason release.yml gives:
# built from anything but the committed Cargo.lock, the image is not what its commit describes.
RUN cargo build --release --locked --manifest-path yidam/cli/Cargo.toml --bin yidam \
      --features calculators-gluon,catalog-fetch

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends git openssh-client ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/yidam/cli/target/release/yidam /usr/local/bin/yidam
# The records pods write for each other, and the scratch every pod clones into.
RUN mkdir -p /tmp/yidam && chmod 1777 /tmp/yidam
# Not root: the pod needs to write scratch and read two mounts, and nothing else. The generated
# workflow asks the cluster to hold the pod to this uid, with a read-only root (#1230).
RUN useradd --create-home --uid 1000 yidam
USER yidam
ENTRYPOINT ["yidam"]
