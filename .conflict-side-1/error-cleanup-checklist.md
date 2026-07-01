# Error Cleanup Checklist

Tracking fixes for the top `toast:show_error` events identified via PostHog (last 30 days, prod builds only).

## Already done in PR #13312 ("Fix bugs and improve error handling")

- [x] `TypeError: undefined is not an object (i.type)` — `customHooks.svelte.ts` now uses RTK Query `unwrap()` (272 events / 81 users)
- [x] `erro_title` typo in PostHog capture — `error.ts:102`
- [x] `commiting` typo — `uncommittedService.svelte.ts:233` error string
- [x] `shouldIgnoreThistError` typo — `parser.ts` + `showError.ts`
- [x] Generate branch name disabled on empty branch — `BranchHeaderContextMenu.svelte` (~20 events/mo)
- [x] Rate-limit toast PostHog captures to 60/hour — `toasts.ts` (caps runaway spikes; mitigates most "Failed to fetch" storms)
- [x] 401 → actionable "Login token expired. Please log in to GitButler again." — `httpClient.ts` (120 events / 95 users)
- [x] `SilentError` for Octokit rate limiter — `ratelimit.ts` (~76 events)

## Bugs to fix

- [x] **Path is a directory** — return empty `FileInfo` for directories in `read_file_from_workspace` (`crates/gitbutler-repo/src/commands.rs:297`) — 188 events / 14 users
- [x] **Hunk not found while committing** — skip unmatched hunks instead of throwing in `uncommittedService.svelte.ts:233` — 56 events / 17 users
- [x] **Failed to fetch diff eliminated** — removed `throw` in `getUnifiedDiff`, widened return type to `UnifiedDiff | null`, callers already handle null via optional chaining; real IPC errors still surface via RTK `unwrap()` — 57 events / 11 users
- [x] **keychain_notfound telemetry fix** — `annotate_keychain_error` now wraps with `Context::new_static(Code, stable_msg)` for both SecretKeychainNotFound and MissingLoginKeychain (`crates/but-secret/src/secret.rs`) — 23 events / 21 users (telemetry only; UI already shows friendly text)

## UX conversions (noisy but legitimate)

- [x] **Generate commit message: info toast on no changes** — swap `showError` → info `showToast` in `macros.svelte.ts` (covers both commit-message and branch-name generation paths) — 104 events / 48 users
- [x] **WSL2 / UNC guidance as info toast** — `projectsService.ts:142-161` now uses `showToast({style: "info"})` for both unsupported-path cases — 53 events / 37 users
- [x] **install_cli osascript decline as info** — Rust side tags status-1 with `Code::CliInstallCancelled` (`but-action/src/cli.rs`); `GeneralSettings.svelte` matches on that code and shows info toast — 32 events / 24 users
- [x] **App update RO-filesystem: info toast with guidance** — detect "Read-only file system" / EROFS in `updater.ts:handleError` and show info toast linking to downloads — 14 events / 5 users

## Follow-ups from review

Concerns surfaced when reviewing the first round of fixes, addressed as
separate commits on the same branch.

- [x] **Silent whole-file commit risk** — `uncommittedService.svelte.ts` pushed `hunkHeaders: []` when every selected hunk in a file was stale, which the backend interprets as "commit whole file." Now we track stale-skip count per path and drop the file from the commit (plus an info toast) when every selection was stale. HIGH severity — potential data surprise.
- [x] **Directory vs. empty-file ambiguity** — introduced `FileInfo::directory()` as a semantic constructor for the directory case so the read path reads cleanly; did not add a dedicated marker field because no current consumer distinguishes directories from zero-byte files, and overloading `mime_type` with `inode/directory` would confuse the `ImageDiff` renderer that uses the field for `data:` URL building. If a future caller needs to differentiate, a proper `is_directory: bool` can be added then.
- [x] **osascript cancel string-matching** — added `Code::CliInstallCancelled` (`errors.cli.install_cancelled`), Rust side attaches it via `Context::new_static`, frontend matches on `getUserErrorCode(err) === Code.CliInstallCancelled` instead of the English message.
- [x] **RO-filesystem pattern English-centric / Linux-centric** — broadened to cover `os error 30` (Linux EROFS numeric), `os error 6032` (Windows `ERROR_WRITE_PROTECT`), and "write-protected" / "write protected" phrasing. Deliberately avoided bare "Permission denied" to prevent over-matching.
- [x] **Keychain annotation Linux-only** — renamed `annotate_linux_keychain` → `annotate_keychain_error` and attached a stable `"System keychain access failed"` label with `Code::Unknown` for macOS/Windows + unmatched Linux paths, so PostHog aggregates these rather than bucketing every localized error separately.

## Deferred (nice-to-haves from review, not actioned)

- **Disable AI-generate button when there are no changes** — would require reactive plumbing from selection/diff state up to the toolbar. The info-toast conversion already removes the error noise; this is a UI polish follow-up.
- **Modal/inline messaging for WSL2/UNC guidance** — info toast conveys the guidance; a modal would be more prominent but is a UX decision for a separate design pass.
- **Dedicated `Code` for macOS/Windows keychain** — those platforms always have a default keychain, so a bespoke Code has no known user-facing remediation to tie to. Revisit if telemetry shows a cluster.

## query:error path (high-volume silent RTK Query errors)

Context: `query:error` captures ~1.6M events / 14 days / 4.2k users — roughly 100× the volume of `toast:show_error`. Investigation (2026-04-16) showed massive single-user spam loops (one user repeatedly hitting a broken `stacks` / `list_reviews` call fires tens of thousands of times) and wide command-not-found fallout (`irc_*`, `forge_provider`) that never surfaces as a toast.

- [x] **Rate limit + per-key dedup on `emitQueryError`** — `error.ts` adds a 60-minute rolling window with an overall 200-event cap and a 5-event cap per `(command, error_title)` pair. Also skips `SilentError` defensively. Expected to cut `query:error` volume by ~100× without losing signal.
- [x] **Forward `command` + `actionName` to capture payload** — `customHooks.svelte.ts` now passes the RTK endpoint context into `emitQueryError`, so PostHog can group by command instead of string-parsing `API error: (cmd)` out of `error_message`. This surfaces clusters like per-project `stacks`/`list_reviews` that were previously fragmented across one-user buckets with project IDs embedded in the message.
- [x] **`erro_title` typo confirmed fixed in source** — grep shows only `error_title` in all four capture sites (`error.ts`, `customHooks.svelte.ts`, `toasts.ts`, `posthog.ts`). Deployed `1.360.2` still emits the typo because the fix rode into nightly but hasn't shipped in a stable release yet; next release will clear it. No code change needed.

## Round 2 — top untracked errors (24h snapshot, 2026-04-17)

- [x] **"Expected to be in edit mode" as Unhandled exception** — 90 events / 36 users in 24h. Already fixed in prior work but still firing; confirmed shipped — residual is old-build tail. No code change needed.
- [x] **Git hook output shown as error toast** — ~20 events / ~15 users. Hook failures were surfacing with generic "Error" title (from `Error.name`) or "Unhandled exception" (from unhandled promise rejections in `commitDropHandler.ts`). Fix: `hooksService.ts` now throws errors with `name = "Git hook failed"` for better PostHog grouping; `commitDropHandler.ts` wraps hook calls in try/catch with `showError("Git hook failed", err)` instead of letting them propagate as unhandled rejections.
- [x] **"Git push failed" toast shows raw command params** — 73 events / 31 users. `backendQuery.ts` was prepending `command: ...\nparams: {JSON}` to every backend error message, burying the actual error. Fix: removed the command/params prefix — the command name is already in the error `name` field (`API error: (push_stack)`), and the actual error message now surfaces cleanly in the toast.
- [x] **401 Unauthorized under generic "Error" title** — 8 events / 7 users. Some servers/proxies return a non-401 status code but with `{"error":"401 Unauthorized"}` in the body, bypassing the status-code check. Fix: `httpClient.ts` `parseResponseJSON` now also checks the response body for "401 Unauthorized" in the `>= 400` branch and shows the friendly login-expired message.
- [x] **"Failed to amend commit: noEffectiveChanges" as error** — 4–7 events / 1–2 users. Fix: `stackEndpoints.ts` `commitAmend.transformResponse` now detects when all rejections are `noEffectiveChanges`, shows an info toast ("No changes to amend"), and throws `SilentError` to suppress the error toast while still signaling mutation failure.

## Not addressed (deferred / out of scope)

- Linux auto-updater "invalid updater binary format" (178 events / 103 users) — Tauri/distro issue, needs separate effort
- Various single-user repeat errors (set_project_active, Windows `R:/` path, etc.) — environment-specific
- `Expected to be in edit mode` (2,811 events / 353 users) — already resolved prior to this session
- **IRC command-not-found spam** (`irc_get_file_message_reactions`, `irc_get_all_commit_reactions` — ~615k events / 1.8k users) — feature-flag gating on the call sites; deliberately out of scope for this round. Rate-limit changes above will already dedupe this down to ~5 events/user/hour per command.
- **`forge_provider` not found / ACL-blocked** (~67k events / ~5k users) — same class as IRC; new rate limit handles it.
