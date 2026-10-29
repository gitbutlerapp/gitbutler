use std::{fs, path::Path};

use anyhow::{Context as _, Result};
use but_clap::generator;

fn main() -> Result<()> {
    use clap::CommandFactory;

    let docs_dir = Path::new("cli-docs");
    let next_docs_dir = Path::new("cli-docs.next");
    if next_docs_dir.exists() {
        fs::remove_dir_all(next_docs_dir)
            .context("Failed to remove stale cli-docs.next directory")?;
    }
    fs::create_dir_all(next_docs_dir).context("Failed to create cli-docs.next directory")?;

    // Get the main Args command
    let app = but::args::Args::command();

    // Generate documentation for each non-hidden subcommand
    for subcommand in app.get_subcommands() {
        if subcommand.is_hide_set() {
            continue;
        }

        let subcommand_name = subcommand.get_name();
        let file_path = next_docs_dir.join(format!("but-{subcommand_name}.mdx"));

        let mdx_content = generator::generate_command_mdx(subcommand);
        fs::write(&file_path, mdx_content).with_context(|| {
            format!("Failed to write subcommand documentation to {file_path:?}")
        })?;
        println!("Generated: {file_path:?}");
    }

    // Generate documentation for each help topic.
    if let Some(help_command) = app
        .get_subcommands()
        .find(|subcommand| subcommand.get_name() == "help")
    {
        for topic_command in help_command.get_subcommands() {
            if topic_command.is_hide_set() {
                continue;
            }

            let topic_name = topic_command.get_name();
            let file_path = next_docs_dir.join(format!("but-help-{topic_name}.mdx"));
            let mdx_content = generator::generate_topic_mdx(
                topic_command,
                &[help_command.get_name(), topic_command.get_name()],
            );
            fs::write(&file_path, mdx_content)
                .with_context(|| format!("Failed to write topic documentation to {file_path:?}"))?;
            println!("Generated: {file_path:?}");
        }
    }

    if docs_dir.exists() {
        fs::remove_dir_all(docs_dir).context("Failed to remove existing cli-docs directory")?;
    }
    fs::rename(next_docs_dir, docs_dir).context("Failed to install generated documentation")?;

    println!("\nDocumentation generation complete!");
    Ok(())
}
