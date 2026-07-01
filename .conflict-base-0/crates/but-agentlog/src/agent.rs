#[derive(Debug, Copy, Clone, Eq, PartialEq, clap::ValueEnum)]
pub enum Agent {
    Codex,
    Claude,
}

impl Agent {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Agent::Codex => "codex",
            Agent::Claude => "claude",
        }
    }
}
