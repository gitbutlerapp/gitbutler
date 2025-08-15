<div align="center">
   <img align="center" width="128px" src="crates/gitbutler-tauri/icons/128x128@2x.png" />
	<h1 align="center"><b>GitButler</b></h1>
	<p align="center">
		Git branch management tool, built from the ground up for modern workflows
    <br />
    <a href="https://gitbutler.com"><strong>gitbutler.com »</strong></a>
    <br />
    <br />
    <b>Download for </b>
    macOS (<a href="https://app.gitbutler.com/downloads/release/darwin/aarch64/dmg">Apple Silicon</a> |
      <a href="https://app.gitbutler.com/downloads/release/darwin/x86_64/dmg">Intel</a>) ·
		Linux (<a href="https://app.gitbutler.com/downloads/release/linux/x86_64/gz">AppImage</a> |
       <a href="https://app.gitbutler.com/downloads/release/linux/x86_64/deb">deb</a>)
      ·
		Windows (<a href="https://app.gitbutler.com/downloads/release/windows/x86_64/msi">msi</a>)
    <br />
    <br />
    (Unstable Nightly releases can be found <a href="https://app.gitbutler.com/downloads">here</a>)
  </p>
</div>

<br/>

![gitbutler_client](https://github.com/user-attachments/assets/bf9bdb33-a979-47a0-b2b2-8fff5ea53afb)

[![CI][s0]][l0] [![BADGE][s6]][l6] [![TWEET][s1]][l1] [![DISCORD][s2]][l2] [![INSTA][s3]][l3] [![YOUTUBE][s5]][l5] [![DEEPWIKI][s7]][l7]

[s0]: https://github.com/gitbutlerapp/gitbutler/actions/workflows/push.yaml/badge.svg
[l0]: https://github.com/gitbutlerapp/gitbutler/actions/workflows/push.yaml
[s1]: https://img.shields.io/badge/Twitter-black?logo=x&logoColor=white
[l1]: https://twitter.com/intent/follow?screen_name=gitbutler
[s2]: https://img.shields.io/discord/1060193121130000425?label=Discord&color=5865F2
[l2]: https://discord.gg/MmFkmaJ42D
[s3]: https://img.shields.io/badge/Instagram-E4405F?logo=instagram&logoColor=white
[l3]: https://www.instagram.com/gitbutler/
[s5]: https://img.shields.io/youtube/channel/subscribers/UCEwkZIHGqsTGYvX8wgD0LoQ
[l5]: https://www.youtube.com/@gitbutlerapp
[s6]: https://img.shields.io/badge/GitButler-%23B9F4F2?logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMzkiIGhlaWdodD0iMjgiIHZpZXdCb3g9IjAgMCAzOSAyOCIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTI1LjIxNDUgMTIuMTk5N0wyLjg3MTA3IDEuMzg5MTJDMS41NDI5NSAwLjc0NjUzMiAwIDEuNzE0MDYgMCAzLjE4OTQ3VjI0LjgxMDVDMCAyNi4yODU5IDEuNTQyOTUgMjcuMjUzNSAyLjg3MTA3IDI2LjYxMDlMMjUuMjE0NSAxNS44MDAzQzI2LjcxOTcgMTUuMDcyMSAyNi43MTk3IDEyLjkyNzkgMjUuMjE0NSAxMi4xOTk3WiIgZmlsbD0iYmxhY2siLz4KPHBhdGggZD0iTTEzLjc4NTUgMTIuMTk5N0wzNi4xMjg5IDEuMzg5MTJDMzcuNDU3MSAwLjc0NjUzMiAzOSAxLjcxNDA2IDM5IDMuMTg5NDdWMjQuODEwNUMzOSAyNi4yODU5IDM3LjQ1NzEgMjcuMjUzNSAzNi4xMjg5IDI2LjYxMDlMMTMuNzg1NSAxNS44MDAzQzEyLjI4MDMgMTUuMDcyMSAxMi4yODAzIDEyLjkyNzkgMTMuNzg1NSAxMi4xOTk3WiIgZmlsbD0idXJsKCNwYWludDBfcmFkaWFsXzMxMF8xMjkpIi8%2BCjxkZWZzPgo8cmFkaWFsR3JhZGllbnQgaWQ9InBhaW50MF9yYWRpYWxfMzEwXzEyOSIgY3g9IjAiIGN5PSIwIiByPSIxIiBncmFkaWVudFVuaXRzPSJ1c2VyU3BhY2VPblVzZSIgZ3JhZGllbnRUcmFuc2Zvcm09InRyYW5zbGF0ZSgxNi41NzAxIDE0KSBzY2FsZSgxOS44NjQxIDE5LjgzODMpIj4KPHN0b3Agb2Zmc2V0PSIwLjMwMTA1NiIgc3RvcC1vcGFjaXR5PSIwIi8%2BCjxzdG9wIG9mZnNldD0iMSIvPgo8L3JhZGlhbEdyYWRpZW50Pgo8L2RlZnM%2BCjwvc3ZnPgo%3D
[l6]: https://gitbutler.com/
[s7]: https://deepwiki.com/badge.svg
[l7]: https://deepwiki.com/gitbutlerapp/gitbutler

![Alt](https://repobeats.axiom.co/api/embed/fb23382bcf57c609832661874d3019a43555d6ae.svg 'Repobeats analytics for GitButler')

GitButler is a git client that lets you work on multiple branches at the same time.
It allows you to quickly organize file changes into separate branches while still having them applied to your working directory.
You can then push branches individually to your remote, or directly create pull requests.

In a nutshell, it's a more flexible version of `git add -p` and `git rebase -i`, allowing you to efficiently multitask across branches.

## How Does It Work?

GitButler keeps track of uncommitted changes in a layer on top of Git. Changes to files or parts of files can be grouped into what we call virtual branches. Whenever you are happy with the contents of a virtual branch, you can push it to a remote. GitButler makes sure that the state of other virtual branches is kept separate.

## How Do GB's Virtual Branches Differ From Git Branches?

The branches that we know and love in Git are separate universes, and switching between them is a full context switch. GitButler allows you to work with multiple branches in parallel in the same working directory. This effectively means having the content of multiple branches available at the same time.

GitButler is aware of changes before they are committed. This allows it to keep a record of which virtual branch each individual diff belongs to. Effectively, this means that you can separate out individual branches with their content at any time to push them to a remote or to unapply them from your working directory.

And finally, while in Git it is preferable that you create your desired branch ahead of time, using GitButler you can move changes between virtual branches at any point during development.

## Why GitButler?

We love Git. Our own [@schacon](https://github.com/schacon) (Scott Chacon), co-founder of GitHub and author of the [Pro Git](https://git-scm.com/book/en/v2) book, helped grow GitHub from 4 founders to 450 employees over 8 years before its $7.5 billion acquisition by Microsoft. At the same time, Git's user interface hasn't been fundamentally changed for 15 years. While it was written for Linux kernel devs sending patches to each other over mailing lists, most developers today have different workflows and needs.

Instead of trying to fit the semantics of the Git CLI into a graphical interface, we are starting with the developer workflow and mapping it back to Git.

## Tech

GitButler is a [Tauri](https://tauri.app/)-based application. Its UI is written in [Svelte](https://svelte.dev/) using [TypeScript](https://www.typescriptlang.org) and its backend is written in [Rust](https://www.rust-lang.org/).

## Main Features

- **Virtual Branches**
  - Organize work on multiple branches simultaneously, rather than constantly switching branches
  - Automatically create new branches when needed
- **Easy Commit Management**
  - Undo, Amend and Squash commits by dragging and dropping
- **Undo Timeline**
  - Logs all operations and changes and allows you to easily undo or revert any operation
- **GitHub Integration**
  - Authenticate to GitHub to open Pull Requests, list branches and statuses and more
- **Easy SSH Key Management**
  - GitButler can generate an SSH key to upload to GitHub automatically
- **AI Tooling**
  - Automatically write commit messages based on your work in progress
  - Automatically create descriptive branch names
- **Commit Signing**
  - Easy commit signing with GPG or SSH

## Example Uses

### Fixing a Bug While Working on a Feature

> Say that while developing a feature, you encounter a bug that you wish to fix. It's often desirable that you ship the fix as a separate contribution (Pull request).

Using Git you can stash your changes and switch to another branch, where you can commit, and push your fix.

_With GitButler_ you simply assign your fix to a separate virtual branch, which you can individually push (or directly create a PR). An additional benefit is that you can retain the fix in your working directory while waiting for CI and/or code review.

### Trying Someone Else's Branch Together With My Work in Progress

> Say you want to test a branch from someone else for the purpose of code review.

Using Git trying out someone else's branch is a full context switch away from your own work.
_With GitButler_ you can apply and unapply (add / remove) any remote branch directly into your working directory.

## Documentation

You can find our end user documentation at: https://docs.gitbutler.com

## Bugs and Feature Requests

If you have a bug or feature request, feel free to open an [issue](https://github.com/gitbutlerapp/gitbutler/issues/new),
or [join our Discord server](https://discord.gg/MmFkmaJ42D).

## AI Commit Message Generation

Commit message generation is an opt-in feature. You can enable it while adding your repository for the first time or later in the project settings.

Currently, GitButler uses OpenAI's API for diff summarization, which means that if enabled, code diffs would be sent to OpenAI's servers.

Our goal is to make this feature more modular such that in the future you can modify the prompt as well as plug a different LLM endpoints (including local ones).

## Contributing

So you want to help out? Please check out the [CONTRIBUTING.md](CONTRIBUTING.md)
document.

If you want to skip right to getting the code to actually compile, take a look
at the [DEVELOPMENT.md](DEVELOPMENT.md) file.

Want to show your support? Add a GitButler badge to your project's README:

```md
[![GitButler](https://img.shields.io/badge/GitButler-%23B9F4F2?logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMzkiIGhlaWdodD0iMjgiIHZpZXdCb3g9IjAgMCAzOSAyOCIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTI1LjIxNDUgMTIuMTk5N0wyLjg3MTA3IDEuMzg5MTJDMS41NDI5NSAwLjc0NjUzMiAwIDEuNzE0MDYgMCAzLjE4OTQ3VjI0LjgxMDVDMCAyNi4yODU5IDEuNTQyOTUgMjcuMjUzNSAyLjg3MTA3IDI2LjYxMDlMMjUuMjE0NSAxNS44MDAzQzI2LjcxOTcgMTUuMDcyMSAyNi43MTk3IDEyLjkyNzkgMjUuMjE0NSAxMi4xOTk3WiIgZmlsbD0iYmxhY2siLz4KPHBhdGggZD0iTTEzLjc4NTUgMTIuMTk5N0wzNi4xMjg5IDEuMzg5MTJDMzcuNDU3MSAwLjc0NjUzMiAzOSAxLjcxNDA2IDM5IDMuMTg5NDdWMjQuODEwNUMzOSAyNi4yODU5IDM3LjQ1NzEgMjcuMjUzNSAzNi4xMjg5IDI2LjYxMDlMMTMuNzg1NSAxNS44MDAzQzEyLjI4MDMgMTUuMDcyMSAxMi4yODAzIDEyLjkyNzkgMTMuNzg1NSAxMi4xOTk3WiIgZmlsbD0idXJsKCNwYWludDBfcmFkaWFsXzMxMF8xMjkpIi8%2BCjxkZWZzPgo8cmFkaWFsR3JhZGllbnQgaWQ9InBhaW50MF9yYWRpYWxfMzEwXzEyOSIgY3g9IjAiIGN5PSIwIiByPSIxIiBncmFkaWVudFVuaXRzPSJ1c2VyU3BhY2VPblVzZSIgZ3JhZGllbnRUcmFuc2Zvcm09InRyYW5zbGF0ZSgxNi41NzAxIDE0KSBzY2FsZSgxOS44NjQxIDE5LjgzODMpIj4KPHN0b3Agb2Zmc2V0PSIwLjMwMTA1NiIgc3RvcC1vcGFjaXR5PSIwIi8%2BCjxzdG9wIG9mZnNldD0iMSIvPgo8L3JhZGlhbEdyYWRpZW50Pgo8L2RlZnM%2BCjwvc3ZnPgo%3D)](https://gitbutler.com/)
```

[![BADGE][s6]][l6]
