# [GitButler](https://gitbutler.com)

<img width="200px" src="https://app.gitbutler.com/assets/gb-logo-c5e20a2be4fe4a7d2dbc8b5c0048782608bb5dbc58b7343cd5e7a49183ff961e.svg" />

[![CI][s0]][l0] [![TWEET][s6]][l6]

[s0]: https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/push.yaml/badge.svg
[l0]: https://github.com/gitbutlerapp/gitbutler-client/actions/workflows/push.yaml
[s6]: https://img.shields.io/twitter/follow/gitbutler?label=follow&style=social
[l6]: https://twitter.com/intent/follow?screen_name=gitbutler

**[GitButler](https://gitbutler.com) is a new approach to version control tooling, using Git as a backend**

It is an open source [Tauri](https://tauri.app/)-based
Git client. It's UI is written in [Svelte](https://svelte.dev/) using [TypeScript](https://www.typescriptlang.org)
and it's backend is written in [Rust](https://www.rust-lang.org/).

![gitbutler_client](https://github.com/gitbutlerapp/gitbutler-client/assets/70/89466226-fc0b-4d42-951c-67d95590e00c)

## Downloading a Build

You can download the newest client build from [our downloads page](https://app.gitbutler.com/downloads).

Currently we have builds for Mac and Linux. A Windows build is on the way.

## Why GitButler?

Git's user interface has hardly been touched for 15 years. While it was written
for Linux kernel devs sending patches to each other over mailing lists, most
modern developers have different workflows and needs.

GitButler aims to rethink the version control concept, while still storing data
in Git and being able to push trees to Git servers.

## Main Features

- **Virtual Branches**
  - Organize work on multiple branches simultaneously, rather than constantly switching branches
  - Automatically create new branches when needed
- **Easy Commit Management**
  - Undo, Amend and Squash commits by dragging and dropping
- **GitHub Integration**
  - Authenticate to GitHub to open Pull Requests, list branches and statuses and more
- **Easy SSH Key Management**
  - GitButler can generate an SSH key to upload to GitHub automatically
- **AI Tools**
  - Automatically write commit messages based on your work in progress
  - Automatically create descriptive branch names
- **Commit Signing**
  - Easy commit signing with our generated SSH key

## Documentation

You can find our end user documentation at: https://docs.gitbutler.com

## Bugs and Feature Requests

If you have a bug or feature request, feel free to open an [issue](https://github.com/gitbutlerapp/gitbutler-client/issues/new),
or [join our Discord server](https://discord.gg/wDKZCPEjXC).

## Contributing

So you want to help out? Please check out the [CONTRIBUTING.md](CONTRIBUTING.md)
document.

If you want to skip right to getting the code to actually compile, take a look
at the [DEVELOPMENT.md](DEVELOPMENT.md) file.
