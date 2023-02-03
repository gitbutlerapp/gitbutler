## prerequisites

[see here](https://tauri.app/v1/guides/getting-started/prerequisites)

## setup

```bash
$ cd src-tauri/binaries && make
$ pnpm install
```

## development

```bash
$ pnpm tauri dev
```

## releasing

Releasing is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler-client-tauri/actions/workflows/publish.yaml).

### runners

Note that to build an arm64 macos app, you need to make sure that there is at least one self-hosted runner
with `macos-aarch64` label is online [here](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners).

If you are a lucky owner of an arm64 macos machine, feel free to [run it yourself](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners/new).
Make sure to label it with `macos-aarch64`.
