<div align="center">
  <img align="center" width="825px" alt="UnityButler" src="https://github.com/user-attachments/assets/7f21600a-aade-427e-9f91-7ebdac9cdad1" />

  <br />
  <br />

  <h1>UnityButler</h1>
  <p>
    <b>Powered by GitButler for Unity projects.</b>
    <br />
    A Unity-focused presentation of the GitButler desktop app, web surface, and
    CLI for Unity-heavy teams, technical artists, and AI-assisted workflows.
  </p>

  <p>
    <a href="./DEVELOPMENT.md">Development</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="./CONTRIBUTING.md">Contributing</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="./apps/desktop">Desktop App</a>
    <span>&nbsp;&nbsp;•&nbsp;&nbsp;</span>
    <a href="./crates/but">CLI</a>
  </p>
</div>

---

## What is UnityButler?

UnityButler is a Unity-focused presentation of the GitButler codebase: the same
Git-powered branching engine and agent-friendly tooling, but framed around the
workflows that show up constantly in Unity development.

It is explicitly **powered by GitButler**, not an unrelated replacement for it.
The branding here is meant to describe a Unity-oriented packaging and
documentation layer on top of the existing GitButler project and license.

That means:

- managing feature work, scene work, prefab changes, tooling experiments, and
  content updates without living in raw Git commands
- giving designers, programmers, technical artists, and automation agents a
  friendlier interface for the same repository
- keeping a powerful desktop UI and a scriptable CLI backed by the same Rust
  core

UnityButler still works on normal Git repositories. The difference is in how
the project is described, organized, and released for teams building with
Unity, while remaining rooted in GitButler itself.

## Why Unity teams would use it

- **Stacked branches** for layered gameplay, content, and tooling work
- **Parallel branches** so experimental scene or prefab changes do not block
  each other
- **Commit surgery** for splitting, moving, rewording, and squashing work
  without a painful interactive rebase flow
- **Timeline and undo** so destructive Git mistakes are easier to recover from
- **Forge integration** for GitHub and GitLab-driven collaboration
- **AI-friendly workflows** through the desktop app, the `but` CLI, and repo
  automation

## Repository layout

This repository is a Rust/Svelte/React/TypeScript monorepo.

- `apps/desktop` - Tauri desktop frontend
- `apps/web` - web frontend
- `apps/lite` - Electron/React desktop app
- `crates` - Rust workspace, including the shared backend APIs and the `but` CLI
- `packages` - shared TypeScript packages and UI libraries
- `e2e` - Playwright, WebdriverIO, and blackbox end-to-end tests

## Tech stack

- **Desktop app:** Tauri + Svelte + TypeScript
- **Backend/core:** Rust
- **CLI:** Rust (`but`)
- **Workspace tooling:** Turborepo + pnpm
- **Release workflow:** GitHub Actions, with Bun used for the Windows push
  release pipeline

## Getting started

### Prerequisites

1. Rust from `rust-toolchain.toml`
2. Node.js from `.nvmrc` / `package.json`
3. pnpm via Corepack for local development
4. Platform dependencies required by Tauri

### Local setup

```bash
corepack enable
pnpm install
cargo build
```

### Common development commands

```bash
pnpm dev:desktop
pnpm dev:web
pnpm build
pnpm test
pnpm lint
```

For the full developer environment and platform-specific setup, see
[DEVELOPMENT.md](./DEVELOPMENT.md).

## Release automation

The repository now includes a push-triggered Windows release workflow:

- it runs on pushes to `master`
- installs JavaScript dependencies with **Bun**
- builds a Windows **NSIS `.exe` installer**
- uploads the installer as a workflow artifact
- publishes a GitHub prerelease and lets GitHub generate release notes from the
  changes included in that release

That keeps the release changelog tied to the actual commits instead of requiring
manual notes for every push build.

## Contributing

If you want to contribute, start here:

- [CONTRIBUTING.md](./CONTRIBUTING.md)
- [DEVELOPMENT.md](./DEVELOPMENT.md)

## License

UnityButler is powered by the existing GitButler codebase and remains subject
to the repository's current Fair Source licensing model. This README does not
relicense or reframe the project as a separate competing product; it is a
Unity-oriented presentation of the same licensed repository. See the license
files in this repository for the exact terms.
