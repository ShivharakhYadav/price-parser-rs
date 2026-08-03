# One command to a runnable artifact that proves the port.
#
#   docker build -t price-parser-rs .
#   docker run --rm price-parser-rs
#
# The default command does not merely start something -- it re-establishes the
# claim: the vendored tests are unmodified, the Rust builds and passes its own
# tests, the original suite passes against it, and the hashes still hold
# afterwards. A judge needs no toolchain of their own.

FROM rust:1-slim-bookworm

# python3-dev is needed to build the extension module; python3-venv for the
# isolated environment the tests run in.
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        python3 \
        python3-venv \
        python3-dev \
    && rm -rf /var/lib/apt/lists/*

# maturin resolves the interpreter from VIRTUAL_ENV, so the venv goes on PATH
# rather than being activated per-command.
ENV VIRTUAL_ENV=/opt/venv
ENV PATH="/opt/venv/bin:$PATH"
RUN python3 -m venv "$VIRTUAL_ENV" \
    && pip install --no-cache-dir --upgrade pip \
    && pip install --no-cache-dir maturin pytest

WORKDIR /port
COPY . .

# --locked so the image builds the dependency versions that were actually
# tested, rather than whatever resolves today.
RUN cargo build --release --locked \
    && cargo test --release --locked --no-run \
    && maturin develop --release

# Ordering is the point. Hashes are verified before anything is claimed and
# again at the end, so a suite that had been edited could not produce a green
# run in between.
CMD ["sh", "-euc", "\
python tools/verify_hashes.py && \
cargo test --release --locked && \
python -m pytest -q && \
python tools/verify_hashes.py && \
echo && echo 'Original suite passed unmodified, hashes intact.'"]
