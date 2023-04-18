## Development

### Prerequisites

[see here](https://tauri.app/v1/guides/getting-started/prerequisites)
for the list of software required to build / develope the app.

### Setup

Then, make sure to install app dependencies:

```bash
$ pnpm install
```

### Run

Now you should be able to run the app in development mode:

```bash
$ pnpm tauri dev
```

## building

To build the app in production mode, run:

```bash
$ pnpm tauri build
```

## Icon generation

```bash
$ pnpm tauri icon path/to/icon.png
```

## Releasing

Building is done via [GitHub Action](https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/publish.yaml).
Go to the link and select `Run workflow` from the desired branch.

### Versioning

When running the [release action](https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/publish.yaml), you will have to choose one of `major`, `minor`, or `patch` release type. Action will generate a new version based on your input and current version found at `https://app.gitbutler.com/releases`.

### publishing

To publish a version that you've just build, use [Release Manager](https://gitbutler.retool.com/apps/cb9cbed6-ae0a-11ed-918c-736c4335d3af/Release%20Manager).

### runners

Note that to build an arm64 macos app, you need to make sure that there is at least one self-hosted runner
with `macos-aarch64` label is online [here](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners).

If you are a lucky owner of an arm64 macos machine, feel free to [run it yourself](https://github.com/gitbutlerapp/gitbutler-client-tauri/settings/actions/runners/new).
Make sure to label it with `macos-aarch64`.
