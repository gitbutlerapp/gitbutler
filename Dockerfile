# syntax=docker/dockerfile:1.7-labs

FROM ivangabriele/tauri:debian-bullseye-18 AS build

ENV DOCKER 1
WORKDIR /build

COPY .nvmrc /build
COPY package.json /build

RUN curl -sfLS "https://install-node.vercel.app/$(cat .nvmrc)" | bash -s -- --force

RUN corepack enable pnpm && corepack install

RUN --mount=type=bind,source=.,target=/build,rw \
  pnpm build:test

FROM scratch

LABEL version="0.0.1"
LABEL description="GitButler Darwin E2E Test Environment"

WORKDIR /app

COPY --from=build /build/target/release/git-butler-dev /app/git-butler-dev

# Debug niceties
RUN apt install -y vim && \
  echo "alias ll='ls -lah' >> ~/.bashrc"

# COPY ./target/release/git-butler-dev /bin/git-butler-dev

# CMD xvfb-run pnpm test:e2e
CMD /bin/bash
