FROM ivangabriele/tauri:debian-bullseye-18

ENV DOCKER 1
WORKDIR /build

# Copy required files for installing node and pnpm
COPY .nvmrc /build
COPY package.json /build

# Install node from our .nvmrc
RUN curl -sfLS "https://install-node.vercel.app/$(cat .nvmrc)" | bash -s -- --force

# Enable and use our defined `packageManager` version of pnpm
RUN corepack enable pnpm && corepack install

# Mount the source code into the container and build a test binary of GitButler
# tauri-driver requires a binary to instrument and execute, against which
# the tests will eventually run
# TODO: It would be great to be able to avoid building this here, but we need:
# - a binary based on the current state of the source code on the persons machine
# - a binary built for their current arch (i.e. amd64/aarch64)
RUN --mount=type=bind,source=.,target=/build,rw \
  pnpm build:test

WORKDIR /app

# Currently borked :thinking:
# COPY /build/target/release/git-butler-dev /app/git-butler-dev

# TODO: Remove debug helpers
RUN apt install -y vim && \
  echo "alias ll='ls -lah' >> ~/.bashrc"

# CMD xvfb-run pnpm test:e2e
CMD /bin/bash
