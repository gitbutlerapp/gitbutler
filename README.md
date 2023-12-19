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

By default it will not print debug logs to console. If you want debug logs, set `LOG_LEVEL` environment variable:

```bash
$ LOG_LEVEL=debug pnpm tauri dev
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

### Building on Windows

Building on Windows is a bit of a tricky process. Here are some helpful tips.

#### File permissions

We use `pnpm`, which requires a relatively recent version of Node.js.
Make sure that the latest stable version of Node.js is installed and
on the PATH, and then `npm i -g pnpm`.

This often causes file permissions. First, the AppData folder may not
be present. Be sure to create it if it isn't.

```
mkdir %APPDATA%\npm
```

Secondly, typically folders within `Program Files` are not writable.
You'll need to fix the security permissions for the `nodejs` folder.

> **NOTE:** Under specific circumstances, depending on your usage of
> Node.js, this may pose a security concern. Be sure to understand
> the implications of this before proceeding.

1. Right click on the `nodejs` folder in `Program Files`.
2. Click on `Properties`.
3. Click on the `Security` tab.
4. Click on `Edit` next to "change permissions".
6. Click on `Add`.  
7. Type in the name of your user account, or type `Everyone` (case-sensitive).
   Click `Check Names` to verify (they will be underlined if correct).
8. Make sure that `Full Control` is checked under `Allow`.
8. Apply / click OK as needed to close the dialogs.

### Perl

A Perl interpreter is required to be installed in order to configure the `openssl-sys`
crate. We've used [Strawberry Perl](https://strawberryperl.com/) without issue.
Make sure it's installed and `perl` is available on the `PATH` (it is by default
after installation, just make sure to restart the terminal after installing).

Note that it might appear that the build has hung or frozen on the `openssl-sys` crate.
It's not, it's just that Cargo can't report the status of a C/C++ build happening
under the hood, and openssl is _large_. It'll take a while to compile.

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
