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

### Context

- Do not re-discover Git repositories, instead take them as inputs to functions and methods. They can be retrieved from contexed via `ctx.repo.get()?`
  and passed as parameter.
- Avoid implicitly using the current time like `std::time::SystemTime::now()`, instead pass the current time as argument.

### Testing

* use `snapbox::str![]` to assert with `.stdout_eq(str![])` and `stderr_eq(str![])` respectively,
  and auto-update expectations with `SNAPSHOTS=overwrite cargo test -p but`.
* When color is involved, use with `.stdout_eq(snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"])`, and update it 
  with `SNAPSHOT=overwrite cargo test -p but`.

### Linting

* use `cargo fmt --check --all` to check for formatting issues.
* use `cargo clippy --all-targets --fix --allow-dirty` to auto-fix clippy errors.

### CLI Skills

* After changing CLI commands or workflows, update the skill files in `crates/but/skill/` so AI agents stay current
* Users update their skills with `but skill install --detect` (auto-detects installation location)
