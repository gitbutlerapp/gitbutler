# Who else serves skills from the CLI, and what broke (Sept 2026)

Research on 2026-09-08 into the "thin installed stub, content served by the binary" pattern that [but-skill-served-from-cli.md](but-skill-served-from-cli.md) adopts. Sources: GitHub PRs, issues, commits, and docs of the projects below. Companion to [agent-browser-case-study-2026-09.md](agent-browser-case-study-2026-09.md).

## 1. Adopters

| Project | Since | Command | Notes |
|---|---|---|---|
| agent-browser (Vercel Labs, Rust) | 2026-04-12, PR #1225 | `agent-browser skills get core [--full]` | Origin of the pattern. Stub is `hidden: true`. 20-case eval suite shipped with it. |
| WrenAI (Canner, Python) | 2026-06-04, PR #2329 | `wren skills get <name> [--full] [--script]` | "Modeled after vercel-labs/agent-browser's single-stub pattern." Deleted five installed skills in favour of one stub. |
| Metabase (`@metabase/cli`) | 2026 | `mb skills get` | Skill repo README: "a thin discovery stub where the workflow content lives in @metabase/cli." |
| Orca (`onorca.dev`) | 2026 | `orca skills get <topic> [--full]` | Coined "hybrid discovery stubs." Stub also resolves which binary to run (`ORCA_CLI_COMMAND`, `orca-dev`, `orca`). App auto-updates installed stub packages in the background. |
| agentuse | 2026 | `agentuse skills get core [--full]` | Stub is `hidden: true`, routes to `runner`, `creator`, `onboarding`. |
| Browser4, sunat-cli, metabigor, univer-workspace-cli | 2026 | `<tool> skills get` | Smaller adopters found by code search. |

Half-adopters, worth knowing about:

- **logchef** (Rust): embeds the skill with `include_dir!` and has `logchef skills get core [--full] [--json]`, but still installs the full skill files; a sync script plus CI keeps both copies identical. Source comment: an embed path that escapes the Cargo workspace "breaks `cross` release builds, which mount only the workspace," so the skill lives inside the crate. `but` already embeds from `crates/but/skill`.
- **GitHub CLI** `gh skill install|search|update|publish|preview` (2026-04-16): a package manager for repo-hosted skills, not binary-served content. Update detection compares git tree SHAs rather than version strings.
- **Atlassian TWG CLI**: installs files to `~/.agents/skills`, refresh by re-running `twg skills install`. No version sync.

## 2. Stated rationale, in their words

- agent-browser PR #1225: "Serves bundled skill content at runtime, always matching the installed CLI version. Solves the problem of agents relying on stale cached SKILL.md files after upgrades."
- agent-browser docs: "the installed SKILL.md rarely changes, while the CLI always serves content matching its own version."
- WrenAI PR #2329: "That bundle drifted from the installed CLI (the markdown referenced commands and flags that didn't always match the user's `pip install`-ed version), and the agent loaded all five skills at session start whether it needed them or not. The new model ships the content inside the `wrenai` wheel itself, so the version an agent reads always matches the installed CLI, and the agent only pays for what it fetches."
- Orca docs: "Command flags live in the binary so they cannot drift from the app version."

Two motives recur: version drift, and context cost of loading every skill up front.

## 3. What went wrong, and whether it applies to `but`

| Incident | What happened | Applies to `but`? |
|---|---|---|
| agent-browser PR #1253 (4 days after launch) | The first served content was itself the ~40-line stub: "For an agent already inside the CLI trying to learn how to use it, that's a dead-end." Real guide moved to `skill-data/core`, 420 lines. | No. `but skill` serves the full SKILL.md body from day one. |
| agent-browser issue #1337 (open since 2026-05, no maintainer reply) | `hidden: true` in the stub's frontmatter makes OpenCode's `skill()` tool refuse to load it: "Skill or command 'agent-browser' not found." Reporter proposes keeping the skill visible with a body that redirects. | Not directly: the `but` stub uses `stub: true` and keeps the description. But it shows harnesses read unknown frontmatter keys. Verify Claude Code, Codex, and OpenCode ignore `stub:` before the flag goes public. |
| WrenAI issue #2341 | User installed the stub from the docs' install script while `pip` still served CLI 0.8.1, which had no `wren skills` command. The stub pointed at a command that did not exist. | Low risk while `but skill install --stub` is the only distribution path, since the binary that writes the stub is the binary that serves it. Becomes real if the stub is ever published to a skills registry ahead of the binary. |
| Metabase issue #35 | Stub description exceeded the agentskills.io spec maximum; the `pi` agent errored. WrenAI's stub description is likewise a wall of triggers. | Already guarded: `skill/AGENTS.md` caps the description at 1024 characters, and the stub reuses the existing description verbatim. |
| agent-browser issue #963 | `npx skills add` installed five skills, not one, because the npm `files` array shipped all of them. Fixed by narrowing the array. | No equivalent; `but` writes exactly the files it intends. |
| agent-browser issue #1351 | Turn count rose 65% versus MCP Playwright because the skill tells agents to re-snapshot after every action. Not stub-related, but every CLI read is a turn. | The stub adds one `but skill` read per session. The loading eval and the vcbench arm measure that. |

No adopter has reverted. agent-browser's stub has been the sole install path since April across several releases; WrenAI deleted its fat skills permanently and added a regression test so they cannot come back.

## 4. Evidence that it works

Thin. Nobody has published an A/B of stub versus full install.

- agent-browser: "Eval framework ... 20 cases ... All 20 cases pass at 100%" (PR #1225). Cases are single-turn regex checks that the agent runs `skills get` before acting and picks the right specialized skill. No task-outcome or wall-time comparison.
- WrenAI: CI guard "validates ~440 `wren <cmd>` invocations across served content against the real Typer command tree." A correctness guard on the served text, not a behavioural measurement.
- Everything else is adoption and absence of complaints about agent behaviour. The issues filed are all packaging and distribution, none say "the agent didn't load the guide."

So the `but+stub` vcbench arm in the design doc would be the first outcome-level measurement anyone has run on this pattern.

## 5. Worth stealing

1. **Served-content command guard** (WrenAI, PR #2329 commit `9ba96f6f`): parse every `wren <cmd>` invocation in the skill markdown against the real command tree in CI. For `but`, a test that extracts every fenced `but ...` line from `skill/**/*.md` and runs it through clap's `try_get_matches_from` would have caught the retired-syntax drift that motivated this whole change. Cheap and high value regardless of the stub decision.
2. **"Start here (for AI agents)" block at the top of `--help`** (agent-browser PR #1251): "Agents skimming `--help` pass right over it on the way to flag docs, then waste turns guessing commands when a hand-written workflow guide is one command away." Gated on the evaluation for `but`, but it is the natural home for the `but skill` pointer.
3. **Redirect stubs during migration, then a test that they stay deleted** (WrenAI): when the default flips, the old four-file layout can be rewritten to a stub by `check --update` once, and a test guards that the full layout is not silently regenerated.
4. **Binary resolution in the stub** (Orca): the stub names which executable to run. Relevant to vcbench's `--but-bin` and to dev builds; a `BUT_BIN` style override may be worth a line in the stub.
5. **Keep the stub visible** (agent-browser #1337 lesson): do not use `hidden:`; use a body that redirects. `but` already does this.

## 6. Bottom line

The pattern has five to ten independent adopters over five months, one of them explicitly copying agent-browser, and no reversals. The failures on record are all about packaging (the served content being empty, the stub reaching users before the binary, a frontmatter key a harness treats as "do not load," description length). None are about agents failing to follow the redirect. `but`'s implementation avoids each of the recorded failures by construction, except the harness-frontmatter one, which needs a quick check across Claude Code, Codex, and OpenCode. Behavioural evidence is absent industry-wide, which is exactly what the planned eval and bench arm supply.

---

## 7. Second sweep: larger projects and their issue trackers

Added 2026-09-08 after checking microsoft/playwright-cli, getsentry/cli, cloudflare/skills, openai/skills, anthropics/skills, vercel-labs/skills, supabase/cli, stripe/stripe-cli. Only the first four had anything relevant.

### 7.1 playwright-cli (Microsoft): the counter-position

Microsoft installs full skill files and nudges on drift; it did not adopt the stub. Its maintainers' stated contract, issue #415: "CLI does not respect semver, we reserve right to rename and change everything with every release as long as we keep the skills up-to-date. Latest CLI should always work with the latest skills." On auto-update, issue #421: "cli will now print skills and suggest updating, that's all we can do for now," and "This seems to be a problem for the ecosystem to solve." That is exactly `but`'s current freshness notice. The two biggest agent browser CLIs therefore split: Vercel serves from the binary, Microsoft reinstalls files. Both are alive and neither has moved. The `but` evaluation is choosing between two live positions, not validating a consensus.

Issue #410 is the single most useful artifact in the sweep: a measured description-recall study. Method: rename the skill to a fictional name so the judge cannot lean on prior knowledge, 40 should-trigger and 10 should-not prompts, run through `codex` and `claude -p --bare`, score recall and precision.

| Description style | Codex recall | Opus recall |
|---|---|---|
| Baseline, one abstract sentence | 0.800 | 0.975 |
| "Use when the user says: '…', '…'" quoted phrases | 0.975 | 0.975 |

Precision was 1.0 everywhere. The maintainer refused a single-model tune: "We would not want to tune it to Codex narrowly, what are the Opus stats?" Two lessons: quoted trigger phrases helped Codex a lot and Opus not at all, and any description change needs a dual-judge number. This is the method for evaluating the `but` description, and it partly contradicts the prompt-audit warning against trigger enumeration: on Codex, enumerated phrases won.

### 7.2 Sentry CLI (getsentry/cli): generated skills, auto-install, and the backlash

A fourth delivery model. `script/generate-skill.ts` introspects the command route tree and merges it with docs to produce SKILL.md plus one reference per command; the result is embedded at build time; `cli setup` and `upgrade` install it automatically; it is also served at `.well-known/skills/`; and `script/eval-skill.ts` runs in CI with Sonnet 5 and GPT-5.6 planning commands, a judge grounded by running `-h` on the real binary, and a 0.75 pass threshold.

Issue #1403 is the backlash to auto-install, worth quoting because every complaint transfers: "That's a security risk!", "clobbers my customized skills", "adds churn for my agent", "25KB of context that mirrors `sentry --help`", "my org may not use sentry exactly the same way as other orgs, so a custom skill is more useful than a generic one." The maintainers shipped `--no-agent-skills`, persisted the preference, fixed the content, and kept opt-out as "a product decision we feel strongly about."

For `but`: the freshness auto-update already rewrites installed skill files in place, which is the "clobbers my customized skills" complaint waiting to happen for anyone who edited theirs. A stub has nothing to customize, so it shrinks that surface. And "mirrors `--help`" is the cache-of-environment critique: `reference.md` is 663 lines that largely restate clap. Sentry's answer is to generate the reference from the command tree so it cannot drift; that is the heavier alternative to the parse guard in section 5.

### 7.3 Cloudflare skills: drift against the world, and a validator that rejects unknown frontmatter

Issue #90: six skill files taught an import removed from a dependency, "12 occurrences across 6 files," so agents following the skill produced projects whose test runner could not start. Serving from the binary does not fix this class; it only fixes drift against the CLI itself. `but`'s skill references almost nothing but `but`, so exposure is low.

Issue #85: "Codex's current skill validator rejects that field," about a top-level `references:` frontmatter key. This is a second, independent report that harnesses do not ignore unknown frontmatter. Together with agent-browser #1337 it makes follow-up 2 concrete: run the `but` stub through Codex's `skill-creator` `quick_validate` and OpenCode's loader before the flag goes public, and check `allowed-tools` as well as `stub`. The same issue reports an 897-line SKILL.md against a "recommended sub-500-line budget" and an umbrella skill whose broad trigger fires alongside the specialists it routes to.

Issue #84: a third-party skill manager auto-skipped twelve official skills because user-local copies existed, so users silently stopped getting updates. Another argument for content that comes from the binary rather than from files a manager owns.

### 7.4 Smaller signals

- openai/skills #420: the canonical user skill directory moved from `~/.codex/skills` to `~/.agents/skills` and OpenAI's own installer lagged its docs. Directory conventions move; `but`'s per-agent format table needs the same maintenance.
- getsentry/sentry-cli #3313: a user asking for "a skill wrapper for this cli instead of official MCP server, since MCP server take so much token." Demand for the CLI-plus-skill shape over MCP, consistent with agent-browser's context-footprint eval.
- playwright-cli #315: `install --skill` (singular) silently succeeded without installing anything. Reject near-miss flags loudly.

### 7.5 What changes in the plan

1. The stub is not the only defensible end state. Microsoft's reinstall-and-nudge is the other, and it is what `but` does today. The vcbench arm decides between them on evidence rather than fashion.
2. Verify the stub's frontmatter against Codex's validator and OpenCode's loader, keys `stub` and `allowed-tools`. Two independent projects hit this.
3. Evaluate the description with the #410 method, dual-judge, before and after any wording change.
4. Decide what the freshness auto-update should do with a user-edited skill file. Today it overwrites. Sentry paid for that.
5. Consider generating `reference.md` from clap instead of maintaining it, as the stronger form of the parse guard.
