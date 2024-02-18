# How to Hack on GitButler

Alrighty, you want to get compiling. We love you already. Your parents raised
you right. Let's get started.

# Overview

So how does this whole thing work?

It's a [Tauri app](https://tauri.app/), which is basically like an Electron app,
in that we can develop a desktop app from one source with multiple OS targets
and write the UI in HTML and Javascript. Except instead of Node for the
filesystem access part, Tauri uses [Rust](https://www.rust-lang.org/).

So everything that hits disk is in Rust, everything that the
user sees is in HTML/JS. Specifically we use [Svelte](https://svelte.dev/)
in Typescript for that layer.

# The Basics

OK, let's get it running.

## Prerequisites

First of all, this is a Tauri app, which is a Rust app. So go install Rust.
The Tauri site has a good primer for the various platforms
[here](https://tauri.app/v1/guides/getting-started/prerequisites).

The next major thing is `pnpm` (because we're a little cooler than people who
use `npm`), so check out how to install that
[here](https://pnpm.io/installation).

## Install dependencies

Next, install the app dependencies.

I hope you have some disk space for 300M of `node_modules`, because this bad
boy will fill er up:

```bash
$ pnpm install
```

You'll have to re-run this occasionally when our deps change.

## Run the app

Now you should be able to run the app in development mode:

```bash
$ pnpm tauri dev
```

By default it will not print debug logs to console. If you want debug logs, set `LOG_LEVEL` environment variable:

```bash
$ LOG_LEVEL=debug pnpm tauri dev
```

## Lint & format

In order to have a PR accepted, you need to make sure everything passes our
Linters, so make sure to run these before submitting. Our CI will shame you
if you don't.

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

# Debugging

Now that you have the app running, here are some hints for debugging whatever
it is that you're working on.

## Logs

The app writes logs into:

1. `stdout` in development mode
2. The Tauri [logs](https://tauri.app/v1/api/js/path/#platform-specific) directory

## Tokio

We are also collecting tokio's runtime tracing information that could be viewed using [tokio-console](https://github.com/tokio-rs/console#tokio-console-prototypes):

- development:
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

# Building

To build the app in production mode, run:

```bash
$ pnpm tauri build --features devtools --config gitbutler-app/tauri.conf.nightly.json
```

This will make an asset similar to our nightly build.

## Building on Windows

Building on Windows is a bit of a tricky process. Here are some helpful tips.

### File permissions

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

1. Right-click on the `nodejs` folder in `Program Files`.
2. Click on `Properties`.
3. Click on the `Security` tab.
4. Click on `Edit` next to "change permissions".
5. Click on `Add`.
6. Type in the name of your user account, or type `Everyone` (case-sensitive).
   Click `Check Names` to verify (they will be underlined if correct).
7. Make sure that `Full Control` is checked under `Allow`.
8. Apply / click OK as needed to close the dialogs.

## Perl

A Perl interpreter is required to be installed in order to configure the `openssl-sys`
crate. We've used [Strawberry Perl](https://strawberryperl.com/) without issue.
Make sure it's installed and `perl` is available on the `PATH` (it is by default
after installation, just make sure to restart the terminal after installing).

Note that it might appear that the build has hung or frozen on the `openssl-sys` crate.
It's not, it's just that Cargo can't report the status of a C/C++ build happening
under the hood, and openssl is _large_. It'll take a while to compile.

# That's It

Now that you're up and running, if you want to change something and open a PR
for us, make sure to read [CONTRIBUTING.md](CONTRIBUTING.md) to make sure you're
not wasting your time.

# Some Other Random Notes

Most of this is for internal GitButler use, but maybe everyone else will find
it interesting too.

## Icon generation

I always forget how to do this, but when we update our app icon, run this to
import it.

```bash
$ pnpm tauri icon path/to/icon.png
```

## Release

Building is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler/actions/workflows/publish.yaml).
Go to the link and select `Run workflow` from the desired branch.

### Versioning

When running the [release action](https://github.com/gitbutlerapp/gitbutler/actions/workflows/publish.yaml),
you will have to choose one of `major`, `minor`, or `patch` release type. Action will generate a new version based on your input and current
version found at `https://app.gitbutler.com/releases`.

### Publishing

To publish a version that you've just build, use [Release Manager](https://gitbutler.retool.com/apps/cb9cbed6-ae0a-11ed-918c-736c4335d3af/Release%20Manager).
