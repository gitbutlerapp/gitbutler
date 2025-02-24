use clap::{Arg, ArgAction, Command};

/// Helper function to create a simple command for testing
fn create_simple_command() -> Command {
    Command::new("test")
        .about("A test command")
        .arg(
            Arg::new("input")
                .help("Input file to process")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose output")
                .action(ArgAction::SetTrue),
        )
}

/// Helper function to create a command with subcommands
fn create_command_with_subcommands() -> Command {
    Command::new("git")
        .about("A distributed version control system")
        .long_about("Git is a free and open source distributed version control system\ndesigned to handle everything from small to very large projects\nwith speed and efficiency.")
        .subcommand(
            Command::new("add")
                .about("Add file contents to the index")
                .arg(
                    Arg::new("files")
                        .help("Files to add to the index")
                        .required(true)
                        .num_args(1..)
                        .index(1),
                )
                .arg(
                    Arg::new("all")
                        .short('A')
                        .long("all")
                        .help("Add all tracked and untracked files")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("commit")
                .about("Record changes to the repository")
                .long_about("Stores the current contents of the index in a new commit\nalong with a log message from the user describing the changes.")
                .arg(
                    Arg::new("message")
                        .short('m')
                        .long("message")
                        .help("Commit message")
                        .value_name("MSG")
                        .required(true),
                )
                .arg(
                    Arg::new("amend")
                        .long("amend")
                        .help("Amend the previous commit")
                        .action(ArgAction::SetTrue),
                ),
        )
}

/// Helper function to create a command with options and default values
fn create_command_with_defaults() -> Command {
    Command::new("server")
        .about("Start a web server")
        .arg(
            Arg::new("port")
                .short('p')
                .long("port")
                .help("Port to listen on")
                .value_name("PORT")
                .default_value("8080"),
        )
        .arg(
            Arg::new("host")
                .short('h')
                .long("host")
                .help("Host to bind to")
                .value_name("HOST")
                .default_value("localhost"),
        )
}

/// Helper function to create a command with optional and required arguments
fn create_command_with_mixed_args() -> Command {
    Command::new("copy")
        .about("Copy files from source to destination")
        .arg(
            Arg::new("source")
                .help("Source file or directory")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("destination")
                .help("Destination file or directory")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("backup")
                .help("Create backup of destination if it exists")
                .index(3),
        )
        .arg(
            Arg::new("recursive")
                .short('r')
                .long("recursive")
                .help("Copy directories recursively")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .short('f')
                .long("force")
                .help("Force overwrite of destination")
                .action(ArgAction::SetTrue),
        )
}

/// Helper function to create a command with hidden options
fn create_command_with_hidden_options() -> Command {
    Command::new("app")
        .about("Application command")
        .arg(
            Arg::new("debug")
                .long("debug")
                .help("Enable debug mode")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("internal")
                .long("internal-flag")
                .help("Internal use only")
                .hide(true)
                .action(ArgAction::SetTrue),
        )
}

#[test]
fn test_simple_command_mdx_generation() {
    let cmd = create_simple_command();
    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Check frontmatter
    assert!(mdx.contains("---"));
    assert!(mdx.contains("title: \"`but test`\""));
    assert!(mdx.contains("description: \"A test command\""));

    // Check main heading
    assert!(mdx.contains("# A test command"));

    // Check usage
    assert!(mdx.contains("**Usage:** `but test <INPUT> [OPTIONS]`"));

    // Check arguments section
    assert!(mdx.contains("## Arguments"));
    assert!(mdx.contains("* `<INPUT>` — Input file to process (required)"));

    // Check options section
    assert!(mdx.contains("## Options"));
    assert!(mdx.contains("* `-v`, `--verbose` — Enable verbose output"));

    // Additional checks
    assert!(!mdx.is_empty());
    assert!(mdx.len() > 100);
}

#[test]
fn test_command_with_subcommands_mdx_generation() {
    let cmd = create_command_with_subcommands();
    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Check frontmatter
    assert!(mdx.contains("title: \"`but git`\""));
    assert!(mdx.contains(
        "description: \"Git is a free and open source distributed version control system\""
    ));

    // Check long description
    assert!(mdx.contains("Git is a free and open source distributed version control system"));
    assert!(mdx.contains("designed to handle everything from small to very large projects"));

    // Check usage with subcommands
    assert!(mdx.contains("**Usage:** `but git <COMMAND>`"));

    // Check subcommands section
    assert!(mdx.contains("## Subcommands"));

    // Check 'add' subcommand
    assert!(mdx.contains("### `but git add`"));
    assert!(mdx.contains("Add file contents to the index"));
    assert!(mdx.contains("**Usage:** `but git add <FILES> [OPTIONS]`"));
    assert!(mdx.contains("* `<FILES>` — Files to add to the index (required)"));
    assert!(mdx.contains("* `-A`, `--all` — Add all tracked and untracked files"));

    // Check 'commit' subcommand
    assert!(mdx.contains("### `but git commit`"));
    assert!(mdx.contains("Stores the current contents of the index in a new commit"));
    assert!(mdx.contains("**Usage:** `but git commit [OPTIONS]`"));
    assert!(mdx.contains("* `-m`, `--message` `<MSG>` — Commit message (required)"));
    assert!(mdx.contains("* `--amend` — Amend the previous commit"));
}

#[test]
fn test_command_with_defaults_mdx_generation() {
    let cmd = create_command_with_defaults();
    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Check default values are documented
    assert!(mdx.contains("(default: `8080`)"));
    assert!(mdx.contains("(default: `localhost`)"));

    // Check value names
    assert!(mdx.contains("`<PORT>`"));
    assert!(mdx.contains("`<HOST>`"));

    // Additional checks
    assert!(!mdx.is_empty());
    assert!(mdx.len() > 100);
}

#[test]
fn test_command_with_mixed_args_mdx_generation() {
    let cmd = create_command_with_mixed_args();
    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Check usage with multiple positional args
    assert!(mdx.contains("**Usage:** `but copy <SOURCE> <DESTINATION> [BACKUP] [OPTIONS]`"));

    // Check required and optional arguments
    assert!(mdx.contains("* `<SOURCE>` — Source file or directory (required)"));
    assert!(mdx.contains("* `<DESTINATION>` — Destination file or directory (required)"));
    assert!(mdx.contains("* `<BACKUP>` — Create backup of destination if it exists"));
    // Optional arg should not have "(required)"
    assert!(!mdx.contains("* `<BACKUP>` — Create backup of destination if it exists (required)"));

    // Check options
    assert!(mdx.contains("* `-r`, `--recursive` — Copy directories recursively"));
    assert!(mdx.contains("* `-f`, `--force` — Force overwrite of destination"));
}

#[test]
fn test_command_with_hidden_options_mdx_generation() {
    let cmd = create_command_with_hidden_options();
    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Visible option should be present
    assert!(mdx.contains("* `--debug` — Enable debug mode"));

    // Hidden option should NOT be present
    assert!(!mdx.contains("internal-flag"));
    assert!(!mdx.contains("Internal use only"));
}

#[test]
fn test_real_but_command_structure() {
    use clap::CommandFactory;

    // Test with actual but command to ensure it generates valid MDX
    let cmd = but::args::Args::command();

    // Get a non-hidden subcommand
    let status_cmd = cmd
        .get_subcommands()
        .find(|sub| sub.get_name() == "status" && !sub.is_hide_set())
        .expect("status subcommand should exist and not be hidden");

    let mdx = but_clap::generator::generate_command_mdx(status_cmd);

    // Basic structure checks
    assert!(mdx.contains("---\n"));
    assert!(mdx.contains("title:"));
    assert!(mdx.contains("description:"));
    assert!(mdx.contains("**Usage:**"));

    // Should not be empty
    assert!(mdx.len() > 100);
}

#[test]
fn test_command_with_long_help() {
    let cmd = Command::new("example")
        .about("Short description")
        .long_about("This is a much longer description\nthat spans multiple lines\nand provides more detail")
        .arg(
            Arg::new("file")
                .help("Short help for file")
                .long_help("Long help for file\nwith additional details\nabout what the file should contain")
                .required(true)
                .index(1),
        );

    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Should use long_about for the description
    assert!(mdx.contains("This is a much longer description"));
    assert!(mdx.contains("that spans multiple lines"));

    // Should use long_help for arguments
    assert!(mdx.contains("Long help for file"));
    assert!(mdx.contains("with additional details"));
}

#[test]
fn test_frontmatter_only_first_line() {
    let cmd = Command::new("multiline")
        .long_about("First line of description\nSecond line of description\nThird line");

    let mdx = but_clap::generator::generate_command_mdx(&cmd);

    // Frontmatter description should only have first line
    assert!(mdx.contains("description: \"First line of description\""));

    // But full description should be in the body
    assert!(mdx.contains("# First line of description\n\nFirst line of description\nSecond line of description\nThird line"));
}
