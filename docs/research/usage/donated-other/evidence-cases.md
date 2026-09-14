# Donated Claude/Pi evidence cases

These are manually selected, source-linked sequences from retained tool calls, not independent users or controlled trials. Commands below are redacted argv reconstructions: messages are replaced by <message>; shell operators are simplified to semicolons for display. Consult the original tool payload at each pointer for short-circuiting, redirections, and wrappers. Results apply to the whole payload. Models are metadata values.

## C01 — Whole hunk IDs, not suffixes

ordinary website work; no benchmark/product-development path tag. The diff prints ur:a and us:e1/us:ec/us:f. A real attempted commit passes only suffixes a/e1/ec/f and fails; copying the complete tokens succeeds immediately. This is direct support for exact entity-ID copying, not a need for another status round.

- `claude-fable-5` · 2026-08-11T08:14:31.201Z · [call L46](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:46) `/message/content/0` · key `f64a37fef68a`

```sh
but diff
```
[Paired result L51](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:51) · tool error: `False`

```text
───────────────────────────────────╮
ur:a src/lib/components/Hero.svelte│
───────────────────────────────────╯
   25 25│       <p>
   26 26│         <span class="accent-text">$</span>
   27 27│         <ShuffleText
   28   │-          text=" co-hosted by GitButler * GitHub • JJ Con by Google • social by GitLab"
      28│+          text=" co-hosted by OpenAI * GitButler • JJ Con by Google • social by GitLab"
   29 29│           duration={2000}
   30 30│           delay={500}
   31 31│         />
───────────────────────────────╮
us:e1 src/routes/
```

- `claude-fable-5` · 2026-08-11T08:14:38.370Z · [call L55](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:55) `/message/content/0` · key `4cf777d4f3c6`

```sh
but commit -b update-cohosts -m '<message>' a e1 ec f
```
[Paired result L56](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:56) · tool error: `True`

```text
Exit code 1
Error: Could not find uncommitted change: 'a'

Hint: Run `but status` for applicable targets.
```

- `claude-fable-5` · 2026-08-11T08:14:44.280Z · [call L58](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:58) `/message/content/0` · key `520b2592144c`

```sh
but commit -b update-cohosts -m '<message>' ur:a us:e1 us:ec us:f
```
[Paired result L59](/Users/kiril/tmp/traces/claude/2026-08-11T08-12-35_ca559e9b546a0eb9b8a4a8fc5ecba0189715488775c8a5d2369b74b14afbeb46.jsonl:59) · tool error: `False`

```text
Created commit wuw on new branch 'update-cohosts'
```

## C02 — Space-separated sources, not one comma token

GitButler development task, not benchmark-tagged. The native error quotes the entire comma list as one unknown uncommitted change. Replacing commas with spaces succeeds without a new read. No stale-ID diagnosis is needed.

- `claude-fable-5` · 2026-07-28T09:30:11.862Z · [call L372](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:372) `/message/content/0` · key `73c2d2b010cd`

```sh
but diff
```
[Paired result L373](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:373) · tool error: `False`

```text
ou:a crates/but/src/utils/rejection.rs│
ou:b crates/but/src/utils/rejection.rs│
ou:c crates/but/src/utils/rejection.rs│
rm:d crates/but/skill/SKILL.md│
un:8 crates/but/src/command/legacy/squash.rs│
un:9 crates/but/src/command/legacy/squash.rs│
wn:3 crates/but/src/command/legacy/commit.rs│
wn:a crates/but/src/command/legacy/commit.rs│
```

- `claude-fable-5` · 2026-07-28T09:30:17.782Z · [call L374](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:374) `/message/content/0` · key `310391bebed3`

```sh
but amend ou,rm,un,wn --target 28cf96e
```
[Paired result L375](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:375) · tool error: `True`

```text
Exit code 1
Error: Could not find uncommitted change: 'ou,rm,un,wn'

Hint: Run `but status` for applicable targets.
```

- `claude-fable-5` · 2026-07-28T09:30:20.832Z · [call L376](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:376) `/message/content/0` · key `c33bb460038c`

```sh
but amend ou rm un wn --target 28cf96e
```
[Paired result L380](/Users/kiril/tmp/traces/claude/2026-07-28T09-02-01_17d6b936d7ba5a996050d7660de79e0fa52d548a53047958c48d98f60fe9c203.jsonl:380) · tool error: `False`

```text
Amended 28cf96e to create d0184ed
```

## C03 — Amend target flag survives as a recurring stumbling block

GitButler development task, not benchmark-tagged. The model uses amend <commit> <files>, receives the missing --target diagnostic, and succeeds on the next call with --target. Other matched native failures use the same pattern; historical syntax churn means this is migration/recipe evidence, not a universal model deficiency.

- `claude-fable-5` · 2026-07-29T18:11:34.401Z · [call L146](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:146) `/message/content/0` · key `6a8584a0acfa`

```sh
but diff
```
[Paired result L147](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:147) · tool error: `False`

```text
mkv:a apps/desktop/src/components/projectSettings/ForgeForm.svelte│
vt:8 crates/but-forge/src/review.rs│
vt:c crates/but-forge/src/review.rs│
```

- `claude-fable-5` · 2026-07-29T18:11:37.725Z · [call L148](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:148) `/message/content/0` · key `09b43e9e89d0`

```sh
but amend qtw mkv vt
```
[Paired result L149](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:149) · tool error: `True`

```text
Exit code 2
error: the following required arguments were not provided:
  --target <COMMIT_OR_BRANCH>

Usage: but amend --target <COMMIT_OR_BRANCH> <SOURCES>...

For more information, try '--help'.
```

- `claude-fable-5` · 2026-07-29T18:11:42.023Z · [call L150](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:150) `/message/content/0` · key `e450c0f092be`

```sh
but amend --target qtw mkv vt
```
[Paired result L151](/Users/kiril/tmp/traces/claude/2026-07-29T17-50-57_ae088ceddcf8ee9fa184d6abfb5740c24f6c982eef4eab597a6ee6d504bcc692.jsonl:151) · tool error: `False`

```text
Amended qtw
```

## C04 — Current syntax can fail against an older binary

GitButler development task; historical installed CLI mismatch. The preceding diff explicitly says the skill was updated to 0.21.2. Current-style commit -b is rejected twice with old positional-BRANCH usage. After help, the model uses commit <branch> -c; that creates a branch but hits a separate stack-not-found error. Do not classify current -b as bad general advice or count all these failures as the same cause. Nearby skill version is observed; exact binary version in this event remains unproven.

- `claude-fable-5` · 2026-07-30T18:06:45.207Z · [call L117](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:117) `/message/content/0` · key `3e5c0526c8c5`

```sh
but diff
```
[Paired result L121](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:121) · tool error: `False`

```text
✓ The GitButler skill was out of date and was updated to 0.21.2. Reload/use the GitButler skill to pick up the changes.

────────────────────────────────────╮
vx:7 crates/but-installer/src/lib.rs│
────────────────────────────────────╯
   71 71│ /// changes on installation failure.
   72 72│ pub fn run_installation() -> Result<()> {
   73 73│     let config = InstallerConfig::new()?;
   74   │-    run_installation_impl(config, ui::is_connected_to_terminal())
      74│+    let interactive = ui::is_connected_to_terminal();
      75│+    run_instal
```

- `claude-fable-5` · 2026-07-30T18:06:51.380Z · [call L122](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:122) `/message/content/0` · key `932d2df41326`

```sh
but commit -b installer-agent-skill-prompt -m '<message>' vx:7 vx:1 xu:6 xu:9
```
[Paired result L123](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:123) · tool error: `True`

```text
Exit code 2
error: unexpected argument '-b' found

  tip: to pass '-b' as a value, use '-- -b'

Usage: but commit [OPTIONS] [BRANCH] [COMMAND]

For more information, try '--help'.
```

- `claude-fable-5` · 2026-07-30T18:07:02.628Z · [call L133](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:133) `/message/content/0` · key `2e090f85eae6`

```sh
but commit -b installer-agent-skill-prompt -m '<message>' vx:7 vx:1 xu:6 xu:9
```
[Paired result L134](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:134) · tool error: `True`

```text
Exit code 2
error: unexpected argument '-b' found

  tip: to pass '-b' as a value, use '-- -b'

Usage: but commit [OPTIONS] [BRANCH] [COMMAND]

For more information, try '--help'.
```

- `claude-fable-5` · 2026-07-30T18:07:07.158Z · [call L137](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:137) `/message/content/0` · key `a7d913908ebc`

```sh
but commit --help
```
[Paired result L138](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:138) · tool error: `False`

```text
Commit changes to a stack.

The but commit command allows you to create a new commit on a specified branch (stack) with the
current uncommitted changes.

If there is only one branch applied, it will commit to that branch by default.

If there are multiple branches applied, you must specify which branch to commit to, or if in
interactive mode, you will be prompted to select one.

By default, all uncommitted changes and all changes already staged to that branch will be included
in the commit. If you only want to commit the changes that are alread
```

- `claude-fable-5` · 2026-07-30T18:07:17.583Z · [call L140](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:140) `/message/content/0` · key `e784d57785e3`

```sh
but commit installer-agent-skill-prompt -c -m '<message>' -p vx:7,vx:1,xu:6,xu:9
```
[Paired result L141](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:141) · tool error: `True`

```text
Exit code 1
Created new independent branch 'installer-agent-skill-prompt'
Error: Stack b3426ac7-952c-4000-868d-5f74f9e1bd00 not found in workspace
```

## C05 — Retired rub diagnosis gives a short successful migration

GitButler development task, not benchmark-tagged. Actual rub ry pvl receives the explicit retirement note. The next payload tries amend ry -t pvl with a help fallback; output is only Amended pvl. The fallback help segment was submitted but not executed, illustrating why fragment counts are not execution counts.

- `claude-fable-5` · 2026-08-06T14:37:01.010Z · [call L250](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:250) `/message/content/0` · key `2f83cc61d93b`

```sh
but status -f
```
[Paired result L251](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:251) · tool error: `False`

```text
╭┄ zz [uncommitted]
┊   ry M crates/but-api/src/legacy/stack.rs
┊
┊╭┄ sb [sbm-branch-create-in-api]
┊●   pvl Handle single-branch mode branch creation in the API
┊│     pvl:r M crates/but-api/src/legacy/stack.rs
┊│     pvl:l M crates/but/src/command/legacy/branch/mod.rs
┊│     pvl:p M crates/but/tests/but/command/branch/new.rs
├╯
┊
┴ 58a82df115 (common base) 2026-08-06 Merge pull request #15218 from gitbutlerapp/push-all-exit-code

Hint: commits are listed newest first. The first token on each line is the ID to use in commands.
Hint: run `but d
```

- `claude-fable-5` · 2026-08-06T14:37:04.483Z · [call L252](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:252) `/message/content/0` · key `e80ad25ff12e`

```sh
but rub ry pvl
```
[Paired result L253](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:253) · tool error: `True`

```text
Exit code 2

note: `but rub` was retired. Squashing sources into a target is now:

    but squash <source>... -t <target>

Moving sources is `but move <source>...` with a placement flag (`--below`/`--above` a commit, `--branch <branch>`).
See `but squash --help` and `but move --help` for details.
Run `but skill install` (or `but skill check --update`) to load current command docs.
error: unrecognized subcommand 'rub'

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.
```

- `claude-fable-5` · 2026-08-06T14:37:07.313Z · [call L254](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:254) `/message/content/0` · key `765de03ff492`

```sh
but amend ry -t pvl ; but amend --help
```
[Paired result L255](/Users/kiril/tmp/traces/claude/2026-08-06T13-48-09_92cf0a739965a01ca41a0176fac859b9fd64bb67c4361a05ea0959648f3476a6.jsonl:255) · tool error: `False`

```text
Amended pvl
```

## C06 — Ambiguous hunk token resolved with exact disambiguators

ordinary publishing-tool work, no benchmark tag. Native error says yr:7 is ambiguous; the next call passes yr:7#0-2 and yr:7#1-2 and succeeds. These are observed historical CLI disambiguators, not line ranges invented by the model. Preserve exact printed IDs; validate current resolver semantics before generalizing the suffix shape.

- `claude-fable-5` · 2026-08-14T10:27:03.993Z · [call L237](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:237) `/message/content/0` · key `3a7913bcf19b`

```sh
but diff
```
[Paired result L238](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:238) · tool error: `False`

```text
───────────────────────╮
xp:8 cli/agent-guide.md│
───────────────────────╯
   18 18│     gpub publish data.csv        # CSV/TSV, rendered to a sortable table
   19 19│     gpub publish config.json     # JSON, rendered to a browsable tree
   20 20│     gpub publish page.html       # a single HTML file
      21│+    gpub publish photo.png       # an image; the printed URL is the image itself
   21 22│     gpub publish ./site/         # a folder of static assets
   22 23│     gpub publish build.tar.gz    # .tar.gz, .tgz, .tar or .zip of a folder
 
```

- `claude-fable-5` · 2026-08-14T10:27:11.708Z · [call L244](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:244) `/message/content/0` · key `e9ff866c4dbb`

```sh
but commit -b image-direct-url -m '<message>' xp:8 yr:d yr:e yr:7 yr:8 yr:f pm:2 pm:7 pm:6 ly:3 so:c nr:8 sp:e uy:8d uy:83 uy:f
```
[Paired result L245](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:245) · tool error: `True`

```text
Exit code 1
Error: Ambiguous uncommitted change 'yr:7', matches multiple items

Hint: Use a longer ID to disambiguate
```

- `claude-fable-5` · 2026-08-14T10:27:18.035Z · [call L247](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:247) `/message/content/0` · key `13a7674c9775`

```sh
but commit -b image-direct-url -m '<message>' xp:8 yr:d yr:e 'yr:7#0-2' 'yr:7#1-2' yr:8 yr:f pm:2 pm:7 pm:6 ly:3 so:c nr:8 sp:e uy:8d uy:83 uy:f
```
[Paired result L248](/Users/kiril/tmp/traces/claude/2026-08-17T09-02-27_0f0076bc91cd7e9cd28cdf1e3fd4ec5671fce6ea70151556ce4d6a4d47e48a0c.jsonl:248) · tool error: `False`

```text
Created commit wkw on new branch 'image-direct-url'
```

## C07 — Entity IDs can change between chained commits

Pi / GitButler development task. The first commit in a two-commit shell chain succeeds. The second rejects mo and says mo is a branch. A fresh diff leads to moq and succeeds. The aggregate tool result is failure despite a successful first commit: recovery must account for partial progress and refresh only the failed selection.

- `gpt-5.6-sol` · 2026-07-28T17:38:26.301Z · [call L108](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:108) `/message/content/1` · key `6b8c680b160c`

```sh
./target/release/but diff
```
[Paired result L109](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:109) · tool error: `False`

```text
─────────────────────────────────╮
rx:4 crates/but/src/args/amend.rs│
─────────────────────────────────╯
   21 21│     pub target: CliIdArg,
   22 22│ 
   23 23│     /// One or more uncommitted files or hunks to amend.
   24   │-    #[clap(required = true)]
      24│+    ///
      25│+    /// If omitted, all changes in the uncommitted area (`zz`) are amended.
   25 26│     pub sources: Vec<CliIdArg>,
   26 27│ 
   27 28│     #[clap(flatten)]
──────────────────────────────────╮
xu:9 crates/but/src/args/squash.rs│
────────────────────────────────
```

- `gpt-5.6-sol` · 2026-07-28T17:38:36.912Z · [call L110](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:110) `/message/content/1` · key `88d4c52f18d8`

```sh
./target/release/but commit --branch dp-squash-amend-default-source -m '<message>' xu un tk ; ./target/release/but commit --branch dp-squash-amend-default-source -m '<message>' rx mm pp mo
```
[Paired result L111](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:111) · tool error: `True`

```text
Created commit npq on new branch 'dp-squash-amend-default-source'
Error: Could not find uncommitted change: 'mo'

Hint: 'mo' is a branch. To commit onto it, run `but commit -b mo -m "message" [<change>...]`


Command exited with code 1
```

- `gpt-5.6-sol` · 2026-07-28T17:38:43.363Z · [call L112](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:112) `/message/content/1` · key `89e5cc4e7a5f`

```sh
./target/release/but diff
```
[Paired result L113](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:113) · tool error: `False`

```text
─────────────────────────────────╮
rx:4 crates/but/src/args/amend.rs│
─────────────────────────────────╯
   21 21│     pub target: CliIdArg,
   22 22│ 
   23 23│     /// One or more uncommitted files or hunks to amend.
   24   │-    #[clap(required = true)]
      24│+    ///
      25│+    /// If omitted, all changes in the uncommitted area (`zz`) are amended.
   25 26│     pub sources: Vec<CliIdArg>,
   26 27│ 
   27 28│     #[clap(flatten)]
───────────────────────────────────────╮
mm:b crates/but/src/args/tests/amend.rs│
──────────────────────
```

- `gpt-5.6-sol` · 2026-07-28T17:38:47.711Z · [call L114](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:114) `/message/content/1` · key `72c0fa2687bb`

```sh
./target/release/but commit --branch dp-squash-amend-default-source -m '<message>' rx mm pp moq
```
[Paired result L115](/Users/kiril/tmp/traces/pi/2026-07-28T17-26-54_90bf04b2d7216f7d2252b1e03a3efe497649ae63e383faab8b8d07c1cf0604d7.jsonl:115) · tool error: `False`

```text
Created commit ppy on branch 'dp-squash-amend-default-source'

```

## C08 — Diff is not Git diff with arbitrary pathspecs

Pi / GitButler development work. Distinct native errors reject --stat, a Git-style branch -- path list, and two direct diff targets vv vs. This supports making read-command cardinality and selector semantics easy to find. These dated binaries are not proof of current flag support.

- `gpt-5.5` · 2026-06-16T12:14:05.817Z · [call L35](/Users/kiril/tmp/traces/pi/2026-06-16T12-11-01_82590717a45032a913d90d64da69360de05d743b50a8cc8d7626cf730ef65f4f.jsonl:35) `/message/content/1` · key `70f5d526562d`

```sh
'/Users/<user>/code/gitbutler/gitbutler-git/target/release/but' diff dp-commit2-pick-changes --stat
```
[Paired result L36](/Users/kiril/tmp/traces/pi/2026-06-16T12-11-01_82590717a45032a913d90d64da69360de05d743b50a8cc8d7626cf730ef65f4f.jsonl:36) · tool error: `True`

```text
error: unexpected argument '--stat' found

  tip: a similar argument exists: '--status-after'

Usage: but diff <TARGET>

For more information, try '--help'.


Command exited with code 2
```

- `gpt-5.5` · 2026-06-16T12:34:49.141Z · [call L46](/Users/kiril/tmp/traces/pi/2026-06-16T12-29-44_d3e6e6b8af2cf56fd63c6713389ad1ca76a0c3fff9ca7b216ddb1d06985479b3.jsonl:46) `/message/content/1` · key `01fec6e8a80b`

```sh
'/Users/<user>/code/gitbutler/gitbutler-git/target/release/but' diff dp-commit2-pick-changes -- crates/but/src/command/legacy/status/tui/mod.rs crates/but/src/command/legacy/status/tui/tests/mod.rs crates/but/src/command/legacy/status/tui/key_bind/mod.rs crates/but/src/command/legacy/status/tui/help.rs crates/but/src/command/legacy/status/tui/mode.rs
```
[Paired result L47](/Users/kiril/tmp/traces/pi/2026-06-16T12-29-44_d3e6e6b8af2cf56fd63c6713389ad1ca76a0c3fff9ca7b216ddb1d06985479b3.jsonl:47) · tool error: `True`

```text
error: unexpected argument 'crates/but/src/command/legacy/status/tui/mod.rs' found

Usage: but diff [OPTIONS] [TARGET]

For more information, try '--help'.


Command exited with code 2
```

- `gpt-5.6-sol` · 2026-08-01T21:15:19.983Z · [call L214](/Users/kiril/tmp/traces/pi/2026-08-01T20-41-14_f6353f50c8d0467d90a7ff724104d18df86339fd8351d36be7be1e0f0bdcdf72.jsonl:214) `/message/content/5` · key `13b3213fe5b2`

```sh
./target/release/but diff vv vs
```
[Paired result L219](/Users/kiril/tmp/traces/pi/2026-08-01T20-41-14_f6353f50c8d0467d90a7ff724104d18df86339fd8351d36be7be1e0f0bdcdf72.jsonl:219) · tool error: `True`

```text
error: unexpected argument 'vs' found

Usage: but diff [OPTIONS] [TARGET]

For more information, try '--help'.


Command exited with code 2
```

## C09 — Missing executable is a harness environment issue

Pi / GitButler development work. Bare but fails with shell code 127. The model discovers the build output (intervening lookup is excluded from invocation counts) and runs ./target/debug/but successfully. Forty-two Pi But-containing payloads pair the command-not-found marker with reported error; this must not become a skill-comprehension failure rate.

- `gpt-5.5` · 2026-06-20T22:17:46.202Z · [call L53](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:53) `/message/content/1` · key `01261b64aad3`

```sh
but diff
```
[Paired result L54](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:54) · tool error: `True`

```text
/bin/bash: line 1: but: command not found


Command exited with code 127
```

- `gpt-5.5` · 2026-06-20T22:17:52.470Z · [call L57](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:57) `/message/content/1` · key `ab6f55b1a5fa`

```sh
./target/debug/but diff
```
[Paired result L58](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:58) · tool error: `False`

```text
   117 117│ ┊●   23e1bf8 create e.txt and f.txt
───────────────────────────────────────╮
vf crates/but/tests/but/command/move.rs│
───────────────────────────────────────╯
   155 155│         .success()
   156 156│         .stderr_eq(snapbox::str![])
   157 157│         .stdout_eq(snapbox::str![[r#"
   158    │-╭┄zz [unassigned changes] (no changes)
       158│+╭┄zz [uncommitted changes] (no changes)
   159 159│ ┊
   160 160│ ┊╭┄g0 [A]
   161 161│ ┊●   0e08d58 create a.txt and b.txt
───────────────────────────────────────╮
wf crates/but/tests/bu
```

- `gpt-5.5` · 2026-06-20T22:17:58.389Z · [call L59](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:59) `/message/content/1` · key `93697b443dc0`

```sh
./target/debug/but status -fv
```
[Paired result L60](/Users/kiril/tmp/traces/pi/2026-06-20T22-12-14_592a9864cf2c44380f0c2d1b1c012f2e1c0781815064bdde31e8229d9796d5ca.jsonl:60) · tool error: `False`

```text
Initiated a background sync...
╭┄zz [uncommitted changes]
┊     ys M crates/but/src/command/legacy/status/mod.rs
┊    pvu M crates/but/src/command/legacy/status/output.rs
┊    quw M crates/but/src/command/legacy/status/tui/mod.rs
┊     mt M crates/but/src/command/legacy/status/tui/tests/branch_picker_tests.rs
┊     xk M crates/but/src/command/legacy/status/tui/tests/branch_tests.rs
┊     nv M crates/but/src/command/legacy/status/tui/tests/command_tests.rs
┊     yy M crates/but/src/command/legacy/status/tui/tests/commit_tests.rs
┊    trk M crate
```

## C10 — Do not guess gh-style PR flags or stdin-file semantics

GitButler development tasks. --title/--body are rejected by this But CLI. A file-based PR command succeeds. In a different event, -F - means a literal file named dash and fails; an actual message file succeeds. Both successful outputs also show Pushing before Creating review, so a separate preceding push is not needed for this recipe.

- `claude-fable-5` · 2026-08-04T13:03:26.731Z · [call L146](/Users/kiril/tmp/traces/claude/2026-08-04T12-21-18_0e9978e48c82583648926d552ac4157f7e11a559b74cdc2c7de0015187b6e0ef.jsonl:146) `/message/content/0` · key `61dc5f3a3fec`

```sh
but pr new skill-notice-wording --title '<message>' --body '<message>'
```
[Paired result L147](/Users/kiril/tmp/traces/claude/2026-08-04T12-21-18_0e9978e48c82583648926d552ac4157f7e11a559b74cdc2c7de0015187b6e0ef.jsonl:147) · tool error: `True`

```text
Exit code 2
error: unexpected argument '--title' found

  tip: a similar argument exists: '--file'

Usage: but pr new --file <FILE> <BRANCH>

For more information, try '--help'.
```

- `claude-fable-5` · 2026-08-04T13:03:38.984Z · [call L150](/Users/kiril/tmp/traces/claude/2026-08-04T12-21-18_0e9978e48c82583648926d552ac4157f7e11a559b74cdc2c7de0015187b6e0ef.jsonl:150) `/message/content/0` · key `0d8c2918781c`

```sh
but pr new --file '/private/tmp/agent-scratch/<workspace>/e6d77bd8-543c-4ae6-ab3f-8359f0e8fd2b/scratchpad/pr-body.md' skill-notice-wording
```
[Paired result L151](/Users/kiril/tmp/traces/claude/2026-08-04T12-21-18_0e9978e48c82583648926d552ac4157f7e11a559b74cdc2c7de0015187b6e0ef.jsonl:151) · tool error: `False`

```text
Pushing skill-notice-wording...
  ✓ Pushed to origin
Creating review for skill-notice-wording → master...

✓ Created review #15167
  Title: De-coerce the agent skill install notice
  Branch: skill-notice-wording
  URL: https://github.com/gitbutlerapp/gitbutler/pull/15167
```

- `claude-fable-5` · 2026-07-30T20:02:21.772Z · [call L345](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:345) `/message/content/0` · key `c2245406e222`

```sh
but pr new in -F -
```
[Paired result L346](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:346) · tool error: `True`

```text
Exit code 1
Error: Failed to read forge review message from file: -

Caused by:
    No such file or directory (os error 2)
```

- `claude-fable-5` · 2026-07-30T20:02:35.602Z · [call L350](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:350) `/message/content/0` · key `0cc5a8629b10`

```sh
but pr new in -F '/private/tmp/agent-scratch/<workspace>/445f367a-549e-4ea7-9fb2-ec6ca7793ff0/scratchpad/pr_message.md'
```
[Paired result L351](/Users/kiril/tmp/traces/claude/2026-07-30T17-59-39_25bb2f2810a8debd851e5eddd86bd5ee48a9fd19bab79ec008765abc6fb02453.jsonl:351) · tool error: `False`

```text
Pushing installer-agent-skill-prompt...
  ✓ Pushed to origin
Creating review for installer-agent-skill-prompt → master...

✓ Created review #15100
  Title: Offer agent skill setup at the end of installation
  Branch: installer-agent-skill-prompt
  URL: https://github.com/gitbutlerapp/gitbutler/pull/15100
```

## C11 — Dependency recovery needs actual stack placement

ordinary dashboard repository, no benchmark/product cwd tag. An atomic dependency rejection names butlerdash-app. Creating a branch above that branch and retrying still rejects the change; inspecting the actual stack then moving above weekly-eng-brief follows. This is evidence that dependency errors/stack context matter, not permission to blindly follow an abbreviated historical branch hint. The intermediate CLI warns --anchor is deprecated; current-main-skill still mentions that historical shape, which the parent should check against current code.

- `claude-opus-5` · 2026-08-08T13:25:45.321Z · [call L111](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:111) `/message/content/0` · key `95add91000b4`

```sh
but commit -b obsidian-cli-vault-access -m '<message>' ws:3
```
[Paired result L112](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:112) · tool error: `True`

```text
Exit code 1
Error: Cannot commit: 1 change could not be applied:
  AGENTS.md
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butl
```

- `claude-opus-5` · 2026-08-08T13:25:54.121Z · [call L114](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:114) `/message/content/0` · key `14e2714edc09`

```sh
but branch new obsidian-cli-vault-access --anchor butlerdash-app ; but commit -b obsidian-cli-vault-access -m '<message>' ws:3
```
[Paired result L115](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:115) · tool error: `True`

```text
Exit code 1
⚠ `--anchor/-a` is deprecated and will be removed in a future release. Use `--above/-A` instead
Created branch 'obsidian-cli-vault-access' above branch 'butlerdash-app'
Error: Cannot commit: 1 change could not be applied:
  AGENTS.md
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 81–106 depends on butlerdash-app (vvq)
    lines 
```

- `claude-opus-5` · 2026-08-08T13:26:01.209Z · [call L117](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:117) `/message/content/0` · key `32a32e800837`

```sh
but status
```
[Paired result L118](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:118) · tool error: `False`

```text
Update available: 0.5.2144 → 0.5.2145 (upgrade with `but update install` or `but update suppress` to dismiss)

╭┄ zz [uncommitted]
┊   ws M AGENTS.md
┊
┊╭┄ vo [voice-guide]
┊●   psw Add a voice guide for posts bylined schacon
├╯
┊
┊╭┄ as [cash-runway-report]
┊●   zzo Add a cash runway report
├╯
┊
┊╭┄ we [weekly-eng-brief]
┊●   mox Add a weekly engineering brief, and the Linear collector behind it
┊│
┊├┄ tw [tweet-reply-candidates]
┊●   mzy Draft replies to tweets about Git, GitHub and GitButler
┊●   wpv Extract the claude CLI runner into src/li
```

- `claude-opus-5` · 2026-08-08T13:26:18.782Z · [call L120](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:120) `/message/content/0` · key `337d7d86d84f`

```sh
but move obsidian-cli-vault-access --above weekly-eng-brief
```
[Paired result L121](/Users/kiril/tmp/traces/claude/2026-08-08T13-21-40_3b1f3025c02b20fed84bd523fec83421e5af1cd57e93e402c2c6ded33d65c7a1.jsonl:121) · tool error: `False`

```text
Stacked branch 'obsidian-cli-vault-access' on top of branch 'weekly-eng-brief'
---exit:0
```

## C12 — Efficient two-command selected commit exists

ordinary website work, no benchmark/product cwd tag. The diff supplies ok:1. Committing the selected whole file ok creates the new branch and commit in one call, with a concise success output. No separate branch creation is required. This is a successful local sequence, not a full task-success claim or a claim that the task never later published its change.

- `claude-fable-5` · 2026-08-12T07:39:41.898Z · [call L121](/Users/kiril/tmp/traces/claude/2026-08-11T08-36-02_921b4809ec61e9f24c008c9f570df7065a6d301ee6f7b730ce992f79f07786f9.jsonl:121) `/message/content/0` · key `38f138c94e9f`

```sh
but diff
```
[Paired result L122](/Users/kiril/tmp/traces/claude/2026-08-11T08-36-02_921b4809ec61e9f24c008c9f570df7065a6d301ee6f7b730ce992f79f07786f9.jsonl:122) · tool error: `False`

```text
──────────────────────────────────╮
ok:1 src/lib/components/Faq.svelte│
──────────────────────────────────╯
    9  9│   const faqs: Faq[] = [
   10 10│     {
   11 11│       flag: "--jj-con",
   12   │-      answer: "All Jujutusu, all day. Bundle tickets available.",
      12│+      answer: "All Jujutsu, all day. Bundle tickets available.",
   13 13│       link: "https://github.com/jj-vcs/jj"
   14 14│     },
   15 15│     {
```

- `claude-fable-5` · 2026-08-12T07:39:50.656Z · [call L123](/Users/kiril/tmp/traces/claude/2026-08-11T08-36-02_921b4809ec61e9f24c008c9f570df7065a6d301ee6f7b730ce992f79f07786f9.jsonl:123) `/message/content/0` · key `9b897c085a98`

```sh
but commit -b fix-jujutsu-typo -m '<message>' ok
```
[Paired result L124](/Users/kiril/tmp/traces/claude/2026-08-11T08-36-02_921b4809ec61e9f24c008c9f570df7065a6d301ee6f7b730ce992f79f07786f9.jsonl:124) · tool error: `False`

```text
Created commit yrt on new branch 'fix-jujutsu-typo'
```

## C13 — Identical retry does not fix wrong workspace context

Pi / GitButler development task. Two consecutive actual tool calls submit the identical absolute-binary status --format json; both return exactly the same setup_required output. This is a concrete redundant failed check, not an inference from a long gap or repeated diff names.

- `gpt-5.5` · 2026-06-20T21:06:25.619Z · [call L39](/Users/kiril/tmp/traces/pi/2026-06-20T21-03-10_e31b626ba75fa2f22d0128686f793fb63916b37c491b7d28c46dbb208c9afc47.jsonl:39) `/message/content/1` · key `4cd038fbac72`

```sh
'/Users/<user>/code/gitbutler/gitbutler-git/target/release/but' status --format json
```
[Paired result L40](/Users/kiril/tmp/traces/pi/2026-06-20T21-03-10_e31b626ba75fa2f22d0128686f793fb63916b37c491b7d28c46dbb208c9afc47.jsonl:40) · tool error: `True`

```text
{
  "error": "setup_required",
  "message": "No GitButler project found at .",
  "hint": "run `but setup` to configure the project"
}
Error: Setup required: No GitButler project found at . - run `but setup` to configure the project


Command exited with code 1
```

- `gpt-5.5` · 2026-06-20T21:06:29.458Z · [call L41](/Users/kiril/tmp/traces/pi/2026-06-20T21-03-10_e31b626ba75fa2f22d0128686f793fb63916b37c491b7d28c46dbb208c9afc47.jsonl:41) `/message/content/1` · key `70ac5470186b`

```sh
'/Users/<user>/code/gitbutler/gitbutler-git/target/release/but' status --format json
```
[Paired result L42](/Users/kiril/tmp/traces/pi/2026-06-20T21-03-10_e31b626ba75fa2f22d0128686f793fb63916b37c491b7d28c46dbb208c9afc47.jsonl:42) · tool error: `True`

```text
{
  "error": "setup_required",
  "message": "No GitButler project found at .",
  "hint": "run `but setup` to configure the project"
}
Error: Setup required: No GitButler project found at . - run `but setup` to configure the project


Command exited with code 1
```

## C14 — Error words in another command or a diff are not But failures

development tool batches. A successful test summary contains 0 failed before but status; a clippy error precedes a successful but diff; a source diff quotes Error: Bad input as code. Automated keyword-error counts conflate all three with CLI mistakes.

- `claude-opus-5` · 2026-07-26T12:49:33.365Z · [call L1441](/Users/kiril/tmp/traces/claude/2026-07-25T21-32-02_063cc1746dbe2abd1f76626ba024d486a4827e9499ac5d9af7bb88b0e70d1119.jsonl:1441) `/message/content/0` · key `ccf1298749ca`

```sh
but status
```
[Paired result L1442](/Users/kiril/tmp/traces/claude/2026-07-25T21-32-02_063cc1746dbe2abd1f76626ba024d486a4827e9499ac5d9af7bb88b0e70d1119.jsonl:1442) · tool error: `False`

```text
fmt ok
test result: ok. 718 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 15.39s
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 529 passed; 0 failed; 5 ignored; 0 measured; 0 filtered out; finished in 21.88s
╭┄ zz [uncommitted]
┊   ys M crates/but/src/command/legacy/status/mod.rs
┊
┊╭┄ dr [drop-assignment-vocabulary] #15015: Stop documenting assignment of changes to a lane 
┊●   ruo docs(cli): stop documenting assignment of changes to a lane
├╯
```

- `claude-fable-5` · 2026-07-22T17:28:17.384Z · [call L741](/Users/kiril/tmp/traces/claude/2026-07-22T16-15-39_19304f9854f9b19bc2ef48828d11562ffe151facce4a9e00a1443d646b5fcef2.jsonl:741) `/message/content/0` · key `847302c86855`

```sh
but diff
```
[Paired result L742](/Users/kiril/tmp/traces/claude/2026-07-22T16-15-39_19304f9854f9b19bc2ef48828d11562ffe151facce4a9e00a1443d646b5fcef2.jsonl:742) · tool error: `False`

```text
warning: variables can be used directly in the `format!` string
  --> crates/but-api/examples/blist_bench.rs:61:5
   |
61 | /     println!(
62 | |         "NEW endpoint:  {:?} | with_review={} with_ci={}",
63 | |         new_api_time, with_reviews, with_ci
--
error: this creates an owned instance just for comparison
   --> crates/but-api/examples/blist_bench.rs:119:23
    |
tp:9 crates/but-api/examples/blist_bench.rs│
```

- `gpt-5.6-sol` · 2026-08-01T21:52:50.806Z · [call L69](/Users/kiril/tmp/traces/pi/2026-08-01T21-45-57_8d34dbc061820649ced1aa352f893aa1194683b10bdeeb4df448ed7063524a0c.jsonl:69) `/message/content/1` · key `5a863b300a93`

```sh
./target/release/but diff
```
[Paired result L70](/Users/kiril/tmp/traces/pi/2026-08-01T21-45-57_8d34dbc061820649ced1aa352f893aa1194683b10bdeeb4df448ed7063524a0c.jsonl:70) · tool error: `False`

```text
──────────────────────────────╮
rm:e crates/but/skill/SKILL.md│
──────────────────────────────╯
   76 76│ - Move commit to branch top: `but move <commit-id> -b <branch>`
   77 77│ - Stack branches: `but move <branch> --above <target-branch>` (**branch names or branch CLI IDs**)
   78 78│ - Tear off a branch: `but move <branch> --unstack`
   79   │-- Discard: `but discard <id> [<id>...]` — accepts branches, commits, committed files, uncommitted files/hunks, or `zz` for all uncommitted changes
      79│+- Discard: `but discard <id> [<id>...]` — a
```

