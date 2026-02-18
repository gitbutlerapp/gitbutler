### API usage

* Avoid using code from `gitbutler-`-prefixed crates, and prefer code from `but-` prefixed crates as long as it's not in the `legacy` module.

### Documentation

* Always add documentation comments for every struct, each struct field, and every function you create.

### Output

Usable output goes to `out: &mut OutputChannel`

- For humans, use `if let Some(out) = out.for_human() { writeln!(out, “{…}")?; }`
- For shell, use `if let Some(out) = out.for_shell() { writeln!(out, “{…}")?; }`
- For JSON, use `if let Some(out) = out.for_json() { out.write_value(json)?; }`
- If intentionally ignoring output-write errors, use:
  - Single-line statements: replace `let _ = ...;` with `....ok();`, but ensure the statement remains on a single line after formatting.
  - Multi-line statements: use `_ = ` instead of `let _ = ` and keep the rest unchanged.

### Stdin

* Do not read from `std::io::stdin()` directly in command/business logic and *human* input.
* For free-form terminal input, gate with `out.can_prompt()` *or* collect input via `out.prepare_for_terminal_input()`.
* Consider using `cli-prompts` for preset choices.
* For machine/piped input, take a `read: impl std::io::Read` parameter and parse from that reader so tests can inject input.
* Keep any `stdin().lock()` usage at the top-level CLI wiring only (for example in `lib.rs`), and pass readers downward.

### Context

- Do not re-discover Git repositories, instead take them as inputs to functions and methods. They can be retrieved from contexed via `ctx.repo.get()?`
  and passed as parameter.
- Prefer `repo` APIs (for example `repo.rev_parse_single(...)`) over shelling out to `git`; only shell out when there is no equivalent API.
- Avoid implicitly using the current time like `std::time::SystemTime::now()`, instead pass the current time as argument.

### Testing

* use `snapbox::str![]` to assert with `.stdout_eq(str![])` and `stderr_eq(str![])` respectively,
  and auto-update expectations with `SNAPSHOTS=overwrite cargo test -p but`.
* When color is involved, use with `.stdout_eq(snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"])`, and update it 
  with `SNAPSHOT=overwrite cargo test -p but`.
* In `crates/but/tests/`, prefer `env.but(...).assert().success()/failure().stdout_eq(str![...]).stderr_eq(str![...])` for CLI output checks.
* Avoid `env.but(...).output()` followed by direct `stdout`/`stderr` assertions (for example, `String::from_utf8_lossy(&output.stdout)` with `assert_*`).
* If only part of output matters, use `snapbox::str!` wildcards (`..`) to ignore unstable sections.
* Do not use `anyhow::ensure!` in tests; use panicking assertions (`assert!`, `assert_eq!`, `assert_ne!`) so failures are test panics.

### Linting

* use `cargo fmt --check --all` to check for formatting issues.
* use `cargo clippy --all-targets --fix --allow-dirty` to auto-fix clippy errors.

### CLI Skills

* After changing CLI commands or workflows, update the skill files in `crates/but/skill/` so AI agents stay current
* Users update their skills with `but skill install --detect` (auto-detects installation location)
