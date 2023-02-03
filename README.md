## development

### prerequisites

[see here](https://tauri.app/v1/guides/getting-started/prerequisites)
for the list of software required to build / develope the app.

### setup

then, make sure to install app dependencies:

```bash
$ cd src-tauri/binaries && make
$ pnpm install
```

### run

now you should be able to run the app in development mode:

```bash
$ pnpm tauri dev
```

## building

to build the app in production mode, run:

```bash
$ cd src-tauri/binaries && make
$ pnpm tauri build
```

## releasing

Releasing is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler-client-tauri/actions/workflows/publish.yaml).

### runners

Note that to build an arm64 macos app, you need to make sure that there is at least one self-hosted runner
with `macos-aarch64` label is online [here](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners).

If you are a lucky owner of an arm64 macos machine, feel free to [run it yourself](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners/new).
Make sure to label it with `macos-aarch64`.
