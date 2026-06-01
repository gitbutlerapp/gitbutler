use crate::utils::Sandbox;

#[test]
fn commit2_help() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata_at_target(&["A"], "origin/main").unwrap();

    env.but("commit2 --help")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
Usage: but commit2 [OPTIONS]

Options:
      --format <FORMAT>
          Explicitly control how output should be formatted.
          
          If unset and from a terminal, it defaults to human output, when redirected it's for
          shells.

          Possible values:
          - human: The output to write is supposed to be for human consumption, and can be more
            verbose
          - shell: The output should be suitable for shells, and assigning the major result to
            variables so that it can be reused in subsequent CLI invocations
          - json:  Output detailed information as JSON for tool consumption
          - none:  Do not output anything, like redirecting to /dev/null
          
          [env: BUT_OUTPUT_FORMAT=]
          [default: human]

  -h, --help
          Print help (see a summary with '-h')

"#]]);
}
