use but_claude::{ClaudeMessageContent, GitButlerMessageType, UserInput};
use serde_json;

fn main() {
    // Test serialization of the new GitButlerMessage variant
    let exit_status = ClaudeMessageContent::GitButlerMessage(
        GitButlerMessageType::ExitStatus {
            code: 0,
            message: Some("Command completed successfully".to_string()),
        }
    );

    let error_msg = ClaudeMessageContent::GitButlerMessage(
        GitButlerMessageType::Error {
            error: "Failed to execute command".to_string(),
            source: Some("IO error".to_string()),
        }
    );

    let info_msg = ClaudeMessageContent::GitButlerMessage(
        GitButlerMessageType::Info {
            message: "Process started".to_string(),
        }
    );

    // Test JSON serialization
    println!("Exit Status: {}", serde_json::to_string_pretty(&exit_status).unwrap());
    println!("Error: {}", serde_json::to_string_pretty(&error_msg).unwrap());
    println!("Info: {}", serde_json::to_string_pretty(&info_msg).unwrap());

    // Test that existing variants still work
    let user_input = ClaudeMessageContent::UserInput(UserInput {
        message: "Hello Claude".to_string(),
    });
    println!("User Input: {}", serde_json::to_string_pretty(&user_input).unwrap());
}