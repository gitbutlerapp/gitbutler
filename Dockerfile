FROM ivangabriele/tauri:debian-bullseye-18

ENV DOCKER 1
WORKDIR /build

# Copy source code into container
COPY . /build

# Install specific node version from our .nvmrc
RUN curl -sfLS "https://install-node.vercel.app/$(cat .nvmrc)" | bash -s -- --force

# Enable and use our defined `packageManager` version of pnpm
RUN corepack enable pnpm \
  && corepack install \
  && pnpm install

# Build a binary of the application
RUN pnpm build:test

# CMD xvfb-run pnpm test:e2e
CMD /bin/bash
