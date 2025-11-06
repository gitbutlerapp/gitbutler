use crate::utils::Sandbox;
use crate::utils::setup_metadata;

#[test]
fn status_json_shows_paths_as_strings() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target("two-stacks")?;

    // Must set metadata to match the scenario, or else the old APIs used here won't deliver.
    setup_metadata(&env, &["A", "B"])?;

    // Create a new file to ensure we have file assignments
    env.file("test-file.txt", "test content");

    let output = env
        .but("--json status")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout)?;

    // Parse the JSON output
    let json_value: serde_json::Value = serde_json::from_str(&stdout)?;

    // Check that paths are strings, not byte arrays
    let mut found_path = false;
    if let Some(stacks) = json_value["stacks"].as_array() {
        for stack in stacks {
            // Each stack is a tuple: [stack_id, [stack_details, [file_assignments]]]
            if let Some(stack_data) = stack.as_array()
                && stack_data.len() >= 2
                    && let Some(tuple) = stack_data[1].as_array()
                        && tuple.len() >= 2
                            && let Some(assignments) = tuple[1].as_array() {
                                for assignment in assignments {
                                    if let Some(path) = assignment.get("path") {
                                        found_path = true;
                                        // If path is a string, this will be Some
                                        // If it's an array of bytes, this will be None
                                        assert!(
                                            path.is_string(),
                                            "Expected path to be a string, but got: {:?}",
                                            path
                                        );
                                    }
                                }
                            }
        }
    }

    // Ensure we actually tested at least one path
    assert!(found_path, "No file paths found in JSON output");

    Ok(())
}
