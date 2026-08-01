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

    Open(OpenArgs),

    Observe {
        key: Option<String>,
    },

    Read {
        key: Option<String>,
        #[arg(long)]
        moved: bool,
    },

    /// 把一棵目录发布成探针 artifact，打印它挣来的版本号。
    Publish {
        from: String,
        #[arg(long, default_value = "probe")]
        entrypoint: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        /// 声明探针需要的环境（K=V）。它进版本号 —— 也进你要负的责任。
        #[arg(long = "env")]
        env: Vec<String>,
    },

    Reprobe {
        key: String,
        #[arg(long)]
        artifact: String,
        #[arg(long, default_value = "{}")]
        params: String,
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

#[derive(clap::Args)]
pub struct OpenArgs {
    pub key: String,
    /// 探针 artifact 的版本号（anchor publish 打印的那个）。
    #[arg(long)]
    pub artifact: String,
    #[arg(long, default_value = "{}")]
    pub params: String,
    #[arg(long = "rule", value_name = "守卫 => 新状态")]
    pub rules: Vec<String>,
    #[arg(long = "terminal", value_delimiter = ',')]
    pub terminal: Vec<String>,
    /// 世界没动时也留完整记录。
    #[arg(long)]
    pub retain_full: bool,
    /// 这个锚自己的观测节奏，秒。
    #[arg(long)]
    pub cadence_secs: Option<u64>,
    /// 接替哪个已经终结的锚。终结不可撤销，纠错只能开新的一代。
    #[arg(long, requires = "why")]
    pub supersedes: Option<String>,
    #[arg(long, requires = "supersedes")]
    pub why: Option<String>,
}
