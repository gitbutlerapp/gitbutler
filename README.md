<p align="center">
  <p align="center">
   <img width="128px" src="gitbutler-app/icons/128x128@2x.png" />
  </p>
	<h1 align="center"><b>GitButler Client</b></h1>
	<p align="center">
		Git based Version Control graphical client, built from the ground up for modern workflows
    <br />
    <a href="https://gitbutler.com"><strong>gitbutler.com »</strong></a>
    <br />
    <br />
    <b>Download for </b>
    macOS (<a href="https://app.gitbutler.com/downloads/release/darwin/aarch64/dmg">Apple Silicon</a> |
      <a href="https://app.gitbutler.com/downloads/release/darwin/x86_64/dmg">Intel</a>) ·
		Linux (<a href="https://app.gitbutler.com/downloads/release/linux/x86_64/gz">AppImage</a> |
       <a href="https://app.gitbutler.com/downloads/release/linux/x86_64/deb">deb</a>)
    <br />
    <i>~ Link for Windows will be added once a release is available. ~</i>
  </p>
</p>

<br/>

![gitbutler_client](https://github.com/gitbutlerapp/gitbutler/assets/70/89466226-fc0b-4d42-951c-67d95590e00c)

[![CI][s0]][l0] [![TWEET][s1]][l1] [![DISCORD][s2]][l2] [![INSTA][s3]][l3] [![YOUTUBE][s5]][l5]

[s0]: https://github.com/gitbutlerapp/gitbutler/actions/workflows/push.yaml/badge.svg
[l0]: https://github.com/gitbutlerapp/gitbutler/actions/workflows/push.yaml
[s1]: https://img.shields.io/badge/Twitter-black?logo=x&logoColor=white
[l1]: https://twitter.com/intent/follow?screen_name=gitbutler
[s2]: https://img.shields.io/discord/1060193121130000425?label=Discord&color=5865F2
[l2]: https://discord.gg/MmFkmaJ42D
[s3]: https://img.shields.io/badge/Instagram-E4405F?logo=instagram&logoColor=white
[l3]: https://instagram.com/gitbutlerapp
[s5]: https://img.shields.io/youtube/channel/subscribers/UCQiEMslIPy6ylW_TJXZ7nUQ
[l5]: https://www.youtube.com/@gitbutlerapp

GitButler is an open source [Tauri](https://tauri.app/)-based
Git client. It's UI is written in [Svelte](https://svelte.dev/) using [TypeScript](https://www.typescriptlang.org)
and it's backend is written in [Rust](https://www.rust-lang.org/).

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
- **AI Tooling**
  - Automatically write commit messages based on your work in progress
  - Automatically create descriptive branch names
- **Commit Signing**
  - Easy commit signing with our generated SSH key

## Documentation

You can find our end user documentation at: https://docs.gitbutler.com

## Bugs and Feature Requests

If you have a bug or feature request, feel free to open an [issue](https://github.com/gitbutlerapp/gitbutler/issues/new),
or [join our Discord server](https://discord.gg/wDKZCPEjXC).

## Contributing

So you want to help out? Please check out the [CONTRIBUTING.md](CONTRIBUTING.md)
document.

If you want to skip right to getting the code to actually compile, take a look
at the [DEVELOPMENT.md](DEVELOPMENT.md) file.
