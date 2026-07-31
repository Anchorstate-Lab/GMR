use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "anchor",
    about = "把判断挂靠在可重算的观测上",
    long_about = "记忆是你写的；锚是机械观测的。\n\
                  探针说去看什么，转换表说什么算变了 —— 两样都由你交，\n\
                  这个工具不带任何出厂判据。"
)]
pub struct Cli {
    #[arg(long, default_value = ".", global = true)]
    pub repo: String,

    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Sync {
        #[arg(default_value = "anchors.toml")]
        file: String,
        #[arg(long)]
        dry_run: bool,
    },

    Open {
        key: String,
        #[arg(long)]
        probe: String,
        #[arg(long = "rule", value_name = "守卫 => 新状态")]
        rules: Vec<String>,
        #[arg(long = "terminal", value_delimiter = ',')]
        terminal: Vec<String>,
    },

    Observe {
        key: Option<String>,
    },

    Read {
        key: Option<String>,
        #[arg(long)]
        moved: bool,
    },

    Reprobe {
        key: String,
        #[arg(long)]
        probe: String,
        #[arg(long)]
        why: String,
    },

    Retransition {
        key: String,
        #[arg(long = "rule", value_name = "守卫 => 新状态", required = true)]
        rules: Vec<String>,
        #[arg(long)]
        why: String,
    },

    Reterminal {
        key: String,
        #[arg(long = "terminal", value_delimiter = ',', required = true)]
        terminal: Vec<String>,
        #[arg(long)]
        why: String,
    },

    Restate {
        key: String,
        #[arg(long)]
        state: String,
        #[arg(long)]
        why: String,
    },

    Bind {
        path: String,
        #[arg(long, value_delimiter = ',', conflicts_with = "detach")]
        anchors: Vec<String>,
        #[arg(long)]
        detach: bool,
    },

    Close {
        key: String,
        #[arg(long)]
        why: String,
    },

    Edges {
        #[arg(long, default_value = "0")]
        since: u64,
        #[arg(long)]
        status: Option<String>,
    },

    Health {
        key: Option<String>,
    },

    Pass,

    Doctor,
}
