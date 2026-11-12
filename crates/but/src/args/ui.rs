use but_ui_control::ui::Input;

/// Drive the running GitButler GPUI app as the keyboard does.
///
/// `dispatch` runs any of the app's actions by name, as `tiling::SplitRight`, where the keys
/// are. `actions` marks the ones that throw work away as risky.
///
/// The app listens at $GITBUTLER_GPUI_SOCKET when set, as in its own terminals.
#[derive(Debug, clap::Parser)]
#[cfg_attr(feature = "raw-clap-docs", clap(verbatim_doc_comment))]
#[deny(missing_docs)]
pub struct Platform {
    /// The subcommand to run.
    #[clap(subcommand)]
    pub cmd: Subcommands,
}

/// The `but _ui` subcommands.
#[derive(Debug, clap::Subcommand)]
pub enum Subcommands {
    /// Print the selected row, the tiles and their panes, and where the keys are.
    State,
    /// List the actions that would act where the keys are, with their keys.
    Actions,
    /// Run an action where the keys are, or in a tile and pane given the keys first.
    Dispatch {
        /// The action, as `actions` names it.
        action: String,
        /// The action's argument, as JSON.
        #[clap(long, value_name = "JSON")]
        args: Option<String>,
        /// Give this tile the keys first, by the number `state` lists it by, or `navigator`.
        #[clap(long, value_name = "TILE")]
        tile: Option<String>,
        /// Give this pane the keys first, as `files` or `diff`.
        #[clap(long, value_name = "PANE")]
        pane: Option<String>,
    },
    /// Select a row, as Go to anything finds it typed in full.
    Select {
        /// A project, branch, commit, worktree or page, by name.
        query: String,
    },
    /// Give a tile, or a pane of its page, the keys.
    Focus {
        /// By the number `state` lists it by, or `navigator`. Defaults to the tile with the keys.
        #[clap(long, value_name = "TILE")]
        tile: Option<String>,
        /// The pane, as `files` or `diff`.
        #[clap(long, value_name = "PANE")]
        pane: Option<String>,
    },
    /// Type text and press keys, in the order given: where the keys are, into a text field or a
    /// terminal, or with `--tile` into that tile's terminal, leaving the keys where they are.
    Type(TypeArgs),
    /// Open a new terminal, or a row or page, in a new tile beside another, in whichever layout
    /// that is, leaving the keys and the selection where they are. Prints the new tile.
    #[clap(group(clap::ArgGroup::new("what").required(true)))]
    Open {
        /// The tile to open it beside, by the number `state` lists it by.
        #[clap(long, value_name = "TILE")]
        beside: String,
        /// Below it, instead of to its right.
        #[clap(long)]
        below: bool,
        /// A new terminal, on the checkout of what the tile beside shows.
        #[clap(long, group = "what")]
        terminal: bool,
        /// A command to run in the new terminal, through the login shell.
        #[clap(
            long,
            value_name = "COMMAND",
            conflicts_with = "show",
            allow_hyphen_values = true
        )]
        run: Option<String>,
        /// A row or a project's page, by name, as Go to anything finds it: a branch, a commit,
        /// `Graph`. Those in the project of the tile beside come first.
        #[clap(long, value_name = "NAME", group = "what")]
        show: Option<String>,
    },
    /// Close a tile, in whichever layout it is, as its × does, leaving the keys and the selection
    /// where they are. A terminal living in the tile closes with it, stopping what runs in it,
    /// without asking first. A page's home tile closes only with its row.
    Close {
        /// The tile, by the number `state` lists it by.
        #[clap(long, value_name = "TILE")]
        tile: String,
    },
    /// Read a terminal, in whichever layout it is: its title, what the program running in it
    /// reports it's doing, and its lines as plain text.
    Read {
        /// The tile showing the terminal, by the number `state` lists it by.
        #[clap(long, value_name = "TILE")]
        tile: String,
        /// Read only this many of the last lines. Without it, all of them: the scrollback and
        /// the screen.
        #[clap(long, value_name = "N")]
        lines: Option<usize>,
    },
}

/// What `type` sends, in the order its `--text` and `--key` flags were given.
///
/// Written by hand, as clap's derive keeps each flag's values apart and loses the order between
/// them.
#[derive(Debug)]
pub struct TypeArgs {
    /// Send it to this tile's terminal instead.
    pub tile: Option<String>,
    /// What to send, in order.
    pub input: Vec<Input>,
}

impl clap::FromArgMatches for TypeArgs {
    fn from_arg_matches(matches: &clap::ArgMatches) -> Result<Self, clap::Error> {
        let mut input: Vec<(usize, Input)> = Vec::new();
        for (name, piece) in [
            ("text", Input::Text as fn(String) -> Input),
            ("key", Input::Key),
        ] {
            if let (Some(values), Some(at)) =
                (matches.get_many::<String>(name), matches.indices_of(name))
            {
                input.extend(at.zip(values.cloned().map(piece)));
            }
        }
        input.sort_by_key(|(at, _)| *at);
        Ok(TypeArgs {
            tile: matches.get_one::<String>("tile").cloned(),
            input: input.into_iter().map(|(_, piece)| piece).collect(),
        })
    }

    fn update_from_arg_matches(&mut self, matches: &clap::ArgMatches) -> Result<(), clap::Error> {
        *self = TypeArgs::from_arg_matches(matches)?;
        Ok(())
    }
}

impl clap::Args for TypeArgs {
    fn augment_args(command: clap::Command) -> clap::Command {
        command
            .arg(
                clap::Arg::new("text")
                    .long("text")
                    .value_name("TEXT")
                    .allow_hyphen_values(true)
                    .action(clap::ArgAction::Append)
                    .help("Text to type as it is. A newline presses Enter, and a tab Tab."),
            )
            .arg(
                clap::Arg::new("key")
                    .long("key")
                    .value_name("KEY")
                    .allow_hyphen_values(true)
                    .action(clap::ArgAction::Append)
                    .help(
                        "A key to press, as `enter`, `backspace`, `up`, `escape` or `ctrl-c`, \
                         as keymaps write keys.",
                    ),
            )
            .group(
                clap::ArgGroup::new("input")
                    .args(["text", "key"])
                    .multiple(true)
                    .required(true),
            )
            .arg(clap::Arg::new("tile").long("tile").value_name("TILE").help(
                "Send it to this tile's terminal instead, by the number `state` lists \
                         it by, leaving the keys where they are.",
            ))
    }

    fn augment_args_for_update(command: clap::Command) -> clap::Command {
        TypeArgs::augment_args(command)
    }
}
