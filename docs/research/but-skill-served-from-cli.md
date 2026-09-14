# Serving the `but` skill from the CLI: inventory and proposed structure

Design note, 2026-09-08. Scope is structure only: which commands exist, which to add, and how to test a stub-based install against the current full install before committing to it. Skill wording is out of scope until the structure is proven. Background in [agent-browser-case-study-2026-09.md](agent-browser-case-study-2026-09.md) section 1.

---

## 1. Inventory: what exists today

### 1.1 Skill content

```
crates/but/skill/
  SKILL.md                    220 lines, frontmatter description is the trigger text, version: 0.0.0 placeholder
  references/reference.md     663 lines, command syntax and flags
  references/concepts.md      330 lines, workspace model
  references/examples.md      485 lines, workflows
  AGENTS.md / README.md       editing rules and dev docs, not installed
```

All four installed files are embedded with `include_bytes!` into a `SKILL_FILES` const in `crates/but/src/command/skill/mod.rs`. `inject_version` rewrites `version: 0.0.0` in the SKILL.md frontmatter at install time; that version is what `skill check` compares against the binary. The content is already inside the binary. Nothing new has to be embedded to serve it.

`skill/AGENTS.md` says "The same facts are deliberately repeated across the four installed files." That is a content decision to revisit in the wording pass, not now, but it matters here because a served skill makes "one place per fact" cheap to enforce.

### 1.2 Commands

| Command | What it does | Notes |
|---|---|---|
| `but skill install [-g] [-p path] [-d]` | Copies the four files to a chosen format directory (Agent Skills, Claude, OpenCode, Codex, Copilot, Cursor, Windsurf, Poolside, Kiro, Junie), local or global; wizard when interactive; `--detect` refreshes existing installs | The single-purpose primitive |
| `but skill check [-u] [-g|-l]` | Scans installs, compares embedded version to the binary, optionally rewrites | Version-based freshness |
| `but agent [setup] [--print]` | Wizard: picks agents and scope, installs skills through `skill::write_skill_files`, writes a managed `gitbutler-agent-setup` block into AGENTS.md / CLAUDE.md with chosen workflow policies | Layered on top of `skill install`, not a duplicate: it calls the same writer and adds steering |
| `but help <topic>` (hidden) | Prints long-form topics such as `cli-ids` from clap metadata | Already a served-reference surface for concepts, just not connected to the skill |
| `but mcp` | Tools over stdio: workspace, commit details, branch details, and more | No skill exposure |
| Freshness notice | On agent-detected human-text output of most commands: "AGENT ACTION REQUIRED: The GitButler skill is not installed ... run: `but skill install`" or "out of date ... `but skill check --update`"; stale global copies are auto-rewritten | Suppressed for `skill`, `agent`, `mcp`, `help`, `completions`, `metrics`, external |

The `agent` versus `skill` overlap is real but shallow. `agent setup` owns the wizard and the steering block; `skill install` owns file placement. Keep that split. What is missing is a third responsibility neither has: printing the content.

### 1.3 How the skill enters the benchmark

version-control-bench installs the skill into `.codex/skills/gitbutler` and `.claude/skills/gitbutler` before timing, writes local AGENTS.md / CLAUDE.md, blocks raw git writes, and accepts `--but-bin` to point at a local build. Arms are `but+skill`, `jj+skill`, `git`. A stub arm is a new arm with a different install step, nothing else changes.

---

## 2. Proposed structure

### 2.1 New: bare `but skill` prints the guide

```
but skill                         print core (bare group runs the default, as `git stash` does)
but skill reference               explicit subcommands, one per doc: reference | concepts | examples
but skill --full                  core plus every reference, each introduced by a `--- <path> ---` line
but skill --json                  {name, content, files: [{path, content}]} (files empty unless --full)
but skill install | check         unchanged
```

Bare `but skill` follows conventions the CLI already has: bare `but` is status, bare `but agent` is setup, and `git stash` is the external precedent. Two words is the shortest pointer an error message or notice can carry. No `show` or `list` subcommand: core already ends with a pointer list to the other docs.

The docs are explicit subcommands, not a free positional. A positional sharing a group with `install` and `check` is the one shape clig.dev and cargo both avoid: shared namespace, no completion of the value set, future collisions. Subcommands are a closed set that tab-completes and appears in `--help`. Verbs and nouns side by side in one listing is the residual smell; `gh` and `git` both live with it. Printing long-form reference from the binary is itself the orthodox pattern (`rustc --explain`, `cargo help`, `git help <concept>`, `kubectl explain`), and the rustc pairing is the model for errors: short text plus a `help:` line naming `but skill reference`.

Names map to the existing files, which gives the hierarchy for free:

| Name | Source | Role |
|---|---|---|
| `core` (default) | `SKILL.md` body | The loop, IDs, rules, recipes; points at the others |
| `reference` | `references/reference.md` | Syntax and flags |
| `concepts` | `references/concepts.md` | Workspace model |
| `examples` | `references/examples.md` | Workflows |

Implementation: `cmd: Option<Subcommands>`, `None` prints core. Reads `SKILL_FILES`; no new embedding, no daemon, no repo required, runs before repo discovery so it works from a fresh agent that has not run `but setup`. Content goes straight to stdout with no pager (git and cargo page only on a tty; this command never should), and `NO_COLOR` applies as elsewhere. Today bare `but skill` errors with a missing-subcommand message, so this changes behaviour only for a call that never succeeded.

`but help <topic>` stays as is. Long term the concept topics (`cli-ids`) and `concepts.md` overlap; that is a wording-pass merge, not a structure change.

### 2.2 New: a stub install mode (team-internal for now)

`but skill install --stub` is a hidden flag (`hide = true`, absent from `--help`, docs, and skill content) intended only for the team while the approach is evaluated. It writes one file instead of four:

```markdown
---
name: but
version: <injected>
description: <unchanged trigger text>
---

# GitButler CLI

This file is a discovery stub, not the usage guide. Before your first `but` command, load the guide from the CLI so it always matches the installed version:

    but skill                       # the loop, IDs, rules, task recipes
    but skill --full                # plus command reference, concepts, examples

Load one reference directly when you know what you need:

    but skill reference             # command syntax and flags
    but skill concepts              # workspace model
    but skill examples              # workflows
```

Frontmatter stays identical so triggering is unchanged and `skill check` keeps working. The body has no facts about `but` that can go stale. Default install behaviour does not change until the stub is proven.

`but agent setup` gets the same hidden switch so the wizard can produce a stub install; the steering block's "Use the installed GitButler skill for command recipes and syntax" line becomes "Run `but skill` for command recipes and syntax" in stub mode.

### 2.3 Notices and errors point at `but skill`, not `install` (after the evaluation)

Bare `but skill` and the doc subcommands are public reads from day one but are not advertised outside the skill content until the stub is proven. Once it is, the freshness notices can stop asking the agent to install anything:

| Today | Proposed |
|---|---|
| "The GitButler skill is not installed ... run: `but skill install`, then read the installed SKILL.md path" | "Read the GitButler guide before continuing: `but skill`" |
| "The GitButler skill is out of date ... run: `but skill check --update`" | Unchanged for full installs; not applicable to stub installs |

That is a cheaper ask (one read versus a filesystem write plus a re-read), works for agents that cannot load skill files at all, and retired-syntax hints and `--help` footers can use the same pointer: "syntax: `but skill reference`".

### 2.4 Later, once proven

- Flip the default: `skill install` and `agent setup` write the stub; `--full` restores the four files for agents that need offline content.
- `but mcp` exposes a `gitbutler_skill` tool returning the same content, mirroring agent-browser's CLI/MCP parity rule.
- Retire the content half of the freshness auto-update; only the stub format needs a version.
- AGENTS.md rule for the repo: any user-facing `but` change updates `--help`, the served skill, and docs in the same change.

---

## 3. Test plan before committing

Two layers, cheap first.

### 3.1 Skill-loading eval (single turn, minutes)

Borrowed from agent-browser's `evals/`: inject the stub as the installed skill, send a task prompt through `claude -p` and `codex exec --json`, regex the single response, optional 1 to 5 judge.

| Category | Pass condition |
|---|---|
| loading | `but skill` appears before any mutating `but` command |
| selection | A syntax question reaches `but skill reference`; a "why did that happen" question reaches `but skill concepts` |
| usage | The emitted `but` sequence matches the full-skill baseline for the same prompt |
| footprint | Bytes and approximate tokens of stub, `but skill`, `but skill --full`, versus the four-file install |

This tests whether the redirect fires at all, which is the failure mode that would sink the approach silently.

### 3.2 vcbench arm (hours)

Add arm `but+stub`: identical to `but+skill` except the pre-run install uses `--stub`. Run the thirteen tasks on both agents against the same `--but-bin`. Compare task VC score, duration, command count, tokens, and count of `but skill` reads per run. Acceptance: no task loses its floor, duration and tokens within noise. Expected cost: one extra turn per session for the `but skill` read, offset by never loading references the task does not need.

Watch for: agents that read `but skill` on every turn instead of once (steering fix in the stub), and agents that skip the read entirely and guess syntax (the loading eval catches this before vcbench does).

### 3.3 Order of work

1. Bare `but skill` and `but skill <doc>` with `--full` and `--json`. Pure read; the only behaviour change is that a call which used to error now prints the guide.
2. `but skill install --stub` and the matching `agent setup` switch, off by default.
3. Loading eval, then the vcbench arm.
4. Decide. Only then touch notice wording, defaults, and skill content.

---

## 4. Follow-ups from the adoption research

From [skill-stub-adoption-2026-09.md](skill-stub-adoption-2026-09.md). Independent of the stub decision unless marked.

1. **Served-content command guard.** A test that extracts every fenced `but …` line from `crates/but/skill/**/*.md` and parses it with clap's `try_get_matches_from`, failing on any invocation the current binary rejects. WrenAI runs the equivalent over ~440 invocations in CI. This is the mechanical fix for the retired-syntax drift that motivated this work.
2. **Frontmatter key check.** Before `--stub` goes public, confirm Claude Code, Codex, and OpenCode load a skill whose frontmatter carries an unknown `stub: true` key. agent-browser's `hidden: true` is refused by OpenCode (issue #1337, open since 2026-05).
3. **"Start here (for AI agents)" block in `but --help`** naming `but skill`, per agent-browser PR #1251. Gated on the evaluation.
4. **Never distribute the stub ahead of the binary.** If the stub is ever published to a skills registry, pin it to a minimum `but` version or keep `but skill install --stub` as the only path. WrenAI issue #2341 is the failure mode.
5. **Migration shape when the default flips.** One-time `check --update` rewrites full installs to stubs; a test guards that the four-file layout is not regenerated (WrenAI's `test_deprecated_dirs_removed`).
6. **Binary resolution in the stub.** Consider a `BUT_BIN` style override line so dev builds and vcbench's `--but-bin` drive the same stub (Orca's stub does this).
7. **Measure, since nobody has.** No adopter has published stub-versus-full outcomes. The `but+stub` vcbench arm plus the single-turn loading eval would be the first. Note that Microsoft's playwright-cli deliberately chose reinstall-and-nudge over a stub (issues #415, #421), which is `but`'s current model, so the bench is choosing between two live positions.
8. **Frontmatter validated (done 2026-09-08).** Codex's runtime parser (`codex-rs/skills/src/parser.rs`, `SkillFrontmatter` without `deny_unknown_fields`) ignores `stub` and `allowed-tools`; OpenCode's loader decodes only `name`, `description`, `slash`; Claude Code documents `allowed-tools`. The strict allow-list (`name, description, license, allowed-tools, metadata`) is only in Codex's `skill-creator` `quick_validate.py`, which already rejects the full skill's `version` and `author` keys, so the stub changes nothing there. If that validator ever matters, move `version`, `author`, and `stub` under `metadata:`. Supersedes item 2.
9. **Description recall eval** using playwright-cli #410's method: fictional skill name, 40 should-trigger and 10 should-not prompts, Codex and Claude as separate judges, report both. Quoted "Use when the user says" phrases lifted Codex recall from 0.80 to 0.975 and left Opus unchanged.
10. **Decide auto-update behaviour for user-edited skill files.** The freshness update overwrites in place today. Sentry's auto-install drew "clobbers my customized skills" (getsentry/cli #1403) and had to add an opt-out and persist it. A stub has nothing to customize, which is a point in its favour.
11. **Consider generating `reference.md` from clap** the way Sentry generates its skill from the command tree, as the stronger alternative to the parse guard in item 1. It removes the drift class rather than detecting it.
12. **Generate the reference from clap instead of maintaining it.** Others split the same way: `gh` and `jj` ship no skill and run on `--help`; playwright-cli and agent-browser mirror help inside the skill and drift; Sentry generates command references from its route tree and hand-writes only judgment (principles, context tips, safety, exit codes), and still got "25KB that mirrors `sentry --help`" as a complaint. For `but`: `but skill reference` renders every command's long help from the live clap tree in one call (batching keeps the turn cost that vcbench's help-probe rule guards against), `concepts` stays hand-written, `examples` folds into clap `## Examples` sections except multi-command recipes. Retires 663 hand-maintained lines and the parse guard for them. Add a `but+help` vcbench arm with no skill at all to measure what the hand-written remainder is worth. Supersedes item 11.

    **Sizing (2026-09-09, but 0.5.2191, `target/debug/but`):** the whole tree is 92 commands; `--help` for all of them is 102 KB / 3575 lines, `-h` is 53 KB, the 19 commands the skill teaches are 33 KB, the same as today's hand-written `reference.md` (33 KB / 668 lines). ~29 KB of the full dump is boilerplate repeated per command (`Options:`, `--json`, `--status-after`, `-h`). Roughly half the tree is agent-irrelevant (`alias`, `completions`, `gui`, `tui`, `update`, `worktree`, `config`, `skill install` alone is 1.9 KB). 35 of 92 commands carry an examples section.

    **Shape decided:** `but skill reference` = generated overview (command groups, one-line `about` + usage line per command, ~5–8 KB) that advertises `but <cmd> --help` for depth; per-command long help stays the unit an agent pays for on demand (1.5–4.7 KB each); hand-written text keeps only what help cannot say (ID model, chaining, merged-upstream rule, git-to-but map, multi-command recipes). Precedent in-tree: `but help cli-ids` is a served topic rendered by `command/help.rs` and already pointed at from `commit`/`diff` help; implement the overview once and expose it from both. Follow-on clap work the generator makes visible: add examples to the 57 commands without them; trim bloated long help (`move` 4.7 KB, `squash` 4 KB) by moving skill-shaped prose out. Risk: the `about` lines become load-bearing (`pull`'s "Updates all applied branches to be up to date with the target branch" is the sentence behind the September sync detour), so they join the wording audit.

    **Revised shape (2026-09-09, after Kiril's point that sub-subcommand capabilities are buried):** the reference is the flattened, boilerplate-stripped `-h` tree of the 44 agent-relevant leaves (21 KB), not an about+usage list (4.5 KB). The flat list surfaces the 18 second-level commands agents never open a parent help to find (`branch update`, the five `resolve` verbs, `oplog restore`, `pr` state changes) but still hides flag-level capabilities: `pick`'s usage is `<SOURCES>...` so `--below`/`--branch` are invisible, `branch update`'s four strategies only appear in options. Generator: walk the clap `Command` in-process (shared with the parse guard), skip the non-agent subtree and hidden commands, per leaf emit about + usage + arguments + options one-liners, state `-C`/`--json`/`--status-after` once at the top, group by the headings `but --help` uses, trailer pointing at `but <cmd> --help` for long prose and examples. The skip list and `about` lines are part of the skill contract.

    **Format decided (2026-09-09):** markdown; group headings as in `but --help`; each command is `### but <cmd> <args>` (the invocation, so a grep by name or by intent lands on it), one intent sentence (the clap `about`), then one bullet per flag with its one-liner, default, and possible values. Global flags stated once at the top; one pointer to `but <cmd> --help` for examples and long prose. No clap terminal layout (the `Usage:`/`Options:` scaffolding and column wrapping are the "mirrors `--help`" smell), no tables (flag-heavy commands make unreadable cells), no per-command examples, no guidance prose, no emphasis. Reasoning: leading words carry the scan; keep parameter contracts and drop layout; the reference is a disclosed branch so it is flat and exhaustive while the core skill routes and `--help` explains.

    **Measured (2026-09-09, mock generated from but 0.5.2191):** 44 commands, 119 args+flags. One-liner (`-h`) variant 12.9 KB / 254 lines / ~3.3k tokens; long-description (`--help`) variant 22.0 KB / ~5.6k tokens; today's `reference.md` 33 KB / 668 lines / ~8.5k tokens. Decision (revised the same day): ship the long-description variant (clap `long_help`, 22 KB). Tokens are cheap and turns are not (a `--help` detour caused by a thin one-liner costs a whole model turn; July lesson: wall time moves only when a turn disappears); both variants are 254 lines so scan cost is equal; the long text carries the contracts and failure modes (`-b` creates unstacked / errors on unapplied, `--below` treats branches as buckets) that the research says are under-supplied. Bloated `long_help` (`commit` repeats the worktree sentence across three flags, `move` is 4.7 KB) is trimmed in the wording pass, which improves human `--help` too. `but <cmd> --help` then adds only examples over the reference.

    **Pre-build decisions (2026-09-09, Kiril):**
    1. Skip only human/meta surfaces (`gui`, `tui`, `alias`, `completions`, `update`, `skill`, `agent`). Keep `setup` (cold adoption, pilot-8), `config` (`forge auth`), `teardown`, `worktree` (`worktree:@` addressing).
    2. Nothing in `references/reference.md` is worth migrating; the generated reference replaces it without a content audit.
    3. The served `but skill reference` and the installed `references/reference.md` (kept as-is for full installs) may diverge during the transition.
    4. Guard kept light: one snapshot test of the generated reference, so clap wording changes are reviewable as reference diffs (an empty `about` shows up there too; no separate check).
    5. Pointer wording in the core skill and stub is left for the wording overhaul.
    Build notes: render from the clap `Command` in-process, sharing the walk with the parse guard; intent line from `about`, never `long_about`; render `{update|-u}` aliases as the plain name; no colour or pager when piped; never inline the reference into the SKILL.md body.

    **Merged as PR #15853 (2026-09-09). Originally:** `crates/but/src/command/skill/reference.rs` (~110 lines) renders from `Args::command()`; `SkillFile::served_text` swaps it in for `reference`; `--full` separators are now `--- but skill <name> ---` and `--json` lists `references: [{name, content}]`. Snapshot at `crates/but/tests/but/command/skill/snapshots/reference.md`. Output is 23.1 KB / 316 lines after the same-day evaluation pass: positionals listed first; `--allow-merged` (was repeated 11 times) stated once in the preamble; `--interactive` flags dropped (TTY rule); `open`, `config ai` (secrets), `config metrics` skipped; preamble points at `but help cli-ids` and states that `-C` goes before the command (it is not a global flag). Second evaluation pass (same day): (a) **Interactive flags are excluded by a declarative clap marker**, `help_heading = "Interactive"`, on `commit -i`, `branch update -i`, `reword --diff/--no-diff`; humans get an "Interactive:" section in `--help`, the generator skips that heading. This is the human-vs-agent split Kiril asked for, in one place per flag. (b) **The tool already guards the prompt paths** (probed with stdin not a TTY, `perl -e 'alarm 15'`): `commit` without `-m` commits with an empty message and never opens the editor (even with `EDITOR=vim`); `squash` without a message flag completes; `commit -i` fails "Terminal doesn't support interactivity"; `branch update -i` ignores the flag; `merge` without `--yes` refuses and names the flag; `pr new` without `-m/-F/-t` refuses and names all three; `push` without a branch pushes every stack; `reword` without `-m` refuses. So the SKILL.md claim "without a flag an editor opens and blocks" is a fossil for non-TTY agents (Phase 2). (c) **Grouping** now shares `help::grouped_subcommands` with `but --help`, so the reference has the same six `## Group` headings and order. (d) **Help text fixed where facts were missing** (Kiril: fix `--help` itself, it serves humans too): `push` positional says ancestors are pushed and what omission does per mode; `pr new` about says it pushes first and what non-interactive runs need; `commit -m`/`squash -m` say what happens without a message flag; `pull` long help says a branch's own remote commits stay unintegrated; `pull --check` fossil removed; `status -f` reworded; "branchs" fixed. Third pass (same day, Kiril's calls): preamble is bullets and carries the recovery fact once ("mutations are recorded in the operation log; `but undo` reverts the last one"); pruned `teardown`, the `config` leaves except `forge auth`, and eight display/forge-polling flags (`status --refresh-prs/--no-hint`, `branch list --review/--no-check/--no-ahead/--empty`, `branch show --review/--ai`) via a path list in `reference.rs`; prompt-first abouts rewritten so the first paragraph says what the command does and a second paragraph states both modes (`pr auto-merge/set-draft/set-ready/template`, `reword -m`), selector lists turned into prose. The convention is now in `crates/but/CLAUDE.md`: first paragraph = what it does, omission behaviour states both modes, terminal-only flags take `help_heading = "Interactive"`. The reference cannot describe outputs (what a command prints), so the core skill owns the loop, ID lifetimes, and chaining. Fourth pass (prompting-agents angle, same day): near-synonym abouts now state the distinguishing effect (`unapply` keeps the branch to apply later, `branch delete` removes the commits too since it runs the discard operation, `uncommit` moves commits back to the uncommitted area); `resolve` about says `conflicts`/`apply` stay in the workspace while `resolve <commit>` enters a mode that lasts until `finish`; `worktree` (feature-flagged) and `branch new --switch` (leaves the workspace model) skipped; "Whether to…", "your", "we default" rewritten to effects; abouts unified to the imperative (`diff`, `show`, `status`, `setup`, `absorb`, `pull`); preamble points at `but skill concepts` for terms; `move` sources list turned into one sentence. Open: bare `but discard` wipes the worktree with no non-TTY gate (product guardrail decision, cf. `merge --yes`); the `squash` sources if-chain rewrite waits for the bench (saturated task = control). Note: `crates/but/tests/but/command/snapshots/help/no-arg.stdout.term.svg` is an orphan snapshot (no test references it) still carrying old abouts. Fifth pass: `resolve` about back to one sentence (the two-sentence version swamped `but --help` and lost its backticks), steering moved into the `[TARGETS]` help which the reference prints; pure routers (`branch`, `oplog`, `config`, `config forge`) skipped by the generator (subcommands and no own visible args); `pick <SOURCES>` states SHA or applied-branch CLI ID and that unapplied-branch IDs do not resolve (the September copy-task failure); `diff [TARGET]` names the entities and says a commit lists hunk IDs; `commit [CHANGES]` says omission commits everything; `amend -t` says a branch means its newest commit; value names `show <COMMIT_OR_BRANCH>` and `push [BRANCH]`. Deferred: `squash` sources rewrite, `setup --init` second person, `pr new --with-force` "defaults to true". Output 19.0 KB / 247 lines. Full CLI suite green (769) on a pristine copy; the three `legacy::status::tui` lib failures are pre-existing on HEAD. Remaining Phase 2 wording: `pull` about + "old `but base check`" fossil, prompt-first `pr` abouts, vague `status -f`, `push` silent on ancestors, "branchs", markdown lists in `move`/`pr` doc comments collapsing into prose. Deviations from the format decision: no group headings (flat, in clap declaration order; reusing `help.rs`'s group match would have meant refactoring it); router commands get a two-line entry. Installed files untouched.
13. **Granular task docs behind the stub.** Split core by the branches an agent takes, not by file: `but skill` keeps the loop, IDs, rules, git-to-but map, and the router (~80 lines); `history` (amend, split, reorder, squash, stack), `conflicts` (pull, dependency and committed and uncommitted conflicts), `review` (push, pr new, drafts, auto-merge) take the existing recipes; `examples` dissolves into them; `concepts` stays; `reference` is generated (item 12). The stub advertises `but skill` first, then the three task pointers by intent. Cap the menu at three: agent-browser and WrenAI both needed a skill-selection eval category once they had a menu, and Cloudflare's umbrella-plus-specialists bundle double-loads (cloudflare/skills #85). Core must stay sufficient for the commit-selected-changes fast path on its own. Mechanically this is one `SkillFile` and one subcommand per doc. Evaluate in the same round as stub-versus-full, with a selection case per task doc in the loading eval.

## 5. Overhaul plan (2026-09-09)

Measuring stick: `/Users/kiril/src/version-control-bench`. Facts that shape the plan (from `docs/optimizing-gitbutler.md` and `docs/results/expansion-2026-09-08.md`):

- The original six tasks are saturated (10/10, 2–6 calls) and serve as regression controls. The seven September expansion tasks (pilot-7..13) have headroom where the skill matters: dual-source sync 2/3 and 1/3 eligible, local integration and selective copy at roughly twice git's wall time and calls on both agents.
- `comparison:run` is a paired skill-or-binary A/B driver for pilot-7..13 (`--skill-dir`, `--but-bin`, alternating order, input hashes, pair table). The upstream-guidance experiment (`docs/experiments/upstream-guidance-2026-09-08.patch`) is the template: sync −37.3s/−2.5 calls on Codex, −18.5s/−5.25 calls on Fable, and +17.6s on the adjacent copy probe. Wording moves the number, and side effects on unrelated tasks are real.
- The runner only needs `SKILL.md` with frontmatter (version `0.0.0` or matching the binary) in `--skill-dir`; `references/` is optional. A stub arm is a one-file directory. The managed AGENTS.md block is rendered from `but agent setup --print` of the selected binary, so it is part of the arm.
- Gap: `skill_reference_output_bytes` (`scripts/run-pilot-agent.mjs`, `transcriptBreakdown`) counts only tool results whose input mentions the installed skill path. Served `but skill …` output must be classified the same way or the stub arm's context footprint is miscounted.
- Method rules from the runbook that apply to every step: hypothesis from a trace (read inefficient successes), name the behavioural evidence before the trial, one change per trial, control task plus the other agent, independent reviewer before and after, retire steering that agents ignore.

### Phase 0: mechanical, unblocked

- Clap-parse guard over every fenced `but …` line in `crates/but/skill/**/*.md` (follow-up 1).
- Generate `but skill reference` from the clap tree; fold single-command snippets from `examples.md` into clap `## Examples` (follow-up 12).
- Runner: count `but skill` tool outputs as skill bytes.
- Description-recall eval per playwright-cli #410 (fictional name, 40/10 prompts, Codex and Claude judges). This is the one thing vcbench cannot measure: it always installs the skill and points AGENTS.md at it.

The single-turn loading/selection eval from §3.1 is dropped: vcbench traces already show whether the agent ran `but skill`, which doc it pulled, and what it did next.

### Phase 1: delivery decision

Arms: `full` (today), `stub` (one-file dir, binary whose managed block routes to `but skill`), `help` (no skill, managed block only). One paired plan per headroom task (sync, integration, copy, plus scoped review as a read-only control), both agents, k=5. Behavioural evidence to look for: the stub arm runs `but skill` once and nothing else before the first real command; reference detours (`skill_reference_output_bytes`) do not grow.

### Phase 2: language, one change class per paired trial

Candidate edits come from three sources compared: `/claude-api prompt-audit` over the skill and managed block, Brodin's audit prompt, and the detours in the September traces (`tmp/overnight-20260908/`). Rank by avoidable work in traces, not by checklist order. Baseline counts for the 220-line core: 24 "do not", 9 "never", 6 "must", 3 "always" in 3.7k words.

Change classes, each its own trial with a six-task control batch:

1. Real constraints stay with the reason beside them (no git writes, merged-upstream refusal, `-m` or an editor blocks). Drop the "Non-Negotiable" header.
2. Incident-shaped prohibitions become the positive statement once ("do not invent `--changes`", "`uvw:16-40` is invalid", "`nk,pn` is one ID").
3. Re-tier recipes vs bridges: numbered steps only where order is fragile (conflict loop bottom-up, split with a preserved block).
4. Completion criteria per task; today only the commit-selected fast path says where to stop.
5. Description: collapse the synonym wall to intent branches plus quoted "Use when the user says" phrases; confirm with the recall eval.
6. Fossils: `AGENT ACTION REQUIRED` note, retired-syntax wording, rule 5 ("prefer this skill over help", inverted once reference is generated), rule 4 (no progress narration; model-specific, leaves a multi-model skill).
7. Kiril's global rules file: retired commit syntax, duplicated rules.

### Phase 3: organization

Core `but skill` becomes an ~80-line router (loop, IDs, rules with reasons, git-to-but map, pointers). Task docs are derived from the trace clusters, not decided up front: the measured detours are sync (a branch's own upstream), integration, and selective copy, which do not fit the earlier `history`/`conflicts`/`review` guess cleanly. `examples` dissolves; `concepts` stays; `reference` is generated. Menu capped at three. Selection evidence comes from traces (which doc was pulled for which task).

### Phase 4: after the bench picks a winner

Flip the install default, migrate full installs to stubs on `check --update` with a layout guard test, delete the freshness auto-update, add the "Start here" block to `but --help`, reword notices to point at `but skill`. Cold-adoption task pilot-8 is the check.

Rules: never restructure and reword in one trial; no doc menu larger than three task docs; no tuning toward oracle vocabulary.

## 6. Stub delivery trial (2026-09-09)

The butgym stub run (`~/.local/state/butgym/stub-skill-20260909/`) kept correctness and lost time, and the traces put the cause in how agents fetch the guide, not in what it says: Fable head-limits `but skill` to 150–200 of 214 lines and `--full` to 400 of ~1300, then searches the slice for sections past the cut; Astra ingests all 84 KB of `--full`; both bundle the guide with `but status` so the inspection output hides behind the preview. The harness also dropped the managed AGENTS block in stub mode, which confounds the Astra verification detours.

Change under test, branch `stub-guide-loading` (`ad16817`, cherry-picks onto the tested `5b00189`): the stub names the guide's size, asks for a call of its own with the reason, and offers only `reference` as a conditional second read. Behavioural evidence before timing: no `head` on `but skill`, no `--full`, no saved-output re-reads, guide never in the same call as status. Rerun with the managed block restored in both arms and the binary held at `5b00189`.
