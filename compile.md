# Compile GitButler

Run from `/home/robert/dev/gitbutler`.

Build variables are scoped to the build: the `/etc/nixos#gitbutler` devShell sets `CHANNEL=nightly`, `VERSION=nightly`, `OS=linux`, and `CARGO_INCREMENTAL=1`, and every recipe below exports what it needs explicitly. Never set `CHANNEL`/`VERSION` globally in the NixOS session and never list them in `WSLENV`: forwarding `VERSION=nightly` into Windows processes broke optima-api deploys.

## NixOS full local build

Do not use `pnpm tauri-for-release`: its `tauri-before-build-command.sh` runs desktop, then `gitbutler-git`, then `but` sequentially and compiles shared crates twice. Leave that upstream script alone. Match the Windows recipe below: overlap the desktop frontend with one `cargo` invocation for `gitbutler-git` / `but`, then inject and build `gitbutler-tauri`. `CARGO_INCREMENTAL=1` keeps later rebuilds from doing a full relink.

```bash
nix develop /etc/nixos#gitbutler -c bash -lc '
set -euo pipefail
export CHANNEL=nightly
export VERSION=nightly
export OS=linux
export CARGO_INCREMENTAL=1
export TAURI_CONFIG="$(tr -d "\n\t" < crates/gitbutler-tauri/tauri.conf.nightly-local.json)"
cargo build --release -p gitbutler-git -p but &
rust=$!
pnpm install --frozen-lockfile
pnpm build:desktop -- --mode nightly
wait "$rust"
bash ./crates/gitbutler-tauri/inject-git-binaries.sh
cargo build --release -p gitbutler-tauri --features "builtin-but packaged-but-distribution"
./target/release/but --version
'
```

Outputs: `target/release/gitbutler-tauri`, `target/release/but`.

### Sync binaries, agent skill, and agent steering (required)

After the build, apply the Home Manager activation from `/etc/nixos/configuration.nix`:

```bash
sudo nixos-rebuild switch --flake /etc/nixos#nixos
```

This step is required on NixOS — do **not** run `but skill install` or `but agent setup` by hand. Skills and workflow steering come from Home Manager.

What it does:

- `~/.local/bin/but` is a symlink to `target/release/but` (via `mkOutOfStoreSymlink` in Home Manager).
- `~/.local/bin/gitbutler-tauri` is a wrapper that sets `LD_LIBRARY_PATH` / GIO / GStreamer and execs `target/release/gitbutler-tauri`. The wrapper plus `system.extraDependencies` keep webkitgtk in the Nix store so weekly GC cannot break the GUI.
- Activation `gitbutlerAgentSkills` runs `but skill install --global --path …` for Claude, Codex, Cursor, and Kimi (not GitHub Copilot).
- Skill content comes from the **embedded skill inside the freshly built `but` binary** (`crates/but/skill/` at compile time), not from loose repo files.
- Activation computes the SHA-256 of `~/.local/bin/but` at runtime and compares it to `.but_skill_stamp` in each skill directory. When `but` is rebuilt, the hash changes and the next `nixos-rebuild switch` reinstalls the skill automatically. **No manual version bump is needed** (unlike `graphifyVersion` for the graphify skill).
- Activation also refreshes agent steering from `gitbutlerAgentPolicy` in `/etc/nixos/configuration.nix` into `~/.cursor/rules/gitbutler.mdc` and the Nix-managed blocks in Claude / Codex / Kimi instruction files. That policy keeps the hard local-only rules (no push / no PR / no `but land`, own branch, HARD BAN on other agents' WIP) and adds the chosen workflow preferences: amend small follow-ups into the matching commit, suggest splitting mixed commits, auto-update from the target with `but pull` (with `--check` when other agents' branches are applied), and commit checkpoints after each turn.

Verify after switch:

```bash
sha256sum /home/robert/dev/gitbutler/target/release/but
cat ~/.cursor/skills/gitbutler/.but_skill_stamp   # must match the hash above
but --version
but skill check
rg -n 'Amend local fixes|Update from the target branch|Commit checkpoints' ~/.cursor/rules/gitbutler.mdc
```

Expected: all four skill paths exist, `.but_skill_stamp` matches the current `but` hash, `but skill check` reports `[nightly]` for Claude/Codex/Cursor (Kimi is installed on disk; `skill check` may not list it), and `gitbutler.mdc` contains the amend / auto-update / checkpoint sections.

**Note:** `but skill check` compares only the version string (`nightly`), not file content. After a rebuild, always run `nixos-rebuild switch` — do not rely on `but skill check` alone to detect stale skill files.

## Windows full local build

Build in the native NTFS clone of this fork at `C:\webarm\gitbutler` (branch `gitea-native-integration`). Do not build from `\\wsl.localhost\...` — the WSL file bridge is slow and fragile. Do not wrap the Windows build in `nix develop`.

### Sync from NixOS before compiling (required)

Windows must not build from a stale `origin` tip. A local NixOS GitButler workspace often holds commits that are **not** on the remote yet (applied stacks such as `gitea-native-integration`, docs/compile fixes, Windows PE stack, etc.). All version-control steps use **`but` only** (never `git fetch` / `git pull` / `git reset`). Building without a push leaves Windows on an outdated tree.

**Required sequence before every Windows compile that should match current NixOS work:**

1. On **NixOS**, publish everything that belongs in the Windows build: updates from the target (`but pull` when appropriate) **and** all local applied fixes that should ship in this nightly (typically including `gitea-native-integration` and any other applied branches whose commits must land on that branch tip).
2. Push those commits with `but push <branch>` to the fork remote (`origin` = `robertmonka/gitbutler`) so `origin/gitea-native-integration` (or the branch Windows builds) matches the NixOS intent.
3. On **Windows**, sync that branch with `but` **before** `cargo` / `pnpm` — never compile first and sync later. Prefer `but branch update gitea-native-integration -s pick-remote` so the local tip matches the pushed remote tip; use `but undo` immediately if the update leaves conflicted commits. Do not use plain `but branch update` without `-s pick-remote` when the goal is “same tip as origin”.

Do **not** treat “Windows is on `gitea-native-integration`” as “same as NixOS” unless step 2 already ran. Unpushed NixOS commits are invisible to Windows.

Run this in PowerShell 7. Call `pnpm.cmd` (not the `pnpm` PowerShell shim): the `.ps1` wrapper drops `--` and turbo then sees `--mode` as its own flag. Overlap the desktop frontend with `gitbutler-git` / `but` so those minutes are not added on top of Rust. `CARGO_INCREMENTAL=1` keeps later Windows rebuilds from doing a full MSVC relink of the 50–70 MB binaries.

Once, as Administrator, exclude the tree from Microsoft Defender realtime scanning (otherwise even a no-op release relink stays many minutes):

```powershell
Add-MpPreference -ExclusionPath C:\webarm\gitbutler
```

Only after the NixOS → remote push above, sync the Windows clone with `but`, then build:

```bash
/mnt/c/Program\ Files/PowerShell/7/pwsh.exe -NoProfile -Command '
$ErrorActionPreference = "Stop"
$p = "C:\webarm\gitbutler"
$install = "$env:LOCALAPPDATA\GitButler\bin"
$but = "$install\but.exe"
$gitbash = "C:\Program Files\Git\bin\bash.exe"
Set-Location $p
& $but branch update gitea-native-integration -s pick-remote
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$env:CHANNEL = "nightly"
$env:VERSION = "nightly"
$env:OS = "windows"
$env:CARGO_INCREMENTAL = "1"
$env:TAURI_CONFIG = Get-Content "$p\crates\gitbutler-tauri\tauri.conf.nightly-local.json" -Raw | ConvertFrom-Json | ConvertTo-Json -Compress -Depth 100
$rust = Start-Process -FilePath cargo -ArgumentList @("build","--release","-p","gitbutler-git","-p","but") -WorkingDirectory $p -NoNewWindow -PassThru
pnpm.cmd install --frozen-lockfile
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
pnpm.cmd build:desktop -- --mode nightly
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
if (-not $rust.HasExited) { Wait-Process -Id $rust.Id }
$rust.Refresh()
if ($rust.ExitCode -ne 0) { exit $rust.ExitCode }
& $gitbash ./crates/gitbutler-tauri/inject-git-binaries.sh
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
cargo build --release -p gitbutler-tauri --features windows
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Get-Process -Name but,gitbutler-tauri -ErrorAction SilentlyContinue | Stop-Process -Force
New-Item -ItemType Directory -Force $install | Out-Null
Copy-Item -Force "$p\target\release\but.exe" "$install\but.exe"
Copy-Item -Force "$p\target\release\gitbutler-tauri.exe" "$install\gitbutler-tauri.exe"
& "$install\but.exe" --version
exit $LASTEXITCODE
'
```

Outputs: `C:\Users\robert\AppData\Local\GitButler\bin\gitbutler-tauri.exe`, `C:\Users\robert\AppData\Local\GitButler\bin\but.exe`. Intermediate artifacts: `C:\webarm\gitbutler\target\release\but.exe`, `C:\webarm\gitbutler\target\release\gitbutler-tauri.exe`.

### Sync agent skill (required)

After copying the binaries, install the GitButler agent skill for Claude, Codex, Cursor, and `~/.agents` from the freshly built `but.exe`:

```bash
/mnt/c/Program\ Files/PowerShell/7/pwsh.exe -NoProfile -Command '
$ErrorActionPreference = "Stop"
$but = "$env:LOCALAPPDATA\GitButler\bin\but.exe"
$stamp = (Get-FileHash $but -Algorithm SHA256).Hash.ToLower()
foreach ($agent in @("claude","codex","cursor","agents")) {
  $dir = "$env:USERPROFILE\.$agent\skills\gitbutler"
  & $but skill install --global --path $dir
  Set-Content -NoNewline -Path "$dir\.but_skill_stamp" -Value $stamp
}
& $but skill check
'
```

What it does:

- Skill content comes from the **embedded skill inside the freshly built `but.exe`** (`crates/but/skill/` at compile time), not from loose repo files.
- `.but_skill_stamp` stores the SHA-256 of `but.exe` so you can skip reinstall when the binary has not changed.

Verify:

```powershell
$but = "$env:LOCALAPPDATA\GitButler\bin\but.exe"
(Get-FileHash $but -Algorithm SHA256).Hash.ToLower()
Get-Content "$env:USERPROFILE\.cursor\skills\gitbutler\.but_skill_stamp"  # must match
& $but --version
& $but skill check
```

Expected: all four skill paths report `[nightly]`, and `.but_skill_stamp` matches the current `but.exe` hash.

**Note:** `but skill check` compares only the version string (`nightly`), not file content. After a rebuild, always rerun the skill install step — do not rely on `but skill check` alone to detect stale skill files.
