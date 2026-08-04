use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "anchor",
    about = "Attach judgment to recomputable observations",
    long_about = "You write the memories; anchors are mechanical observations.\n\
                  Probes say what to inspect, transition tables say what counts as change,\n\
                  and this tool ships no built-in criteria."
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
    /// Create .anchor/, register bundled probes, and report what is readable.
    Init,

    /// Build every declared probe recipe and install it for this machine.
    #[command(subcommand)]
    Probes(ProbesCmd),

    Sync {
        #[arg(default_value = crate::verbs::sync::DEFAULT_FILE)]
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
    },

    /// Publish a directory as a probe artifact and print its earned version.
    Publish {
        from: String,
        #[arg(long, default_value = "probe")]
        entrypoint: String,
        #[arg(long = "arg")]
        args: Vec<String>,
        /// Environment required by the probe (K=V). It enters the version and your responsibility.
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
        #[arg(long = "rule", value_name = "GUARD => NEW_STATE", required = true)]
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

    /// Re-stamp a binding's content version without changing which anchors it's about.
    Reaffirm {
        path: String,
    },

    /// Other references bound to any anchor `path` is also bound to.
    Cobound {
        path: String,
    },

    /// Record that `from` relates to `to`. Independent of anchoring — linking
    /// two references says nothing about which anchors either is bound to.
    Link {
        from: String,
        to: String,
        #[arg(long)]
        kind: String,
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

    /// Force `key` due now, clearing any backoff or parked state. Unlike
    /// `sync`, which only repairs a missing queue row, this always resets it.
    Requeue {
        key: String,
    },

    Pass,

    Doctor,
}

#[derive(Subcommand)]
pub enum ProbesCmd {
    /// Build, publish and install every recipe. Users get these prebuilt.
    Build,
    /// What each probe is, and the obs vocabulary it emits.
    List {
        #[arg(short, long)]
        verbose: bool,
    },
}

#[derive(clap::Args)]
pub struct OpenArgs {
    pub key: String,
    /// Probe artifact version printed by `anchor publish`.
    #[arg(long)]
    pub artifact: String,
    #[arg(long, default_value = "{}")]
    pub params: String,
    #[arg(long = "rule", value_name = "GUARD => NEW_STATE")]
    pub rules: Vec<String>,
    #[arg(long = "terminal", value_delimiter = ',')]
    pub terminal: Vec<String>,
    /// Keep a full record even when the world did not move.
    #[arg(long)]
    pub retain_full: bool,
    /// This anchor's observation cadence, in seconds.
    #[arg(long)]
    pub cadence_secs: Option<u64>,
    /// Supersede an already-closed anchor. Closure is irreversible; correction opens a new generation.
    #[arg(long, requires = "why")]
    pub supersedes: Option<String>,
    #[arg(long, requires = "supersedes")]
    pub why: Option<String>,
}
