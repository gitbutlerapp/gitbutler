### API usage

* Avoid using code from `gitbutler-`-prefixed crates, and prefer code from `but-` prefixed crates as long as it's not in the `legacy` module.

### Output
* Usable output goes to `stdout` with `writeln!(stdout, …).ok()`.
    - Obtain `stdout` once per function using `let mut stdout = std::io::stdout();`
    - Use `stdout` when writing: `writeln!(stdout, "…").ok();`
    - Use `atty::is` to print human output, otherwise print output optimised for use in bash scripts, when single values are returned.
    - **But** when writing `json`, always use `writeln!(stdout, "{json_pretty}")?`
* Error or side-channel information goes to `stderr` with `writeln!(stderr, …).ok()`.
    - Obtain `stderr` once per function using `let mut stderr = std::io::stderr();`
    - Use `stderr` when writing: `writeln!(stderr, "…").ok();`
* The `.ok()` at the end ignores write errors gracefully (e.g., broken pipe) instead of panicking.
* `--json` only outputs the happy path, there are no JSON errors.

### Testing

* use `snapbox::str![]` to assert with `.stdout_eq(str![])` and `stderr_eq(str![])` respectively,
  and auto-update expectations with `SNAPSHOTS=overwrite cargo test -p but`.
* When color is involved, use with `.stdout_eq(snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"])`, and update it 
  with `SNAPSHOT=overwrite cargo test -p but`.
