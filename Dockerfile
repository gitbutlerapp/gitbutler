FROM ivangabriele/tauri:debian-bullseye-18 AS build

ENV DOCKER 1
WORKDIR /build

COPY .nvmrc /build
COPY package.json /build

# Ensure our defined version of node is installed
RUN curl -sfLS "https://install-node.vercel.app/$(cat .nvmrc)" | bash -s -- --force

# Ensure our defined version of pnpm is installed and used
RUN corepack enable pnpm && corepack install

WORKDIR /app

# COPY ./target/release/git-butler-dev /bin/git-butler-dev

# CMD xvfb-run pnpm test:e2e
CMD /bin/bash
