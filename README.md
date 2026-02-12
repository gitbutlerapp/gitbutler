<div align="center">
  
  <img align="center" width="100px" src="https://gitbutler-docs-images-public.s3.us-east-1.amazonaws.com/md-logo.png" alt="GitButler logo" />
  <br />

  <h1 align="center">GitButler</h1>
  
  <p align="center">
   <b>Git, <i>but</i> better</b>.
   <br/>
   GitButler is a modern Git-based version control interface with both a GUI and CLI built from the ground up for AI-powered workflows.
    <br />
    <br />
    <a href="https://gitbutler.com">Website</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="https://blog.gitbutler.com/">Blog</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="https://docs.gitbutler.com/">Docs</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="https://gitbutler.com/downloads">Downloads</a>
  </p>

  <br/>

  <img width="100%" src="https://gitbutler-docs-images-public.s3.us-east-1.amazonaws.com/app-preview-light.png" alt="GitButler desktop app preview" />
  <p align="center"><i>Our beautiful GUI</i></p>

  <img width="100%" src="https://gitbutler-docs-images-public.s3.us-east-1.amazonaws.com/cli-preview.png" alt="GitButler CLI preview" />
  <p align="center"><i>Our amazing <code>but</code> CLI</i></p>

  <br/>

[![TWEET][s1]][l1] [
![BLUESKY][s8]][l8] [![DISCORD][s2]][l2]

[![CI][s0]][l0] [![INSTA][s3]][l3] [![YOUTUBE][s5]][l5] [![DEEPWIKI][s7]][l7]

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
[s7]: https://deepwiki.com/badge.svg
[l7]: https://deepwiki.com/gitbutlerapp/gitbutler
[s8]: https://img.shields.io/badge/Bluesky-0285FF?logo=bluesky&logoColor=fff
[l8]: https://bsky.app/profile/gitbutler.com

</div>

<br/>

GitButler is a powerful new Git-based version control system, designed from scratch to be simple, powerful and flexible. It is designed for ease of use and modern agentic workflows.

It features stacked branches, parallel branches, unlimited undo, easy commit mutations, forge integrations and more.

Works instantly in any existing Git repo as a friendlier and more powerful drop-in Git user interface replacement - for you and your agents.

## Main Features

Why use GitButler instead of vanilla Git? What a great question.

- **Stacked Branches** ([gui](https://docs.gitbutler.com/features/branch-management/stacked-branches), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/branching-and-commiting#stacked-branches))
  - Effortlessly create branches stacked on other branches. Amend or edit any commit easily with automatic restacking.
- **Parallel Branches** ([gui](https://docs.gitbutler.com/features/branch-management/virtual-branches), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/branching-and-commiting#parallel-branches))
  - Organize work on multiple branches simultaneously, rather than constantly switching branches.
- **Easy Commit Management** ([gui](https://docs.gitbutler.com/features/branch-management/commits), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/rubbing))
  - Uncommit, reword, amend, move, split and squash commits by dragging and dropping or simple CLI commands. Forget about `rebase -i`, you don't need it anymore.
- **Undo Timeline** ([gui](https://docs.gitbutler.com/features/timeline), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/operations-log))
  - Logs all operations and changes and allows you to easily undo or revert any operation.
- **First Class Conflicts** ([gui](https://docs.gitbutler.com/overview#conflicting-branches), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/conflict-resolution))
  - Rebases always succeed. Commits can be marked as conflicted and resolved at any time, in any order.
- **Forge Integration** ([gui](https://docs.gitbutler.com/features/forge-integration/github-integration), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/forges))
  - Authenticate to GitHub or GitLab to easily open and update Pull Requests, list branches, get CI statuses and more. No other tools required.
- **AI Tooling** ([gui](https://docs.gitbutler.com/features/ai-integration/ai-overview), [cli](https://docs.gitbutler.com/cli-guides/cli-tutorial/ai-stuff))
  - Use built-in AI handlers to help create commit messages, branch names, PR descriptions and more.
  - Easily install hooks or skills for all modern agent systems to level up their Git management.

## Tech

The GitButler desktop app is a [Tauri](https://tauri.app/)-based application. Its UI is written in [Svelte](https://svelte.dev/) using [TypeScript](https://www.typescriptlang.org) and its backend is written in [Rust](https://www.rust-lang.org/).

The `but` CLI is the same Rust backend engine with a Rust command line UI.

## Documentation

You can find our end user documentation at: <https://docs.gitbutler.com>

## Bugs and Feature Requests

If you have a bug or feature request, feel free to open an [issue](https://github.com/gitbutlerapp/gitbutler/issues/new),
or [join our Discord server](https://discord.gg/MmFkmaJ42D).

## License

The TLDR is that GitButler is under a [Fair Source](https://fair.io/) software license, meaning that you can use it, view the source, contribute, etc. You just can't build a competitor with it. It also becomes MIT after 2 years. So, MIT with an expiring non-compete clause.

## Contributing

So you want to help out? Please check out the [CONTRIBUTING.md](CONTRIBUTING.md)
document.

If you want to skip right to getting the code to actually compile, take a look
at the [DEVELOPMENT.md](DEVELOPMENT.md) file.

### Contributors

<a href="https://github.com/gitbutlerapp/gitbutler/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=gitbutlerapp/gitbutler" />
</a>
