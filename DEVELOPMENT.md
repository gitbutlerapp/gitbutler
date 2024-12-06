# How to Hack on GitButler

Alrighty, you want to get compiling. We love you already. Your parents raised
you right. Let's get started.

---

## Table of Contents

- [Overview](#overview)
- [The Basics](#the-basics)
  - [Prerequisites](#prerequisites)
  - [Install dependencies](#install-dependencies)
  - [Run the app](#run-the-app)
  - [Lint & format](#lint--format)
- [Debugging](#debugging)
  - [Logs](#logs)
  - [Tokio](#tokio)
- [Building](#building)
  - [Building on Windows](#building-on-windows)
    - [File permissions](#file-permissions)
    - [Perl](#perl)
    - [Crosscompilation](#crosscompilation)
- [Design](#design)
- [Contributing](#contributing)
- [Some Other Random Notes](#some-other-random-notes)
  - [Icon generation](#icon-generation)
  - [Release](#release)
  - [Versioning](#versioning)
  - [Publishing](#publishing)
- [Development mode OAuth login](#development-mode-oauth-login)
- [Joining the GitButler Team](#joining-the-gitbutler-team)

---

## Overview

So how does this whole thing work?

It's a [Tauri app](https://tauri.app/), which is basically like an Electron app,
in that we can develop a desktop app from one source with multiple OS targets
and write the UI in HTML and Javascript. Except instead of Node for the
filesystem access part, Tauri uses [Rust](https://www.rust-lang.org/).

So everything that hits disk is in Rust, everything that the
user sees is in HTML/JS. Specifically we use [Svelte](https://svelte.dev/)
in Typescript for that layer.

---

## The Basics

OK, let's get it running.

### Prerequisites

First of all, this is a Tauri app, which uses Rust for the backend and Javascript for the frontend. So let's make sure you have all the prerequisites installed.

1. Tauri Dev Deps (https://tauri.app/start/prerequisites/#system-dependencies)

On Mac OS, ensure you've installed XCode and `cmake`. On Linux, if you're on Debian or one of its derivatives like Ubuntu, you can use the following command.

<details>
<summary>Linux Tauri dependencies</summary>

```bash
$ sudo apt update
$ sudo apt install libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev \
  cmake
```

</details>

2. Rust

For both Mac OS and Linux, you can use the following `rustup` quick install script to get all the necessary tools.

```bash
$ cd gitbutler-repo
$ curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh
```

3. Node

Next, ensure you've got at least Node 20 installed. If you're on Mac OS or Linux and you're missing `node`, you can use your favorite package manager like `brew` or `apt`.

Alternatively, you can use the following Node installer from Vercel to get the latest version.

```bash
$ curl https://install-node.vercel.app/latest > install_node.sh
$ sudo ./install_node.sh
```

4. pnpm

Finally, we use `pnpm` as our javascript package manager. You can leverage `corepack`, which comes shipped with `node`, to install and use the `pnpm` version we defined in our `package.json`.

```bash
$ cd gitbutler-repo
$ corepack enable
```

### Install dependencies

Next, install the app dependencies.

I hope you have some disk space for 300M of `node_modules`, because this bad
boy will fill er up:

```bash
$ pnpm install # This should now ask you to confirm the download, installation, and use of pnpm via corepack
```

You'll have to re-run this occasionally when our deps change.

> [!NOTE]  
> We use [turborepo](https://turbo.build/repo) as our monorepo tooling and by default Vercel collects some [basic telemetry](https://turbo.build/repo/docs/telemetry). If you'd like to disable this, please run `pnpm exec turbo telemetry disable` once in the project's root directory after installing dependencies.

### Run the app

First, run cargo build such that supplementary bins such as `gitbutler-git-askpass` and `gitbutler-git-setsid` are created:

```bash
$ pnpm build:desktop
```

Now you should be able to run the app in development mode:

```bash
$ pnpm dev:desktop
```

By default it will not print debug logs to console. If you want debug logs, set `LOG_LEVEL` environment variable:

```bash
$ LOG_LEVEL=debug pnpm dev:desktop
```

### Lint & format

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

---

## Debugging

Now that you have the app running, here are some hints for debugging whatever
it is that you're working on.

### Logs

The app writes logs into:

1. `stdout` in development mode
2. The Tauri [logs](https://tauri.app/v1/api/js/path/#platform-specific) directory

One can get performance log when launching the application locally as follows:

```bash
GITBUTLER_PERFORMANCE_LOG=1 LOG_LEVEL=debug pnpm tauri dev
```

For more realistic performance logging, use local release builds with `--release`.

```bash
GITBUTLER_PERFORMANCE_LOG=1 LOG_LEVEL=debug pnpm tauri dev --release
```

Since release builds are configured for public releases, they are very slow to compile.
Speed them up by sourcing the following file.

```bash
export CARGO_PROFILE_RELEASE_DEBUG=0
export CARGO_PROFILE_RELEASE_INCREMENTAL=false
export CARGO_PROFILE_RELEASE_LTO=false
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=256
export CARGO_PROFILE_RELEASE_OPT_LEVEL=2
```

### Tokio

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

---

## Building

To build the app in production mode, run:

```bash
$ pnpm tauri build --features devtools --config crates/gitbutler-tauri/tauri.conf.nightly.json
```

This will make an asset similar to our nightly build.

### Building on Windows

Building on Windows is a bit of a tricky process. Here are some helpful tips.

#### Nightly Compiler

As a few crates require nightly features on Windows, a `rust-toolchain.toml` is provided
to have rustup use the right compiler version.

If for some reason this cannot be used or doesn't kick-in, one can also set an override.

```shell
rustup override add nightly-2024-07-01
```

If a stable nightly isn't desired or necessary, the latest nightly compiler can also be used:

```shell
rustup override add nightly
```

#### File permissions

We use `pnpm`, which requires a relatively recent version of Node.js.
Make sure that the latest stable version of Node.js is installed and
on the PATH, and then `npm i -g pnpm`.

Sometimes npm's prefix is incorrect on Windows, we can check this via:

```sh
npm config get prefix
```

If it's not `C:\Users\<username>\AppData\Roaming\npm` or another folder that is
normally writable, then we can set it in Powershell:

```sh
mkdir -p $APPDATA\npm
npm config set prefix $env:APPDATA\npm
```

Afterwards, add this folder to your PATH.

#### Perl

A Perl interpreter is required to be installed in order to configure the `openssl-sys`
crate. We've used [Strawberry Perl](https://strawberryperl.com/) without issue.
Make sure it's installed and `perl` is available on the `PATH` (it is by default
after installation, just make sure to restart the terminal after installing).
[Scoop](https://scoop.sh/) users can install this via `scoop install perl`.

Note that it might appear that the build has hung or frozen on the `openssl-sys` crate.
It's not, it's just that Cargo can't report the status of a C/C++ build happening
under the hood, and openssl is _large_. It'll take a while to compile.

#### Crosscompilation

This paragraph is about crosscompilation to x86_64-MSVC from ARM Windows,
a configuration typical for people with Apple Silicon and Parallels VMs,
which only allow ARM Windows to be used.

The `windows` dependency on `gitbutler-git` doesn't currently compile on ARM,
which means cross-compilation to x86-64 is required to workaround that. Besides,
most users will probably still be on INTEL machines, making this capability
a common requirement.

In a Git `bash`, _with MSVC for x86-64 installed on the system_, run the following
to prepare the environment.

```bash
export TRIPLE_OVERRIDE=x86_64-pc-windows-msvc
export CARGO_BUILD_TARGET=x86_64-pc-windows-msvc
export OPENSSL_SRC_PERL="c:/Strawberry/perl/bin/perl.exe"
```

Here is how to produce a nightly release build:

```
pnpm tauri build --features windows,devtools --config  crates/gitbutler-tauri/tauri.conf.nightly.json
```

And this is how to get a local developer debug build:

```bash
pnpm tauri dev --features windows --target x86_64-pc-windows-msvc
```

Note that it's necessary to repeat the `--target` specification as otherwise the final copy operation doesn't work,
triggered by `tauri` itself.

---

## Design

We use [Figma](https://www.figma.com/) for our design work.
If you're a designer (and even if you're not), you want to contribute to the
design of GitButler, or your work involves UI, you could duplicate our design file.

GitButler design: [Figma file](https://www.figma.com/file/FbeLt0yjY9RiNn8MXUXsYs/Client-Design?type=design&node-id=0%3A1&mode=design&t=MUDQhR3iOM3DpI9m-1) 🎨

---

## Contributing

Now that you're up and running, if you want to change something and open a PR
for us, make sure to read [CONTRIBUTING.md](CONTRIBUTING.md) to make sure you're
not wasting your time.

---

## Some Other Random Notes

Most of this is for internal GitButler use, but maybe everyone else will find
it interesting too.

---

### Icon generation

I always forget how to do this, but when we update our app icon, run this to
import it.

```bash
$ pnpm tauri icon path/to/icon.png
```

### Release

Building is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler/actions/workflows/publish.yaml).
Go to the link and select `Run workflow` from the desired branch.

### Versioning

When running the [release action](https://github.com/gitbutlerapp/gitbutler/actions/workflows/publish.yaml),
you will have to choose one of `major`, `minor`, or `patch` release type. Action will generate a new version based on your input and current
version found at `https://app.gitbutler.com/releases`.

### Publishing

To publish a version that you've just build, use [Release Manager](https://gitbutler.retool.com/apps/cb9cbed6-ae0a-11ed-918c-736c4335d3af/Release%20Manager).

---

## Development mode OAuth login

By default, you will not be able to log into GitButler using Github/Google because the base url does not match. To be able to do this add ( or update ) the following line to your `.env.development` file. You will need to create the file if it does not exist.

```
PUBLIC_API_BASE_URL=https://app.gitbutler.com/
```

---

## Joining the GitButler Team

If you are interested in joining our small but tightly knit engineering team, we are currently looking for the following roles:

- [Senior Rust developer](https://gitbutler.homerun.co/senior-rust-developer) (Onsite Berlin)
- [Senior TypeScript developer](https://gitbutler.homerun.co/senior-typescript-developer) (Onsite Berlin)
- [Senior Rails developer](https://gitbutler.homerun.co/senior-rails-developer) (Onsite Berlin)

## Code Hitlist

This is a list of crates/modules that we want to eliminate or split into smaller crates:

- [gitbutler-reference](crates/gitbutler-reference/) (just bad)
- [gitbutler-storage](crates/gitbutler-storage/) (legacy way of dealing with files)
- [gitbutler-branch-actions](crates/gitbutler-branch-actions/) (contains functionality outside of the virtual branch domain (e.g. commit actions etc.))
- [gitbutler-repository](crates/gitbutler-repository/)
- [gitbutler-branch](crates/gitbutler-branch/) (contains `diff` and `branch` contexts due to a cyclic dependency)
- [gitbutler-url](crates/gitbutler-url/) (this is a huge mess and ideally we need none of it)
- [gitbutler_repo::config](crates/gitbutler-repo/src/config.rs) (seems like the wrong abstraction)
- [gitbutler-config](crates/gitbutler-config) (this provides an API for the UI layer to read and write git config and we want none of that)
- [gitbutler_virtual::assets](crates/gitbutler-branch-actions/src/assets.rs) (this is a caching of things like favicons and it's clearly a UI concern that doesn't belong here)
