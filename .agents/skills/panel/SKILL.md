---
name: panel
description: Use when asked to open, show, or try the `but panel` workspace view, or when the user types /panel. Starts the experimental read-only panel for this repository and opens it in the Claude Code browser pane beside the chat.
---

`but panel` serves a narrow, read-only view of a GitButler workspace on
`http://localhost:7789`: branches with their pull requests and CI, commits, their
files and diffs, the uncommitted changes, and linked worktrees, refreshing
every few seconds. It is hidden and experimental, so run it from source.

## 1. Find the project

Show the workspace of the repository this session works in:

```console
$ git rev-parse --show-toplevel
```

A linked worktree reports its own path, but reads the same GitButler
workspace as the main checkout, so either path works.

## 2. Start or reuse the server

Check what is already on the port. The response names the repository the
panel serves:

```console
$ curl -sf --max-time 2 http://localhost:7789/api/workspace
```

- **It answers `"ok":true` with this repository's name:** reuse it, skip ahead.
- **It answers with another repository, or something else holds the port:**
  stop it with `lsof -ti tcp:7789 | xargs kill`, then start a new one.
- **Nothing answers:** start one.

Start it with `run_in_background: true`. `--no-open` matters: without it the
command opens a separate system browser window instead of the pane.

```console
$ cargo run -p but -- -C "<project root>" panel --no-open
```

The first build takes a few minutes. Wait for the port instead of sleeping:

```console
$ until curl -sf -o /dev/null http://localhost:7789/; do sleep 2; done
```

If the command reports the directory is not a GitButler project, say so
plainly rather than setting one up.

## 3. Open it in the browser pane

Reuse a tab rather than stacking new ones:

- Call `tabs_context`. If a tab is already on `http://localhost:7789`, close any
  other tabs on that origin and `navigate` the remaining one to the URL.
- Otherwise call `preview_start` with `url: "http://localhost:7789"`.

## 4. Tell the user

Call `tabs_context` again and read its last line.

- **Pane displayed:** say it is open, in one short line.
- **Pane hidden:** no tool can reveal a hidden Browser pane; `preview_start`,
  `tabs_select`, and closing and reopening all leave it hidden. Do not retry.
  Say in one line that the panel is loaded and the globe (Browser) toggle in
  the session header shows it.

Do not narrate the intermediate steps.

## Notes

- The page polls on its own; the server never needs restarting when the
  workspace changes.
- Running a development build of `but` rewrites the user's installed GitButler
  agent skill to `dev`. Mention once that running their release `but` (for
  example `but status`) restores it.
- Stop the server with `lsof -ti tcp:7789 | xargs kill` when asked.
