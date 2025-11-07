### Output
* Usable output goes to `stdout` with `wrinteln!(stdout, "…")`, with `stdout` being a re-used variable filled with `std::io::stdout()`.
    - Use `atty::is` to print human output, otherwise print output optimised for use in shell scripts, when single values are returned.
* Error or side-channel information goes to `stderr` with `writeln!(stderr, "…")`, with `stderr` being a re-used variable filled with `std::io::stderr()` as needed.
* `--json` only outputs the happy path, there are no JSON errors.

### Testing

* use `snapbox::str![]` to assert with `.stdout_eq(str![])` and `stderr_eq(str![])` respectively,
  and auto-update expectations with `SNAPSHOTS=overwrite cargo test -p but`.
* When color is involved, use with `.stdout_eq(snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"])`, and update it 
  with `SNAPSHOT=overwrite cargo test -p but`.
