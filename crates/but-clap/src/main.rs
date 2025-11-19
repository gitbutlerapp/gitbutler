use std::{fs, path::Path};

use anyhow::{Context, Result};
use but_clap::generator;

fn main() -> Result<()> {
    use clap::CommandFactory;

    // Create the cli-docs directory if it doesn't exist
    let docs_dir = Path::new("cli-docs");
    fs::create_dir_all(docs_dir).context("Failed to create cli-docs directory")?;

    // Get the main Args command
    let app = but::args::Args::command();

    // Generate documentation for each non-hidden subcommand
    for subcommand in app.get_subcommands() {
        if subcommand.is_hide_set() {
            continue;
        }

        let subcommand_name = subcommand.get_name();
        let file_path = docs_dir.join(format!("but-{}.mdx", subcommand_name));

        let mdx_content = generator::generate_command_mdx(subcommand);
        fs::write(&file_path, mdx_content).with_context(|| {
            format!(
                "Failed to write subcommand documentation to {:?}",
                file_path
            )
        })?;
        println!("Generated: {:?}", file_path);
    }

    println!("\nDocumentation generation complete!");
    Ok(())
}
