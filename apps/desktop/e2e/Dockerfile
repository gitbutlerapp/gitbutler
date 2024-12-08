# Container for running E2E tests on MacOS
#
# 1. docker build --tag gitbutler/e2e:latest
# 2. docker run --name e2e-agent --detach -p 2222:22 -v $GITBUTLER_REPO:/root/gitbutler \
#     -v /tmp/e2e-pnpm-store:/tmp/pnpm-store gitbutler/e2e:latest
# 3. docker cp -a ~/.ssh/id_ed25519.pub e2e-agent:/root/.ssh/authorized_keys
# 4. ssh -p 2222 -Y root@localhost
# 4.5 Fix .ssh/ permissions with chmod and chgrp
# The rest is run inside the container in your SSH session:
# 5. cd /root/gitbutler
# 6. pnpm install && cargo build
# 7. cargo build -p gitbutler-git && cargo build -p gitbutler-cli
# 8. pnpm build:test
# 9. pnpm test:e2e

FROM ubuntu:24.04

RUN apt update && \
  apt install -y \
  build-essential \
  curl \
  git \
  pkg-config \
  psmisc \
  vim \
  # Required for release builds
  jq  \
  file && \
  # Clean up
  apt -y autoremove && apt -y clean

# Install Tauri dependencies
# https://v2.tauri.app/start/prerequisites/#linux
RUN apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  libssl-dev \
  libgtk-3-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev && \
  # Clean up
  apt -y autoremove && apt -y clean

# Install tauri-driver dependencies
RUN apt install -y \
  dbus-x11 \
  webkit2gtk-driver \
  xvfb && \
  # Clean up
  apt -y autoremove && apt -y clean

# Install ssh server for X11 forwarding
RUN apt install -y openssh-server && \
  # Needed for recording test execution.
  apt install -y ffmpeg && \
  # Install gitbutler dependencies
  apt install -y cmake && \
  # Clean up
  apt -y autoremove && apt -y clean

# Needed by at least pnpm.
ENV SHELL=bash

# Install rust.
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install node.
RUN curl -fsSL "https://deb.nodesource.com/setup_20.x" | bash - && \
  apt install -y nodejs && \
  corepack enable

# Install pnpm and configure store directory.
ENV PNPM_HOME=/tmp/.pnpm
RUN wget -qO- https://get.pnpm.io/install.sh | ENV="$HOME/.bashrc" SHELL="$(which bash)" bash -
# Home/store dir should be mounted from the host.
RUN pnpm config set store-dir /tmp/.pnpm
ENV PATH="/tmp/.pnpm:${PATH}"
RUN pnpm setup

# Used to manage build dependencies between packages.
RUN pnpm add --global turbo && \
  # Used as a proxy for communicating with WebKitWebDriver.
  cargo install tauri-driver && \
  # vi bindings on by default.
  echo "set -o vi" >> /root/.bashrc

# Run SSH server in foreground.
CMD ["service", "ssh", "start", "-D"]

