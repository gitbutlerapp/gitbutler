# Development

## Prerequisites

[see here](https://tauri.app/v1/guides/getting-started/prerequisites)
for the list of software required to build / develope the app.

### Setup

Then, make sure to install app dependencies:

```bash
$ pnpm install
```

### Run the app

Now you should be able to run the app in development mode:

```bash
$ pnpm tauri dev
```

By default it will not print debug logs to console. If you want debug logs, use `debug` feature:

```bash
$ pnpm tauri dev --features debug
```

### Run Stories

Stories is our easy way to view our app components. Running the following command will launch in your default browser.

```bash
$ pnpm story:dev
```

### Lint & format

Javascript:

```bash
$ pnpm lint
$ pnpm format
```

Rust:

```bash
$ cargo clippy   # see linting errors
$ cargo fmt      # format code
```

## Debug

### Logs

App writes logs into:

1. stdout in development mode
2. [Logs](https://tauri.app/v1/api/js/path/#platform-specific) directory

### Tokio

We are also collecting tokio's runtime tracing information that could be viewed using [tokio-console](https://github.com/tokio-rs/console#tokio-console-prototypes):

- developlent:
  ```bash
  $ tokio-console
  ```
- nightly:
  ```bash
  $ tokio-console http://127.0.0.1:6668
  ```
- production:
  ```bash
  $ tokio-console http://127.0.0.1:6667
  ```

## Build

To build the app in production mode, run:

```bash
$ pnpm tauri build --features devtools --config packages/tauri/tauri.conf.nightly.json
```

This will make an asset similar to our nightly build.

## Icon generation

```bash
$ pnpm tauri icon path/to/icon.png
```

## Release

Building is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/publish.yaml).
Go to the link and select `Run workflow` from the desired branch.

### Versioning

When running the [release action](https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/publish.yaml),
you will have to choose one of `major`, `minor`, or `patch` release type. Action will generate a new version based on your input and current
version found at `https://app.gitbutler.com/releases`.

### Publishing

To publish a version that you've just build, use [Release Manager](https://gitbutler.retool.com/apps/cb9cbed6-ae0a-11ed-918c-736c4335d3af/Release%20Manager).
