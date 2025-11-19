use clap::{Arg, Command};

pub fn generate_command_mdx(cmd: &Command) -> String {
    let mut output = String::new();

    // Generate frontmatter
    let title = format!("`but {}`", cmd.get_name());
    let description = get_description(cmd);
    output.push_str("---\n");
    output.push_str(&format!("title: \"{}\"\n", title));
    output.push_str(&format!("description: \"{}\"\n", description));
    output.push_str("---\n\n");

    // Add main heading with description
    output.push_str(&format!("# {}\n\n", description));

    // Add long description if available (full version, not just first line)
    if let Some(long_about) = cmd.get_long_about() {
        output.push_str(&format!("{}\n\n", long_about));
    } else if let Some(about) = cmd.get_about() {
        // If no long_about but we have about, and it's different from description
        let about_str = about.to_string();
        if about_str != description {
            output.push_str(&format!("{}\n\n", about_str));
        }
    }

    // Add usage
    let usage = generate_usage(cmd);
    output.push_str(&format!("**Usage:** `{}`\n\n", usage));

    // Add subcommands section if there are any
    let subcommands: Vec<_> = cmd
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set())
        .collect();

    if !subcommands.is_empty() {
        output.push_str("## Subcommands\n\n");
        for subcmd in &subcommands {
            let sub_name = subcmd.get_name();

            output.push_str(&format!("### `but {} {}`\n\n", cmd.get_name(), sub_name));

            // Add subcommand long description
            if let Some(long_about) = subcmd.get_long_about() {
                output.push_str(&format!("{}\n\n", long_about));
            } else if let Some(about) = subcmd.get_about() {
                output.push_str(&format!("{}\n\n", about));
            }

            // Add subcommand usage
            let sub_usage = generate_usage_for_subcommand(cmd.get_name(), subcmd);
            output.push_str(&format!("**Usage:** `{}`\n\n", sub_usage));

            // Add subcommand arguments
            let args: Vec<_> = subcmd
                .get_positionals()
                .filter(|arg| !arg.is_hide_set())
                .collect();
            if !args.is_empty() {
                output.push_str("**Arguments:**\n\n");
                for arg in args {
                    output.push_str(&generate_argument_doc(arg));
                }
                output.push_str("\n");
            }

            // Add subcommand options/flags
            let opts: Vec<_> = subcmd
                .get_opts()
                .filter(|arg| !arg.is_hide_set())
                .collect();
            if !opts.is_empty() {
                output.push_str("**Options:**\n\n");
                for opt in opts {
                    output.push_str(&generate_option_doc(opt));
                }
                output.push_str("\n");
            }
        }
    }

    // Add arguments for the main command (if it has no subcommands or also accepts args)
    let args: Vec<_> = cmd.get_positionals().filter(|arg| !arg.is_hide_set()).collect();
    if !args.is_empty() && subcommands.is_empty() {
        output.push_str("## Arguments\n\n");
        for arg in args {
            output.push_str(&generate_argument_doc(arg));
        }
        output.push_str("\n");
    }

    // Add options/flags for the main command
    let opts: Vec<_> = cmd.get_opts().filter(|arg| !arg.is_hide_set()).collect();
    if !opts.is_empty() && subcommands.is_empty() {
        output.push_str("## Options\n\n");
        for opt in opts {
            output.push_str(&generate_option_doc(opt));
        }
        output.push_str("\n");
    }

    output
}

fn get_description(cmd: &Command) -> String {
    // For frontmatter description, use the first line of long_about or about
    cmd.get_long_about()
        .or_else(|| cmd.get_about())
        .map(|s| {
            // Take only the first line for short description
            s.to_string()
                .lines()
                .next()
                .unwrap_or("Command-line documentation")
                .to_string()
        })
        .unwrap_or_else(|| "Command-line documentation".to_string())
}

fn generate_usage(cmd: &Command) -> String {
    let mut usage = format!("but {}", cmd.get_name());

    // Add subcommands indicator if present
    if cmd.has_subcommands() {
        usage.push_str(" <COMMAND>");
    }

    // Add positional arguments
    for arg in cmd.get_positionals() {
        if arg.is_hide_set() {
            continue;
        }
        let arg_name = arg.get_id().as_str().to_uppercase();
        if arg.is_required_set() {
            usage.push_str(&format!(" <{}>", arg_name));
        } else {
            usage.push_str(&format!(" [{}]", arg_name));
        }
    }

    // Add options indicator
    if cmd.get_opts().any(|opt| !opt.is_hide_set()) {
        usage.push_str(" [OPTIONS]");
    }

    usage
}

fn generate_usage_for_subcommand(parent: &str, cmd: &Command) -> String {
    let mut usage = format!("but {} {}", parent, cmd.get_name());

    // Add positional arguments
    for arg in cmd.get_positionals() {
        if arg.is_hide_set() {
            continue;
        }
        let arg_name = arg.get_id().as_str().to_uppercase();
        if arg.is_required_set() {
            usage.push_str(&format!(" <{}>", arg_name));
        } else {
            usage.push_str(&format!(" [{}]", arg_name));
        }
    }

    // Add options indicator
    if cmd.get_opts().any(|opt| !opt.is_hide_set()) {
        usage.push_str(" [OPTIONS]");
    }

    usage
}

fn generate_argument_doc(arg: &Arg) -> String {
    let mut doc = String::new();
    let arg_name = arg.get_id().as_str().to_uppercase();

    doc.push_str(&format!("* `<{}>` ", arg_name));

    // Add help text, preferring long_help
    if let Some(long_help) = arg.get_long_help() {
        doc.push_str(&format!("— {}", long_help));
    } else if let Some(help) = arg.get_help() {
        doc.push_str(&format!("— {}", help));
    }

    // Add required indicator
    if arg.is_required_set() {
        doc.push_str(" (required)");
    }

    doc.push_str("\n");
    doc
}

fn generate_option_doc(opt: &Arg) -> String {
    let mut doc = String::new();

    // Build the option signature
    let mut sig = String::new();

    if let Some(short) = opt.get_short() {
        sig.push_str(&format!("`-{}`", short));
    }

    if let Some(long) = opt.get_long() {
        if !sig.is_empty() {
            sig.push_str(", ");
        }
        sig.push_str(&format!("`--{}`", long));
    }

    // Add value name if it takes a value
    if opt.get_action().takes_values() {
        let value_name = opt
            .get_value_names()
            .and_then(|names| names.first())
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| opt.get_id().as_str().to_uppercase());
        sig.push_str(&format!(" `<{}>`", value_name));
    }

    doc.push_str(&format!("* {} ", sig));

    // Add help text, preferring long_help
    if let Some(long_help) = opt.get_long_help() {
        doc.push_str(&format!("— {}", long_help));
    } else if let Some(help) = opt.get_help() {
        doc.push_str(&format!("— {}", help));
    }

    // Add default value
    let default_values = opt.get_default_values();
    if !default_values.is_empty() {
        let default_str = default_values[0].to_string_lossy();
        doc.push_str(&format!(" (default: `{}`)", default_str));
    }

    // Add required indicator
    if opt.is_required_set() {
        doc.push_str(" (required)");
    }

    doc.push_str("\n");
    doc
}
