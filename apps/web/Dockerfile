# syntax=docker/dockerfile:1
# Build from repository root: docker build -f apps/web/Dockerfile -t gitbutler-web .
FROM node:24-bookworm-slim AS build
WORKDIR /app
RUN corepack enable

COPY . .
# Skip workspace prepare hooks (including desktop); package web dependencies explicitly.
RUN --mount=type=cache,id=gitbutler-web-pnpm,target=/pnpm/store \
    pnpm --filter @gitbutler/web... --filter root install \
    --frozen-lockfile --ignore-scripts --store-dir=/pnpm/store
RUN pnpm exec turbo run package --filter=@gitbutler/web
RUN pnpm --filter @gitbutler/web build:node
RUN --mount=type=cache,id=gitbutler-web-pnpm,target=/pnpm/store \
    pnpm --filter @gitbutler/web deploy --prod --legacy --ignore-scripts \
    --store-dir=/pnpm/store /runtime

FROM node:24-bookworm-slim AS runtime
WORKDIR /app
ENV NODE_ENV=production \
    HOST=0.0.0.0 \
    PORT=3000
COPY --from=build --chown=node:node /runtime/package.json ./package.json
COPY --from=build --chown=node:node /runtime/node_modules ./node_modules
COPY --from=build --chown=node:node /app/apps/web/build ./build
USER node
EXPOSE 3000
CMD ["node", "build/index.js"]
