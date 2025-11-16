### API usage

* Avoid using code from `gitbutler-`-prefixed crates, and prefer code from `but-` prefixed crates as long as it's not in the `legacy` module.

### Output

Usable output goes to `out: &mut OutputChannel`

- For humans, use `if let Some(out) = out.for_human() { writeln!(out, “{…}")?; }`
- For shell, use `if let Some(out) = out.for_shell() { writeln!(out, “{…}")?; }`
- For JSON, use `if let Some(out) = out.for_json() { out.write_value(json)?; }`

### Testing

* use `snapbox::str![]` to assert with `.stdout_eq(str![])` and `stderr_eq(str![])` respectively,
  and auto-update expectations with `SNAPSHOTS=overwrite cargo test -p but`.
* When color is involved, use with `.stdout_eq(snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"])`, and update it 
  with `SNAPSHOT=overwrite cargo test -p but`.
