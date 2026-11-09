use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Browse tmux sessions without changing them.",
    after_help = "Without a command, Sagebrush opens the TUI when stdin and stdout are terminals.\nUse `sagebrush list` for redirected output. No configuration file is read.\nBoth views show name, window count, uptime, current-window dimensions and active status.\nInside tmux the inherited server is used; outside tmux the default server is used.\nNo server is treated as empty and Sagebrush never creates one.\nNO_COLOR, TERM=dumb, and --no-color disable color; redirected streams are unstyled.\nIf tmux is missing or inaccessible, install tmux 3.3+ and check the server.\nFor a small terminal, resize to at least 40x10; the TUI retries failed refreshes.\nFor invalid input, consult --help. List statuses: 0 success, 1 error/partial, 2 usage;\n129/130/143 signal cancellation, 141 closed output pipe.\nExamples:\n  sagebrush\n  sagebrush tui\n  sagebrush list --json"
)]
pub struct Cli {
    /// Disable color on every output stream.
    #[arg(long, global = true)]
    pub no_color: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Open the interactive session browser.
    #[command(
        after_help = "Requires terminal stdin and stdout. Press ? for keyboard help.\nUse j/k to navigate, Esc to return from help and q to quit. The focused row\nis not the active session. Details crawl automatically and sessions refresh.\nResize to at least 40x10 if the terminal is too small; failed requests retry.\nNO_COLOR, TERM=dumb, and --no-color disable color."
    )]
    Tui,
    /// List sessions without interactive navigation.
    #[command(
        after_help = "Shows name, total windows, creation-based uptime, current-window width and height, and active status.\nActive identifies the invoking client's session, not every attached session.\nOutside tmux no session is active; if the client cannot be identified, active status is unavailable.\nSessions are ordered by numeric tmux ID. Unavailable fields are reported explicitly.\nJSON keys: name, windows, uptime, width, height, active. Uptime is integer seconds;\neach unavailable value is null. Human uptime uses d/h/m/s, size uses WIDTHxHEIGHT.\nPlain output is headerless tab-separated text. Text names escape backslashes, tabs,\nnewlines, carriage returns and other controls; JSON preserves the original name.\nEmpty JSON is []; redirected table and plain output are empty when there are no sessions.\nNo output is truncated or paged. NO_COLOR, TERM=dumb, and --no-color disable color;\nnon-terminal streams are unstyled. No configuration file is read.\nInside tmux the inherited server is used; outside tmux the default server is used.\nAn absent server is empty; it is never created. Invalid inherited context is an error.\nExit status: 0 success, 1 retrieval failure or partial data, 2 invalid usage,\n129/130/143 signal cancellation, 141 closed output pipe.\nIf tmux is missing or inaccessible, install tmux 3.3+ and check the server;\nfor invalid options consult list --help.\nExamples:\n  sagebrush list --plain | cut -f1\n  sagebrush list --json"
    )]
    List {
        /// Emit a pretty JSON array with typed fields.
        #[arg(long, conflicts_with = "plain")]
        json: bool,
        /// Emit headerless tab-separated text.
        #[arg(long)]
        plain: bool,
    },
}
