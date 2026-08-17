# Environment for capturing Lite's before/after PR screenshots.
#
# Capture used to run on the GitHub Linux runners and was abandoned: under the
# bare X server there, three of eight surfaces hung indefinitely and every log
# carried "sandboxed_renderer.bundle.js script failed to run". A hosted runner
# gives no way to change how Chromium is sandboxed; a container does, which is
# why the fix is an image rather than another attempt at the same job.
#
# The second thing it buys is commensurability. A before/after pair is only
# meaningful if the two halves were rendered the same way; captured on two
# contributors' machines they differ in font hinting and device scale before the
# UI is even considered. Here both halves share one OS, one font set and one
# rasteriser, so a pixel that differs differs because the UI changed.
#
# Built and driven by capture.sh — see .agents/skills/lite-screenshots/SKILL.md.

FROM mcr.microsoft.com/playwright:v1.58.2-noble

# The base image carries Chromium's shared libraries, a broad font set and Xvfb.
# What it lacks is a Rust toolchain for `but` and the napi SDK, and rsync for
# mirroring the checkout.
#
# The build dependencies are the same ones .github/workflows/lite.yml installs
# for its Linux job; libdbus-1-dev in particular is what libdbus-sys needs, and
# without it the napi SDK fails to compile.
RUN apt-get update && \
	apt-get install -y --no-install-recommends \
	build-essential \
	cmake \
	git \
	libdbus-1-dev \
	libssl-dev \
	libxss-dev \
	pkg-config \
	rsync && \
	apt-get -y autoremove && apt-get -y clean && \
	rm -rf /var/lib/apt/lists/*

# Lite bundles its own webfonts, so system fonts only ever serve as fallback —
# but a missing fallback still shifts a metric somewhere, so pin the set rather
# than inherit whatever the base image happens to ship next.
RUN apt-get update && \
	apt-get install -y --no-install-recommends \
	fonts-dejavu-core \
	fonts-liberation2 \
	fonts-noto-color-emoji && \
	apt-get -y autoremove && apt-get -y clean && \
	rm -rf /var/lib/apt/lists/* && \
	fc-cache -f

# The capture drops to an ordinary user before it runs anything (see
# run-capture.sh for why). These toolchains default to living under /root, which
# that user cannot read, so put them somewhere shared and writable instead —
# rustup fetches the pinned toolchain on first use, and cargo writes its registry
# cache, both as the unprivileged user.
ENV RUSTUP_HOME=/usr/local/rustup \
	CARGO_HOME=/usr/local/cargo \
	COREPACK_HOME=/usr/local/corepack \
	PATH=/usr/local/cargo/bin:${PATH}

# rustup reads rust-toolchain.toml from the mirrored checkout, so the version is
# not pinned here; this only provides the installer.
RUN curl https://sh.rustup.rs -sSf | \
	sh -s -- -y --profile minimal --default-toolchain none --no-modify-path && \
	corepack enable && corepack prepare pnpm@10.20.0 --activate && \
	mkdir -p "${COREPACK_HOME}" && \
	chmod -R a+rwX "${RUSTUP_HOME}" "${CARGO_HOME}" "${COREPACK_HOME}"

# As an environment variable rather than `pnpm config set`, which would write to
# root's .npmrc and be invisible to the user that actually runs the install.
ENV npm_config_store_dir=/pnpm-store

# Both are volume mount points in capture.sh, so neither compiled artefact nor
# downloaded package is paid for twice. CARGO_TARGET_DIR also keeps the Linux
# build out of the mirrored tree, where it would shadow the host's own target/.
ENV CARGO_TARGET_DIR=/cargo-target
# setup.ts reads $BUT before falling back to target/debug/but.
ENV BUT=/cargo-target/debug/but

# Deterministic text shaping and date formatting, independent of the host.
ENV LANG=C.UTF-8
ENV TZ=UTC

# Larger than the 1440x900 viewport the catalogue uses: a screen the exact size
# of the window leaves no room for the window manager's own decorations, and
# Electron then reports a viewport an off-by-a-few pixels smaller.
ENV XVFB_SCREEN=1600x1000x24

# Ships with the Playwright base image. The container still starts as root so
# the entrypoint can settle volume ownership; it drops to this user itself.
ENV RUN_AS_USER=pwuser

WORKDIR /work
COPY run-capture.sh /usr/local/bin/run-capture
RUN chmod +x /usr/local/bin/run-capture
ENTRYPOINT ["/usr/local/bin/run-capture"]
