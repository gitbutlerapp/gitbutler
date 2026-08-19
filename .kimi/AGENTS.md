<!-- nixos:begin agent-policy -->
<!-- GENEROWANE z /etc/nixos/configuration.nix. Nie edytuj między markerami; zmień źródło i uruchom: sudo nixos-rebuild switch -->

# ss
- **ss** (global WSL skill, source `~/skills/ss/`, linked in `~/.claude/skills/ss/`, `~/.agents/skills/ss/`, `~/.codex/skills/ss/`, `~/.cursor/skills/ss/`, `~/.kimi/skills/ss/`) - load image or text from WSL/Wayland clipboard. Trigger: `/ss`, `ss`, "screenshot", "wklej grafikę", "wczytaj screen"
When the user types `/ss` or `ss`, invoke `/skill:ss` (or auto-invoke the `ss` skill) and follow its `SKILL.md` before doing anything else.
Kimi discovers skills from `~/.kimi/skills`, `~/.agents/skills`, and (with `merge_all_available_skills`) also `~/.claude/skills` / `~/.codex/skills`.


# pplx
- **pplx** (global skill, source `/etc/nixos/skills/pplx/`, linked in `~/.claude/skills/pplx/`, `~/.agents/skills/pplx/`, `~/.codex/skills/pplx/`, `~/.cursor/skills/pplx/`, `~/.kimi/skills/pplx/`) - web research with cited sources through the Perplexity Pro account via `~/bin/pplx ask "<question>"` or `~/bin/pplx research "<question>"` (on Windows: `pplx ask ...` talks to the loopback HTTP bridge at 127.0.0.1:18765; never call `wsl.exe` for pplx). Trigger: `/pplx`, `pplx`, "mcp pplx", "perplexity", "zbadaj w perplexity", "zrób research", "znajdź w internecie", "poszukaj w internecie", "sprawdź w internecie", or any task that depends on current external facts (third-party API docs, breaking changes, regulations, market data).
Load the `pplx` skill and follow its `SKILL.md` before running the helper. Exit code 3 means the user has to sign in with `pplx login` themselves; the agent does not automate the sign-in and does not touch the browser profile.
`pplx` is a shell command, not an MCP server: when the user says "mcp pplx", "pplx mcp" or "przez perplexity", run the command; never report it as a missing MCP. While `pplx` works, do not replace it with a built-in web search tool; use built-in search only after `pplx` failed in this turn, and say so.


# herdr
- **herdr** (global skill, source `/etc/nixos/skills/herdr/`, linked in `~/.claude/skills/herdr/`, `~/.agents/skills/herdr/`, `~/.codex/skills/herdr/`, `~/.cursor/skills/herdr/`, `~/.kimi/skills/herdr/`; Windows: `C:\Users\robert\skills\herdr` junctioned into Claude/Codex/Cursor/Agents) - control Herdr panes, tabs, workspaces, and agents via the `herdr` CLI. Trigger only when the user explicitly mentions Herdr or asks to inspect/control Herdr panes, tabs, workspaces, commands, or another agent. Requires `HERDR_ENV=1` (agent must run inside a Herdr-managed pane). Load the `herdr` skill and follow its `SKILL.md`; if `HERDR_ENV` is unset, say you are not inside Herdr and stop.


# mailbird-reply
- **mailbird-reply** (global skill, source `/etc/nixos/skills/mailbird-reply/`, linked in `~/.claude/skills/mailbird-reply/`, `~/.agents/skills/mailbird-reply/`, `~/.codex/skills/mailbird-reply/`, `~/.cursor/skills/mailbird-reply/`, `~/.kimi/skills/mailbird-reply/`; Windows: `C:\Users\robert\skills\mailbird-reply` junctioned into Claude/Codex/Cursor/Agents) - mandatory procedure for every customer email reply, draft or offer in Mailbird. Trigger: any request to answer, reply to, draft or write an email ("odpowiedz", "odpowiedź mailbird", "napisz maila", "szkic", "Re: ...", "zaproponuj mu", "wyślij ofertę").
Load the `mailbird-reply` skill and follow its `SKILL.md` before reading the thread or calling any Mailbird draft tool: search Graphiti email rules first, verify every fact yourself, never ask for what can be found, no placeholders, Mailbird HTML (`__MailbirdStyleContent` + full `mb_sig`) already in the first `reply_to_conversation` call, never fix a draft with `update_draft`. Do not install Claude Code hooks for this skill.


# Mandatory fast customer and environment check

Before customer-specific SSH, SQL, API calls, diagnostics, or preparing operational SQL, check the actual target in the existing registry/configuration. This is mandatory for reads as well as writes, in every agent session. Repeat when the customer/environment changes or a resumed session lacks the checked target. The purpose is a quick, concrete lookup, not a new access-control system or routine approval ceremony.

Never infer a customer, installation, host, or database from similar names, IP suffixes, neighboring addresses, an unrelated deployment script, memory, or a previous session's conclusion. A log's account ID is a lookup key, not proof that the log concerns the customer currently named by the user. A working connection alone does not identify its owner.

Fast lookup for WebArm / Optima:
1. Resolve the account and API endpoint in the selected WebArm environment with `SELECT a.id, a.name, a.nip, s.optima_api_url, s.company_mode FROM account a LEFT JOIN account_setting s ON s.account_id = a.id WHERE a.id = %s` (database `webarm_ksef`) or an exact name lookup. Resolve the company separately with `SELECT id, account_id, optima_firma_id, optima_firma, company_name, nip FROM company WHERE account_id = %s AND optima_firma_id = %s`. Always scope company IDs to the account. The accounting office/account and a company it serves are different identities. MariaDB MCP parameters use `%s`; bind values instead of interpolating SQL.
2. Read the SSH target from `C:\webarm\optima-api\vault\INVENTORY.md` and the matching `vault/<installation>/meta.json` (`host`, `server`, `hostname`, `harvestedAt`, `status`); WSL root is `/mnt/c/webarm/optima-api`. Use the explicitly matched installation, not a similar inventory key. `deploy-<installation>.ps1` is an additional configuration lookup, never a reason to execute a deployment. Do not decrypt or print credentials merely to identify a target.
3. On the selected SSH connection, first compare `hostname` with that installation's metadata. For Optima SQL, read `SELECT @@SERVERNAME AS server_name, DB_NAME() AS database_name`; resolve the selected company in the configured Optima configuration database with `SELECT Baz_BazID, Baz_Nazwa, Baz_NazwaSerwera, Baz_NazwaBazy FROM CDN.Bazy WHERE Baz_BazID = @id`. Use this exact server/database mapping, not a database name guessed from a company name. Recheck the target on the same connection before writes.
4. Keep a short resolved target in the session: customer/account, environment, installation/SSH host, and company/database when relevant. For other projects, use their actual account registry and deployment inventory in the same way. Do not copy live customer mappings into global instructions; look them up where they are maintained.

A complete matching lookup means continue immediately within the user's authorized scope, without extra approval prompts. If a name/alias is absent, records conflict, or the target differs from the user's request, state the exact discrepancy and ask one focused question. Never silently substitute another customer or reuse SQL prepared for another account. Do not turn a normal lookup into broad infrastructure research.


# Source code comments
Do not add comments in source files unless the user explicitly asks in this turn. Do not edit or delete existing comments unless asked or they became false because of your change.


# Browser windows for user logins
When the user must log in or confirm something in a browser (Google Ads, Surfer, any web panel), never open that window with `agent-browser --headed`: agent-browser closes its browser when the command ends, so the window disappears within seconds. Launch a standalone system Chromium in the background with a persistent profile and a CDP port, and leave it open until the user closes it:
`nohup /run/current-system/sw/bin/chromium --user-data-dir=$HOME/.agent-browser-profiles/<service> --remote-debugging-port=<port> --no-first-run <url> >/dev/null 2>&1 & disown`
Before telling the user to log in, verify the window is still up with `curl http://127.0.0.1:<port>/json/list`. After the login, drive it through CDP (`agent-browser connect <port>`) and never close it. Reuse the same profile directory next time so the login persists. On Windows, start Chrome or Edge the same way with `--user-data-dir` and `--remote-debugging-port`. Perplexity keeps its own flow: the user runs `pplx login`.


# GitButler version control (canonical — overrides generic "commit only when asked" guidance)
- Use one shared GitButler workspace for multiple coding agents (WSL, Windows, every project).
- HARD BAN: NEVER disable GitButler. Forbidden: `but teardown`, checking out `main` to leave `gitbutler/workspace`, deleting `gitbutler/workspace` or `gitbutler/target`. No exceptions — not for a clean tree, one branch, or no worktrees.
- Work ONLY in a dedicated GitButler branch/stack for this agent session. Commit only this session's own files/hunks. Never mix other agents' changes into your commit.
- HARD BAN: NEVER discard, revert, undo, restore-over, checkout away, or otherwise destroy another agent's uncommitted WIP. No exceptions — not for deploy, build, tests, or a clean tree.
- If deploy, build, or tests fail because another agent has in-progress work, WAIT. Do not clear or rewrite their changes to unblock yourself.
- Load and follow the GitButler `but` skill before any version-control write. If `but` prints `AGENT ACTION REQUIRED`, run the suggested `but skill install` / `but skill check --update` once, reload the skill, then continue.
- Use the GitButler CLI (`but`) for all version-control write operations (never plain `git commit` / `git push`).
- Use the installed GitButler skill for command recipes and syntax before guessing flags, using `--help`, or translating Git habits directly.
- To commit only selected changes, use the skill's two-command fast path: `but diff`, then `but commit -b <branch> -m "message" <id> <id>`; after the commit succeeds, stop and summarize.
- Mutation commands report their result without appending workspace status. Add `--status-after` only when the next step needs resulting workspace IDs or details; do not rerun status or diff just to verify success.
- Keep commit messages succinct: what changed, why it changed, and any important decision.
- When requested work is finished, always commit this session's files/hunks with `but` (inspect `but status` / `but diff` first). Do not ask whether to commit.
- If branch or hunk ownership is ambiguous, stop and ask instead of committing.
- Do not rely on MCP to commit; the agent must run `but` itself before ending the turn.
- Never push, open a PR/MR, or use `but land`; create local commits only, with no exceptions.

### Amend local fixes into the right commits
- For small cleanup or follow-up fixes, amend an unpublished local commit when the change clearly belongs with that commit's intent.
- Do not create tiny fixup commits unless the user asks.
- Use GitButler to move the relevant changes into the commit where they belong.
- Ask before rewriting pushed, reviewed, shared, or ambiguous history.

### Split unrelated changes into separate commits
- If one file contains unrelated changes, split them by hunk instead of committing the whole file.
- Keep tests with the behavior they verify.
- Split generated output, docs-only edits, or mechanical cleanup into separate commits when each commit remains coherent on its own.
- If the split is ambiguous, summarize the options before committing.

### Update from the target branch automatically
- When GitButler status shows new changes on the target branch and the workspace holds only this session's branches, update with `but pull` directly — its output reports the result and `but undo` reverts it.
- If an update you started on your own initiative reports conflicted commits, stop and ask before resolving them (`but undo` reverts the pull if the user prefers).
- When other agents' branches are applied, run `but pull --check` first and ask before updating if it reports conflicts or their branches would move.
- If the user asks you to handle update conflicts, use GitButler's conflict tools. Ask before resolving semantic conflicts, dependency updates, generated files, or conflicts involving another person's work.

### Commit checkpoints after each turn
- Commit after a working checkpoint, when the requested change is complete and relevant checks have passed or been reported.
- Treat checkpoint commits as local savepoints, not final review history.
- When the user asks you to tidy the history, use GitButler to squash commits, reword commits, and move changes between commits where appropriate.
- Only tidy unpublished local history unless the user explicitly authorizes changing pushed or shared history.


## Instructions for Using Graphiti's MCP Tools for Agent Memory

Synced from official getzep/graphiti `mcp-v1.1.0` `mcp_server/docs/cursor_rules.md`, with local corrections:
- Tool name is `search_memory_facts` (live MCP API), not `search_facts`.
- Always use `group_id` / `group_ids`: `main`.
- If Graphiti MCP is unavailable, stop; do not bypass.

Before every task, load and follow the global `graphiti-memory` skill.

### Before Starting Any Task

- **Always search first:** Use the `search_nodes` tool to look for relevant preferences and procedures before beginning work.
- **Search for facts too:** Use the `search_memory_facts` tool to discover relationships and factual information that may be relevant to your task.
- **Filter by entity type:** Specify `Preference`, `Procedure`, or `Requirement` in your node search to get targeted results.
- **Scope to group `main`:** Pass `group_ids: ["main"]` (and `group_id: "main"` when writing).
- **Review all matches:** Carefully examine any preferences, procedures, or facts that match your current task.
- **If MCP fails:** Surface the exact error and stop. Do not call Graphiti over unofficial HTTP/curl or invent workarounds.

### Always Save New or Updated Information

- **Capture requirements and preferences immediately:** When a user expresses a requirement or preference, use `add_memory` to store it right away.
  - _Best practice:_ Split very long requirements into shorter, logical chunks.
- **Be explicit if something is an update to existing knowledge.** Only add what's changed or new to the graph.
- **Document procedures clearly:** When you discover how a user wants things done, record it as a procedure.
- **Record factual relationships:** When you learn about connections between entities, store these as facts.
- **Be specific with categories:** Label preferences and procedures with clear categories for better retrieval later.

### During Your Work

- **Respect discovered preferences:** Align your work with any preferences you've found.
- **Follow procedures exactly:** If you find a procedure for your current task, follow it step by step.
- **Apply relevant facts:** Use factual information to inform your decisions and recommendations.
- **Stay consistent:** Maintain consistency with previously identified preferences, procedures, and facts.

### Best Practices

- **Search before suggesting:** Always check if there's established knowledge before making recommendations.
- **Combine node and fact searches:** For complex tasks, search both nodes and facts to build a complete picture.
- **Use `center_node_uuid`:** When exploring related information, center your search around a specific node.
- **Prioritize specific matches:** More specific information takes precedence over general information.
- **Be proactive:** If you notice patterns in user behavior, consider storing them as preferences or procedures.

**Remember:** The knowledge graph is your memory. Use it consistently to provide personalized assistance that respects the user's established preferences, procedures, and factual context.


# Graphify (global only — never write project-local AGENTS/CLAUDE/Cursor graphify rules)
When `graphify-out/graph.json` exists, for codebase questions first run `graphify query "<question>"`, `graphify path "<A>" "<B>"`, or `graphify explain "<concept>"` before broad Read/Grep/Glob.
Prefer one `graphify watch .` per checkout. Do not run `graphify update .` after every edit.
Run `graphify update .` only if watch is down, after docs/images (`needs_update`), or after pull/merge when the graph is stale.
Use the native binary for that OS: Linux `graphify` on Linux trees, Windows `graphify.exe` on Windows/NTFS trees. Do not mix.


# Version control language
Commit messages, branch names, and commit/PR descriptions are ALWAYS in English. Conversation with the user stays in the user's language; this rule covers only Git/VCS artifacts.


# Fail-fast application policy

Do not use fallback behavior to hide errors. Build and modify applications in fail-fast mode so configuration, schema, integration, and runtime errors surface immediately and can be fixed directly.

Follow the principle: do it right the first time. Do not add compatibility shims, silent fallback paths, or error-masking behavior unless the user explicitly asks for a temporary emergency mitigation.


# optima-api (`C:\webarm\optima-api`)

- After functionality changes: validate locally, then run `.\deploy.ps1`; deploy bumps version.
- API tests: default to demo DB `webarm`.
- Kapelanczyk RM: read-only unless exceptional write is unavoidable; remove/undo it immediately.


# Test-Driven Development

When implementing features, bug fixes, refactors, or behavior changes, use the TDD workflow whenever practical.

Default to:
- write or update a focused failing test first,
- run the narrowest relevant test and verify the expected RED failure,
- implement the smallest production change needed,
- run the focused test and verify GREEN,
- refactor only after tests are green,
- run the relevant broader test suite before completion.

If test-first work is impractical, explain why before changing production code and use the strongest available verification instead.

Do not let TDD workflow management override existing project instructions, workspace management, memory/indexing requirements, or version-control preferences.


# No self attribution
Never add Co-Authored-By, session URL, or other attribution trailers for yourself or your vendor to commits or generated content.


<!-- nixos:end agent-policy -->
